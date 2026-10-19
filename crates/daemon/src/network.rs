// Historical fixed-role implementation from 0.2.0; not compiled.
// Replaced by peers.rs in 0.3.0. Kept to preserve the previous implementation.
use crate::{config::Config, native::*, status::*};
use anyhow::{bail, Context, Result};
use crossdesk_core::*;
use crossdesk_platform as platform;
use crossdesk_transport::*;
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpListener, TcpStream},
    sync::mpsc,
    time::timeout,
};
use tokio_rustls::{TlsAcceptor, TlsConnector};

async fn send<W: AsyncWrite + Unpin>(w: &mut W, m: &Message) -> Result<()> {
    timeout(Duration::from_millis(100), write_frame(w, m)).await??;
    Ok(())
}
fn hello(id: &Identity, session: u64, topology: DeviceTopology) -> Message {
    Message::Hello {
        version: VERSION,
        source: id.id.clone(),
        session,
        topology,
    }
}
fn check_hello(m: Message, id: &str) -> Result<(u64, DeviceTopology)> {
    match m {
        Message::Hello {
            version,
            source,
            session,
            topology,
        } if version == VERSION && source == id && topology.device.id == id => {
            Layout {
                topologies: vec![topology.clone()],
                links: vec![],
            }
            .validate()
            .map_err(anyhow::Error::msg)?;
            Ok((session, topology))
        }
        _ => bail!("invalid authenticated hello"),
    }
}
pub async fn listen(
    identity: Arc<Identity>,
    config: Config,
    status: SharedStatus,
    stop: Arc<AtomicBool>,
) -> Result<()> {
    let peers = load_trust(&data_dir())?;
    let acceptor = TlsAcceptor::from(identity.server_config(&peers)?);
    let listener = TcpListener::bind((std::net::Ipv4Addr::UNSPECIFIED, config.port)).await?;
    update(&status, |s| s.state = "等待已授权来源连接".into());
    while !stop.load(Ordering::Acquire) {
        let accepted = timeout(Duration::from_millis(100), listener.accept()).await;
        let Ok(Ok((socket, _))) = accepted else {
            continue;
        };
        socket.set_nodelay(true)?;
        // Single connection is intentional: no second source can steal the target.
        let result = async {
            let mut stream = timeout(Duration::from_secs(3), acceptor.accept(socket)).await??;
            let peer = verify_peer(stream.get_ref().1.peer_certificates(), &peers)?;
            let (session, remote_topology) = check_hello(
                timeout(Duration::from_secs(2), read_frame(&mut stream)).await??,
                &peer.id,
            )?;
            let topo = platform::topology(&identity.id, 1)?;
            send(&mut stream, &hello(&identity, session, topo.clone())).await?;
            update(&status, |s| {
                s.topology = vec![topo.clone(), remote_topology];
                s.source = Some(peer.id.clone());
                s.state = "已连接 · 等待 Ctrl 切入".into();
            });
            receive_session(
                &mut stream,
                &peer.id,
                session,
                topo,
                status.clone(),
                stop.clone(),
                config.tuning.lease_ms,
            )
            .await
        }
        .await;
        if let Err(e) = result {
            update(&status, |s| {
                s.last_error = Some(e.to_string());
                s.state = "连接已释放 · 等待连接".into();
                s.active = None;
                for t in &mut s.topology {
                    if t.device.id != identity.id {
                        t.device.online = false;
                    }
                }
            });
        }
    }
    Ok(())
}
async fn receive_session<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
    source: &str,
    session: u64,
    topo: DeviceTopology,
    status: SharedStatus,
    stop: Arc<AtomicBool>,
    lease_ms: u64,
) -> Result<()> {
    let native = receiver(topo.clone(), lease_ms);
    let mut pending: Option<(u64, Instant)> = None;
    let mut active_epoch = None;
    let mut last_epoch = 0;
    let mut rate_start = Instant::now();
    let mut count = 0u32;
    // Dedicated reader prevents cancellation from corrupting partially read frames.
    let (mut reader, mut writer) = tokio::io::split(stream);
    let (tx, mut rx) = mpsc::channel(256);
    let read = async move {
        loop {
            let m = read_frame(&mut reader).await;
            let failed = m.is_err();
            if tx.send((Instant::now(), m)).await.is_err() || failed {
                break;
            }
        }
    };
    let process = async {
        let mut tick = tokio::time::interval(Duration::from_millis(50));
        loop {
            tokio::select! {
                _=tick.tick()=>{
                    if stop.load(Ordering::Acquire) || native.interrupted.load(Ordering::Acquire){bail!("receiver stopped: local activity, native failure or user pause");}
                    if pending.as_ref().is_some_and(|(_,t)|t.elapsed()>Duration::from_millis(500)){pending=None;}
                },
                incoming=rx.recv()=>{
                    let (queued,m)=incoming.context("connection closed")?;let m=m?;
                    if queued.elapsed()>Duration::from_millis(100){bail!("stale receive queue");}
                    if rate_start.elapsed()>Duration::from_secs(1){rate_start=Instant::now();count=0;}count+=1;if count>6000{bail!("input rate limit exceeded");}
                    match m {
                        Message::Prepare{epoch,topology_version,position}=>{
                            if epoch<=last_epoch || active_epoch.is_some() || topology_version!=topo.version || !topo.monitors.iter().any(|m|m.enabled && m.mirror_of.is_none() && m.rect.contains(position)){bail!("invalid control request");}
                            pending=Some((epoch,Instant::now()));send(&mut writer,&Message::Ready{epoch}).await?;
                        },
                        Message::Activate{epoch,topology_version,position}=>{
                            if !pending.as_ref().is_some_and(|(e,t)|*e==epoch && t.elapsed()<Duration::from_millis(500)) || topology_version!=topo.version || !topo.monitors.iter().any(|m|m.enabled && m.mirror_of.is_none() && m.rect.contains(position)){bail!("activation without current preparation");}
                            let (reply,done)=tokio::sync::oneshot::channel();native.tx.try_send(ReceiverCommand::Activate{source:source.into(),session,epoch,position,reply}).map_err(|_|anyhow::anyhow!("native queue full"))?;
                            timeout(Duration::from_millis(100),done).await???;pending=None;active_epoch=Some(epoch);last_epoch=epoch;
                            update(&status,|s|{s.active=Some(topo.device.id.clone());s.state="接收远端输入 · 本机操作可立即打断".into();});
                            send(&mut writer,&Message::Activated{epoch}).await?;
                        },
                        Message::Release{epoch}=>{
                            if active_epoch.is_some_and(|e|epoch<e){bail!("old release epoch");}
                            let (reply,done)=tokio::sync::oneshot::channel();native.tx.try_send(ReceiverCommand::Release(reply)).map_err(|_|anyhow::anyhow!("native queue full"))?;
                        timeout(Duration::from_millis(100),done).await???;active_epoch=None;pending=None;
                            update(&status,|s|{s.active=None;s.state="已连接 · 等待 Ctrl 切入".into();});send(&mut writer,&Message::Released{epoch}).await?;
                        },
                        Message::Input(packet)=>{
                            if packet.source!=source || packet.session!=session || active_epoch!=Some(packet.epoch){bail!("input without active authenticated control");}
                            native.tx.try_send(ReceiverCommand::Input(queued,packet)).map_err(|_|anyhow::anyhow!("native queue overflow"))?;
                            update(&status,|s|s.received+=1);
                        },
                        Message::Heartbeat{epoch,nonce}=>{
                            if active_epoch==Some(epoch){native.tx.try_send(ReceiverCommand::Heartbeat(source.into(),session,epoch)).map_err(|_|anyhow::anyhow!("native queue overflow"))?;}
                            send(&mut writer,&Message::Pong{nonce,cursor:platform::cursor()?}).await?;
                        },
                        _=>bail!("unexpected message for receiver"),
                    }
                }
            }
        }
    };
    tokio::pin!(read);
    tokio::pin!(process);
    tokio::select! {_=&mut read=>bail!("input connection ended"),r=&mut process=>r}
    // Dropping native joins the independent injection thread, releasing its ledger.
}

type Writer = tokio::io::WriteHalf<tokio_rustls::client::TlsStream<TcpStream>>;
struct Peer {
    writer: Writer,
    topology: DeviceTopology,
    last_pong: Instant,
}
struct Readers(Vec<tokio::task::JoinHandle<()>>);
impl Drop for Readers {
    fn drop(&mut self) {
        for t in &self.0 {
            t.abort();
        }
    }
}
pub async fn source_session(
    identity: Arc<Identity>,
    config: Config,
    status: SharedStatus,
    stop: Arc<AtomicBool>,
) -> Result<()> {
    let trusted = load_trust(&data_dir())?;
    let sid = rand::random::<u64>();
    let local = platform::topology(&identity.id, 1)?;
    let (in_tx, mut incoming) = mpsc::channel(512);
    let mut peers: BTreeMap<String, Peer> = BTreeMap::new();
    let mut readers = Readers(vec![]);
    for endpoint in &config.endpoints {
        let peer = trusted
            .iter()
            .find(|p| p.id == endpoint.peer)
            .context("endpoint has no confirmed certificate")?;
        let connector = TlsConnector::from(identity.client_config(peer)?);
        let socket = timeout(
            Duration::from_secs(3),
            TcpStream::connect(&endpoint.address),
        )
        .await??;
        socket.set_nodelay(true)?;
        let mut stream = timeout(
            Duration::from_secs(3),
            connector.connect("crossdesk.local".try_into()?, socket),
        )
        .await??;
        verify_peer(
            stream.get_ref().1.peer_certificates(),
            std::slice::from_ref(peer),
        )?;
        send(&mut stream, &hello(&identity, sid, local.clone())).await?;
        let (_, topology) = check_hello(
            timeout(Duration::from_secs(2), read_frame(&mut stream)).await??,
            &peer.id,
        )?;
        let (mut reader, writer) = tokio::io::split(stream);
        let tx = in_tx.clone();
        let id = peer.id.clone();
        readers.0.push(tokio::spawn(async move {
            loop {
                let m = read_frame(&mut reader).await;
                let failed = m.is_err();
                if tx.send((id.clone(), Instant::now(), m)).await.is_err() || failed {
                    break;
                }
            }
        }));
        peers.insert(
            peer.id.clone(),
            Peer {
                writer,
                topology,
                last_pong: Instant::now(),
            },
        );
    }
    if peers.is_empty() {
        bail!("add at least one trusted manual IP endpoint");
    }
    let mut topologies = vec![local];
    topologies.extend(peers.values().map(|p| p.topology.clone()));
    let layout = Layout {
        topologies: topologies.clone(),
        links: config.links,
    };
    layout.validate().map_err(anyhow::Error::msg)?;
    if layout.links.is_empty() {
        update(&status, |s| s.topology = topologies);
        bail!("已连接并读取对方显示屏；请先在“显示屏布局”添加往返边界，再启用来源。");
    }
    let mut controller =
        ControlSession::new(identity.id.clone(), layout).map_err(anyhow::Error::msg)?;
    controller.tuning = config.tuning.clone();
    if !platform::physical_idle() {
        bail!("release all physical keys and mouse buttons before enabling the input source");
    }
    let (tx, mut actions) = mpsc::channel(512);
    let native = source(controller, tx);
    update(&status, |s| {
        s.topology = topologies;
        s.source = Some(identity.id.clone());
        s.active = Some(identity.id.clone());
        s.state = "来源已启用 · Ctrl + 边缘切换".into();
    });
    let mut active = identity.id.clone();
    let mut epoch = 0;
    let mut sequence = 0;
    let mut pending: Option<(String, u64)> = None;
    let mut waiting_release: Option<String> = None;
    let start = Instant::now();
    for p in peers.values_mut() {
        p.last_pong = Instant::now();
    }
    let mut tick = tokio::time::interval(Duration::from_millis(config.tuning.heartbeat_ms));
    let result:Result<()>=async{loop{tokio::select! {
        _=tick.tick()=>{
            if stop.load(Ordering::Acquire){break;}
            if native.fault.load(Ordering::Acquire){bail!("native safety watchdog stopped sharing");}
            native.alive.store(native.started.elapsed().as_millis() as u64,Ordering::Release);
            let nonce=start.elapsed().as_millis() as u64;
            for p in peers.values_mut(){if p.last_pong.elapsed()>Duration::from_millis(750){bail!("peer heartbeat expired; returned to local input");}send(&mut p.writer,&Message::Heartbeat{epoch,nonce}).await?;}
        },
        event=actions.recv()=>{
            let (queued,batch)=event.context("native input thread stopped")?;
            if queued.elapsed()>Duration::from_millis(100){bail!("stale source queue");}
            for action in batch {match action {
                Action::Prepare{target,epoch:e,position}=>{
                    pending=Some((target.clone(),e));
                    if target==identity.id {
                        if active!=identity.id{send(&mut peers.get_mut(&active).context("old target missing")?.writer,&Message::Release{epoch}).await?;waiting_release=Some(active.clone());}else{native.commands.try_send(SourceCommand::Ready(target,e))?;}
                    }else{
                        let p=peers.get_mut(&target).context("target not connected")?;
                        send(&mut p.writer,&Message::Prepare{epoch:e,topology_version:p.topology.version,position}).await?;
                    }
                },
                Action::Activate{target,epoch:e,position}=>{
                    epoch=e;active=target.clone();sequence=0;pending=None;
                    if target==identity.id {platform::inject(&InputEvent::Move{position,topology_version:1})?;}else{let p=peers.get_mut(&target).context("target missing")?;send(&mut p.writer,&Message::Activate{epoch,topology_version:p.topology.version,position}).await?;}
                    update(&status,|s|s.active=Some(target));
                },
                Action::Deliver{target,event} if target!=identity.id=>{sequence+=1;let p=peers.get_mut(&target).context("target missing")?;send(&mut p.writer,&Message::Input(InputPacket{source:identity.id.clone(),session:sid,epoch,sequence,event})).await?;},
                Action::Recovered=>bail!("safety pause: emergency shortcut, topology change, timeout or queue overload"),
                _=>{},
            }}
        },
        incoming=incoming.recv()=>{
            let (id,queued,m)=incoming.context("all connections ended")?;if queued.elapsed()>Duration::from_millis(100){bail!("stale network control queue");}
            match m? {
                Message::Ready{epoch:e} if pending.as_ref().is_some_and(|(t,k)|t==&id && *k==e)=>{
                    if active!=identity.id{send(&mut peers.get_mut(&active).unwrap().writer,&Message::Release{epoch}).await?;waiting_release=Some(active.clone());}else{native.commands.try_send(SourceCommand::Ready(id,e))?;}
                },
                Message::Released{..} if waiting_release.as_ref()==Some(&id)=>{waiting_release=None;if let Some((target,e))=&pending{native.commands.try_send(SourceCommand::Ready(target.clone(),*e))?;}},
                Message::Activated{epoch:e} if id==active && e==epoch=>{},
                Message::Pong{nonce,cursor}=>{peers.get_mut(&id).context("unknown pong identity")?.last_pong=Instant::now();update(&status,|s|s.rtt_ms=Some((start.elapsed().as_millis() as u64).saturating_sub(nonce)));if id==active{native.commands.try_send(SourceCommand::Cursor(cursor))?;}},
                Message::Error{reason}=>bail!("remote failure: {reason}"),
                _=>bail!("unexpected control response"),
            }
        }
    }}Ok(())}.await;
    // Fail local first; remote cleanup is also guaranteed by its independent lease.
    drop(native);
    for p in peers.values_mut() {
        let _ = send(&mut p.writer, &Message::Release { epoch }).await;
    }
    result
}
