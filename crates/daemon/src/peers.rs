//! Persistent, mutually authenticated peer presence and exclusive control.
//! All socket I/O is owned by bounded worker tasks. Only one native capture
//! handle exists per process: local source OR remote receiver, never both.
use crate::{config::Config, native::*, status::*};
use anyhow::{bail, ensure, Context, Result};
use crossdesk_core::{
    ownership::{Claim, Effect, Ownership},
    *,
};
use crossdesk_platform as platform;
use crossdesk_transport::*;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
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

struct Tasks(Vec<tokio::task::JoinHandle<()>>);
impl Drop for Tasks {
    fn drop(&mut self) {
        for t in &self.0 {
            t.abort();
        }
    }
}
struct Peer {
    sharing: bool,
    generation: u64,
    preferred: bool,
    topology: DeviceTopology,
    pair_layout: PairLayout,
    settings: SharedSettings,
    tx: mpsc::Sender<(Instant, Message)>,
    last_seen: Instant,
    pings: Pings,
    _tasks: Tasks,
}
#[derive(Default)]
struct Pings(VecDeque<(u64, Instant)>);
impl Pings {
    fn sent(&mut self, nonce: u64) {
        if self.0.len() == 32 {
            self.0.pop_front();
        }
        self.0.push_back((nonce, Instant::now()));
    }
    fn accept(&mut self, nonce: u64, cursor: Point, topology: &DeviceTopology) -> Result<u64> {
        ensure!(
            topology
                .monitors
                .iter()
                .any(|m| m.enabled && m.mirror_of.is_none() && m.rect.contains(cursor)),
            "无效远端光标反馈"
        );
        let index = self
            .0
            .iter()
            .position(|(n, _)| *n == nonce)
            .context("心跳反馈没有对应请求，或已经处理")?;
        let (_, sent) = self.0.remove(index).unwrap();
        Ok(sent.elapsed().as_millis() as u64)
    }
}

fn release_matches(epoch: u64, active: Option<u64>, prepared: Option<u64>) -> bool {
    active == Some(epoch) || (active.is_none() && prepared == Some(epoch))
}

// Cancel and input travel in opposite TLS directions. The peer can have already
// sent input when it receives Cancel. Drain only that known retired control until
// its ordered Cancelled acknowledgement; never pass these packets to native input.
struct CancelledControl {
    claim: Claim,
    outgoing_epochs: Vec<u64>,
}
impl CancelledControl {
    fn discards(&self, local: &str, peer: &str, message: &Message) -> bool {
        match message {
            Message::Input(p) => {
                self.claim.origin == peer && p.source == peer && p.session == self.claim.nonce
            }
            Message::Prepare { .. } | Message::Activate { .. } | Message::Release { .. } => {
                self.claim.origin == peer
            }
            Message::Ready { epoch }
            | Message::Activated { epoch }
            | Message::Released { epoch } => {
                self.claim.origin == local && self.outgoing_epochs.contains(epoch)
            }
            _ => false,
        }
    }
}
enum Event {
    Connected(String, Peer),
    Message(String, u64, Instant, Result<Message>),
    Failed(String, String),
}
async fn send<W: AsyncWrite + Unpin>(w: &mut W, m: &Message) -> Result<()> {
    timeout(Duration::from_millis(250), write_frame(w, m))
        .await
        .context("发送超时：局域网拥堵，已停止此次控制")??;
    Ok(())
}
#[allow(clippy::too_many_arguments)]
async fn establish<S: AsyncRead + AsyncWrite + Unpin + Send + 'static>(
    mut stream: S,
    id: Arc<Identity>,
    peer: TrustedPeer,
    status: SharedStatus,
    tx: mpsc::Sender<Event>,
    outbound: bool,
) -> Result<()> {
    let session = rand::random::<u64>();
    let local = status
        .lock()
        .unwrap()
        .topology
        .iter()
        .find(|t| t.device.id == id.id)
        .cloned()
        .context("缺少本机显示屏")?;
    let sharing = status.lock().unwrap().config.sharing;
    send(
        &mut stream,
        &Message::Hello {
            version: VERSION,
            sharing,
            source: id.id.clone(),
            session,
            topology: local,
        },
    )
    .await?;
    let message = timeout(Duration::from_secs(5), read_frame(&mut stream))
        .await
        .context("交换屏幕信息超时，请将两端都更新到相同版本")??;
    let Message::Hello {
        version,
        sharing,
        source,
        mut topology,
        ..
    } = message
    else {
        bail!("对方协议不兼容，请更新两端程序");
    };
    ensure!(
        version == VERSION,
        "连接版本不同，请将两台电脑都更新到相同候选版本"
    );
    ensure!(
        source == peer.id && topology.device.id == peer.id,
        "对方身份与已保存的配对不匹配"
    );
    topology.device.online = true;
    topology.device.authorized = true;
    Layout {
        topologies: vec![topology.clone()],
        links: vec![],
    }
    .validate()
    .map_err(anyhow::Error::msg)?;
    let layout = crate::layout_sync::snapshot(&status.lock().unwrap().config, &id.id, &peer.id);
    send(&mut stream, &Message::PairLayout(layout)).await?;
    let Message::PairLayout(pair_layout) = timeout(Duration::from_secs(5), read_frame(&mut stream))
        .await
        .context("同步布局超时")??
    else {
        bail!("未收到配对布局");
    };
    let settings = crate::settings_sync::snapshot(&status.lock().unwrap().config);
    send(&mut stream, &Message::Settings(settings)).await?;
    let Message::Settings(settings) = timeout(Duration::from_secs(5), read_frame(&mut stream))
        .await
        .context("同步设置超时")??
    else {
        bail!("未收到共享设置");
    };
    let lease_ms = status.lock().unwrap().config.tuning.lease_ms;
    let generation = rand::random::<u64>();
    let (mut reader, mut writer) = tokio::io::split(stream);
    let (out, mut messages) = mpsc::channel::<(Instant, Message)>(512);
    let incoming = tx.clone();
    let name = peer.id.clone();
    let reader_task = tokio::spawn(async move {
        loop {
            let result = read_frame(&mut reader).await;
            let failed = result.is_err();
            if incoming
                .send(Event::Message(
                    name.clone(),
                    generation,
                    Instant::now(),
                    result,
                ))
                .await
                .is_err()
                || failed
            {
                break;
            }
        }
    });
    let incoming = tx.clone();
    let name = peer.id.clone();
    let writer_task = tokio::spawn(async move {
        while let Some((queued, message)) = messages.recv().await {
            let result = if matches!(message, Message::Input(_))
                && queued.elapsed() >= Duration::from_millis(lease_ms)
            {
                Err(anyhow::anyhow!("发送停顿超过控制期限，已释放控制"))
            } else {
                send(&mut writer, &message).await
            };
            if let Err(e) = result {
                let _ = incoming
                    .send(Event::Message(
                        name.clone(),
                        generation,
                        Instant::now(),
                        Err(e),
                    ))
                    .await;
                break;
            }
        }
    });
    tx.send(Event::Connected(
        peer.id.clone(),
        Peer {
            sharing,
            generation,
            preferred: outbound == (id.id < peer.id),
            topology,
            pair_layout,
            settings,
            tx: out,
            last_seen: Instant::now(),
            pings: Pings::default(),
            _tasks: Tasks(vec![reader_task, writer_task]),
        },
    ))
    .await
    .map_err(|_| anyhow::anyhow!("后台已停止"))?;
    Ok(())
}

pub fn default_links(topologies: &[DeviceTopology]) -> Vec<EdgeLink> {
    let mut ordered: Vec<_> = topologies.iter().filter(|t| t.device.online).collect();
    ordered.sort_by_key(|t| &t.device.id);
    let edge = |t: &DeviceTopology, right: bool| -> Option<EdgeEnd> {
        let m = t
            .monitors
            .iter()
            .filter(|m| m.enabled && m.mirror_of.is_none())
            .min_by(|a, b| {
                let value = |m: &Monitor| {
                    if right {
                        -(m.rect.x + m.rect.width)
                    } else {
                        m.rect.x
                    }
                };
                value(a).total_cmp(&value(b)).then(a.id.cmp(&b.id))
            })?;
        Some(EdgeEnd {
            device: t.device.id.clone(),
            monitor: m.id.clone(),
            edge: if right { Edge::Right } else { Edge::Left },
            start: 0.,
            end: 1.,
        })
    };
    let mut links = vec![];
    for pair in ordered.windows(2) {
        if let (Some(a), Some(b)) = (edge(pair[0], true), edge(pair[1], false)) {
            links.push(EdgeLink {
                from: a.clone(),
                to: b.clone(),
                enabled: true,
                require_ctrl: true,
            });
            links.push(EdgeLink {
                from: b,
                to: a,
                enabled: true,
                require_ctrl: true,
            });
        }
    }
    links
}

pub struct Configure {
    pub config: Config,
    pub done: tokio::sync::oneshot::Sender<std::result::Result<(), String>>,
}

struct Manager {
    id: String,
    config: Config,
    status: SharedStatus,
    peers: BTreeMap<String, Peer>,
    source: Option<SourceHandle>,
    receiver: Option<ReceiverHandle>,
    actions: mpsc::Receiver<(Instant, Vec<Action>)>,
    ownership: Ownership,
    cancelled: BTreeMap<String, CancelledControl>,
    pending: Option<(String, u64, Point)>,
    active: String,
    epoch: u64,
    sequence: u64,
    waiting_release: Option<String>,
    incoming_epoch: Option<u64>,
    prepared_epoch: Option<u64>,
    observe_only: bool,
    started: Instant,
    renewal: u64,
    retry_after: Instant,
    restart_requested: bool,
}
impl Manager {
    fn apply_tuning(&mut self, next: &Config) -> Result<()> {
        let mut without_speed = next.tuning.clone();
        without_speed.mouse_speed = self.config.tuning.mouse_speed;
        if without_speed != self.config.tuning
            || next.links != self.config.links
            || next.same_subnet_only != self.config.same_subnet_only
        {
            self.cancel()?;
        }
        if let Some(source) = &self.source {
            source
                .mouse_speed
                .store(next.tuning.mouse_speed.to_bits(), Ordering::Release);
        }
        Ok(())
    }
    fn configure(&mut self, mut next: Config) -> Result<()> {
        next.sharing = self.config.sharing;
        next.validate()?;
        Layout {
            topologies: self.status.lock().unwrap().topology.clone(),
            links: next.links.clone(),
        }
        .validate()
        .map_err(anyhow::Error::msg)?;
        let peers = self
            .status
            .lock()
            .unwrap()
            .trusted
            .iter()
            .map(|p| p.id.clone())
            .collect::<Vec<_>>();
        crate::layout_sync::record_changes(&mut next, &self.config, &self.id, &peers)?;
        crate::settings_sync::record_changes(&mut next, &self.config, &self.id)?;
        let old = self.config.clone();
        next.save()?;
        self.apply_tuning(&next)?;
        self.config = next.clone();
        update(&self.status, |s| {
            s.config = next;
            s.last_error = None;
        });
        for id in self.peers.keys() {
            if old.links != self.config.links {
                self.send(
                    id,
                    Message::PairLayout(crate::layout_sync::snapshot(&self.config, &self.id, id)),
                )?;
            }
            if crate::settings_sync::snapshot(&old) != crate::settings_sync::snapshot(&self.config)
            {
                self.send(
                    id,
                    Message::Settings(crate::settings_sync::snapshot(&self.config)),
                )?;
            }
        }
        self.publish();
        Ok(())
    }
    fn send(&self, id: &str, m: Message) -> Result<()> {
        self.send_at(id, Instant::now(), m)
    }
    fn send_at(&self, id: &str, queued: Instant, m: Message) -> Result<()> {
        self.peers
            .get(id)
            .context("对方已断开")?
            .tx
            .try_send((queued, m))
            .map_err(|_| anyhow::anyhow!("发送队列已满，已安全释放控制"))
    }
    fn now(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }
    fn neighbors(&self) -> BTreeSet<String> {
        self.peers
            .iter()
            .filter(|(_, p)| p.sharing)
            .map(|(id, _)| id.clone())
            .collect()
    }
    fn layout(&self) -> Layout {
        let local = self
            .status
            .lock()
            .unwrap()
            .topology
            .iter()
            .find(|t| t.device.id == self.id)
            .cloned()
            .unwrap();
        let mut topologies = vec![local];
        topologies.extend(self.peers.values().map(|p| p.topology.clone()));
        let links = if self.config.links.is_empty() {
            default_links(&topologies)
        } else {
            self.config
                .links
                .iter()
                .filter(|l| {
                    topologies.iter().any(|t| t.device.id == l.from.device)
                        && topologies.iter().any(|t| t.device.id == l.to.device)
                })
                .cloned()
                .collect()
        };
        Layout { topologies, links }
    }
    fn publish(&self) {
        let layout = self.layout();
        update(&self.status, |s| {
            s.peer_sharing = self
                .peers
                .iter()
                .map(|(id, p)| (id.clone(), p.sharing))
                .collect();
            for t in &mut s.topology {
                if t.device.id != self.id {
                    t.device.online = false;
                }
            }
            for t in &layout.topologies {
                s.topology.retain(|old| old.device.id != t.device.id);
                s.topology.push(t.clone());
            }
            s.effective_links = layout.links;
            s.topology.sort_by(|a, b| {
                (a.device.id != self.id)
                    .cmp(&(b.device.id != self.id))
                    .then(a.device.id.cmp(&b.device.id))
            });
            s.source = self.ownership.claim().map(|c| c.origin.clone());
            s.active = if self.receiver.is_some() && self.incoming_epoch.is_some() {
                Some(self.id.clone())
            } else if self.active != self.id {
                Some(self.active.clone())
            } else {
                None
            };
            s.state = if self.observe_only {
                "连接测试模式 · 不捕获、不模拟任何输入".into()
            } else if !self.config.sharing {
                "已暂停键鼠共享 · 屏幕连接保持在线".into()
            } else if self.peers.is_empty() {
                "共享已开启 · 正在等待已配对电脑上线".into()
            } else if self.incoming_epoch.is_some() {
                "正在接受远端控制 · 操作本机键鼠可收回控制".into()
            } else if self.active != self.id {
                format!(
                    "正在控制对方 · {}返回 · Ctrl + Alt + {} 紧急释放",
                    self.config.tuning.switch_modifier.label(),
                    if self.config.tuning.emergency_key == PhysicalKey(0x45) {
                        "F12"
                    } else {
                        "Esc"
                    }
                )
            } else {
                format!(
                    "双向共享已就绪 · {}",
                    self.config.tuning.switch_modifier.label()
                )
            };
        });
    }
    fn reset_native(&mut self) {
        // Drop waits for cleanup; an unconfirmed worker latches out new input owners.
        self.source.take();
        self.receiver.take();
        while self.actions.try_recv().is_ok() {}
        self.active = self.id.clone();
        self.pending = None;
        self.waiting_release = None;
        self.incoming_epoch = None;
        self.prepared_epoch = None;
        self.retry_after = Instant::now() + Duration::from_millis(200);
    }
    fn effects(&mut self, effects: Vec<Effect>) -> Result<()> {
        for effect in effects {
            match effect {
                Effect::Request(id, c) => self.send(&id, Message::Claim(c))?,
                Effect::Grant(id, c) => self.send(&id, Message::Granted(c))?,
                Effect::Deny(id, c) => self.send(&id, Message::Denied(c))?,
                Effect::Cancel(id, c) => {
                    let _ = self.send(&id, Message::Cancel(c));
                }
                Effect::Released => {
                    self.reset_native();
                }
                Effect::Acquired => {
                    self.prepare_target()?;
                }
            }
        }
        self.publish();
        Ok(())
    }
    fn cancel(&mut self) -> Result<()> {
        if let Some(c) = self.ownership.claim().cloned() {
            let effects = self.ownership.cancel(&c);
            self.remember_cancelled(&c, &effects);
            self.effects(effects)?;
        } else {
            self.reset_native();
        }
        self.publish();
        Ok(())
    }
    // Local input can stop the native worker between the manager tick, enqueue,
    // and acknowledgement. Treat that as a normal handoff in all three places.
    fn receiver_result<T>(&mut self, result: Result<T>) -> Result<Option<T>> {
        if self
            .receiver
            .as_ref()
            .is_some_and(|n| n.interrupted.load(Ordering::Acquire))
        {
            self.cancel()?;
            Ok(None)
        } else {
            result.map(Some)
        }
    }
    fn receiver_command(&mut self, command: ReceiverCommand) -> Result<bool> {
        let result = self
            .receiver
            .as_ref()
            .context("接收后台已停止")?
            .tx
            .try_send(command)
            .map_err(|_| anyhow::anyhow!("接收队列不可用"));
        Ok(self.receiver_result(result)?.is_some())
    }
    fn remember_cancelled(&mut self, claim: &Claim, effects: &[Effect]) {
        for peer in effects.iter().filter_map(|e| match e {
            Effect::Cancel(peer, _) => Some(peer),
            _ => None,
        }) {
            let mut outgoing_epochs = vec![];
            if &self.active == peer {
                outgoing_epochs.push(self.epoch);
            }
            if let Some((target, epoch, _)) = &self.pending {
                if target == peer {
                    outgoing_epochs.push(*epoch);
                }
            }
            self.cancelled.insert(
                peer.clone(),
                CancelledControl {
                    claim: claim.clone(),
                    outgoing_epochs,
                },
            );
        }
    }
    fn prepare_target(&mut self) -> Result<()> {
        let Some((target, epoch, position)) = self.pending.clone() else {
            return Ok(());
        };
        if target == self.id {
            self.release_old()?;
        } else {
            self.send(
                &target,
                Message::Prepare {
                    epoch,
                    topology_version: self
                        .peers
                        .get(&target)
                        .context("目标离线")?
                        .topology
                        .version,
                    position,
                },
            )?;
        }
        Ok(())
    }
    fn reserve_pending(&mut self) -> Result<()> {
        if self.pending.is_none() || self.ownership.claim().is_some() || !self.cancelled.is_empty()
        {
            return Ok(());
        }
        let effects = self.ownership.begin(
            Claim {
                origin: self.id.clone(),
                nonce: rand::random(),
            },
            self.neighbors(),
            self.now(),
        );
        self.effects(effects)
    }
    fn release_old(&mut self) -> Result<()> {
        if self.active != self.id {
            self.send(&self.active, Message::Release { epoch: self.epoch })?;
            self.waiting_release = Some(self.active.clone());
        } else {
            self.ready()?;
        }
        Ok(())
    }
    fn ready(&self) -> Result<()> {
        if let (Some(n), Some((target, epoch, _))) = (&self.source, &self.pending) {
            n.commands
                .try_send(SourceCommand::Ready(target.clone(), *epoch))?;
        }
        Ok(())
    }
    fn start_source(&mut self) -> Result<()> {
        if self.source.is_some()
            || self.receiver.is_some()
            || self.ownership.claim().is_some()
            || !self.cancelled.is_empty()
            || !self.config.sharing
            || self.observe_only
            || self.peers.is_empty()
            || Instant::now() < self.retry_after
            || !platform::physical_idle()
        {
            return Ok(());
        }
        crate::native::ensure_cleanup_confirmed()?;
        let mut layout = self.layout();
        for topology in &mut layout.topologies {
            if self
                .peers
                .get(&topology.device.id)
                .is_some_and(|p| !p.sharing)
            {
                topology.device.online = false;
            }
        }
        let mut controller =
            ControlSession::new(self.id.clone(), layout).map_err(anyhow::Error::msg)?;
        controller.tuning = self.config.tuning.clone();
        controller.prepare_timeout_ms = 1500;
        let (tx, rx) = mpsc::channel(512);
        self.actions = rx;
        self.source = Some(source(controller, tx));
        Ok(())
    }
    async fn message(&mut self, id: &str, queued: Instant, m: Message) -> Result<()> {
        self.receiver_result(Ok(()))?;
        if self
            .cancelled
            .get(id)
            .is_some_and(|c| c.discards(&self.id, id, &m))
        {
            return Ok(());
        }
        // Network presence is not input. Only input/control events have the short age bound.
        if matches!(m, Message::Input(_))
            && queued.elapsed() >= Duration::from_millis(self.config.tuning.lease_ms)
        {
            bail!("过期输入或切换请求已丢弃");
        }
        match m {
            Message::PairLayout(incoming) => {
                let topologies = self.status.lock().unwrap().topology.clone();
                if let Some(next) =
                    crate::layout_sync::merge(&self.config, &self.id, id, &incoming, topologies)?
                {
                    next.save()?;
                    self.apply_tuning(&next)?;
                    self.config = next.clone();
                    update(&self.status, |s| s.config = next);
                    self.publish();
                }
            }
            Message::Settings(incoming) => {
                if let Some(next) = crate::settings_sync::merge(&self.config, &incoming)? {
                    next.save()?;
                    self.apply_tuning(&next)?;
                    self.restart_requested = next.same_subnet_only != self.config.same_subnet_only
                        || next.tuning.lease_ms != self.config.tuning.lease_ms;
                    self.config = next.clone();
                    update(&self.status, |s| s.config = next);
                    for peer in self.peers.keys().filter(|peer| peer.as_str() != id) {
                        self.send(peer, Message::Settings(incoming.clone()))?;
                    }
                    self.publish();
                }
            }
            Message::Claim(c) => {
                ensure!(c.origin.len() <= 128, "无效控制请求");
                // A new request on this ordered connection follows the old traffic.
                self.cancelled.remove(id);
                if !self.config.sharing
                    || !self.peers[id].sharing
                    || self.observe_only
                    || !self.cancelled.is_empty()
                    || (self.ownership.claim().is_none() && !platform::physical_idle())
                {
                    self.send(id, Message::Denied(c))?;
                    return Ok(());
                }
                if self.ownership.claim().is_none() {
                    self.reset_native();
                }
                let effects = self.ownership.request(id, c, self.neighbors(), self.now());
                self.effects(effects)?;
            }
            Message::Granted(c) => {
                let effects = self.ownership.granted(id, &c);
                self.effects(effects)?;
            }
            Message::Denied(c) => {
                if self.ownership.claim() == Some(&c) {
                    self.cancel()?;
                }
            }
            Message::Cancel(c) => {
                if self.ownership.claim() == Some(&c) {
                    self.cancel()?;
                }
                self.send(id, Message::Cancelled(c))?;
            }
            Message::Cancelled(c) => {
                if self.cancelled.get(id).is_some_and(|old| old.claim == c) {
                    self.cancelled.remove(id);
                    self.reserve_pending()?;
                }
            }
            Message::Renew { claim, sequence } => {
                ensure!(
                    queued.elapsed() < Duration::from_millis(1200),
                    "过期控制续租已拒绝"
                );
                for peer in self.ownership.renew(Some(id), &claim, sequence, self.now()) {
                    self.send(
                        &peer,
                        Message::Renew {
                            claim: claim.clone(),
                            sequence,
                        },
                    )?;
                }
            }
            Message::Prepare {
                epoch,
                topology_version,
                position,
            } => {
                ensure!(
                    !self.observe_only
                        && self.config.sharing
                        && self.ownership.claim().is_some_and(|c| c.origin == id)
                        && self.source.is_none()
                        && self.incoming_epoch.is_none(),
                    "尚未获得独占控制权"
                );
                let local = self.layout().topology(&self.id).cloned().unwrap();
                ensure!(
                    topology_version == local.version
                        && local.monitors.iter().any(|m| m.enabled
                            && m.mirror_of.is_none()
                            && m.rect.contains(position)),
                    "屏幕布局已变化，请刷新后重试"
                );
                self.receiver.take();
                crate::native::ensure_cleanup_confirmed()?;
                let n = receiver(local, self.config.tuning.lease_ms);
                // Preparation covers cold native/guardian startup. Never acknowledge a non-ready receiver.
                let wait = Instant::now();
                while !n.ready.load(Ordering::Acquire)
                    && !n.interrupted.load(Ordering::Acquire)
                    && wait.elapsed() < Duration::from_millis(350)
                {
                    tokio::time::sleep(Duration::from_millis(4)).await;
                }
                ensure!(
                    n.ready.load(Ordering::Acquire) && !n.interrupted.load(Ordering::Acquire),
                    "原生输入后台未就绪，请松开按键后重试"
                );
                self.receiver = Some(n);
                self.prepared_epoch = Some(epoch);
                self.send(id, Message::Ready { epoch })?;
            }
            Message::Activate {
                epoch,
                topology_version,
                position,
            } => {
                ensure!(
                    self.ownership.claim().is_some_and(|c| c.origin == id)
                        && self.prepared_epoch == Some(epoch),
                    "没有对应的控制准备请求"
                );
                let local = self.layout().topology(&self.id).cloned().unwrap();
                ensure!(
                    topology_version == local.version
                        && local.monitors.iter().any(|m| m.enabled
                            && m.mirror_of.is_none()
                            && m.rect.contains(position)),
                    "无效目标位置"
                );
                let (reply, done) = tokio::sync::oneshot::channel();
                if !self.receiver_command(ReceiverCommand::Activate {
                    source: id.into(),
                    session: self.ownership.claim().unwrap().nonce,
                    epoch,
                    position,
                    reply,
                })? {
                    return Ok(());
                }
                let result = timeout(Duration::from_millis(200), done)
                    .await
                    .context("激活远端输入超时，已释放控制")
                    .and_then(|r| r.context("激活确认已结束"))
                    .and_then(|r| r);
                if self.receiver_result(result)?.is_none() {
                    return Ok(());
                }
                self.incoming_epoch = Some(epoch);
                self.prepared_epoch = None;
                self.send(id, Message::Activated { epoch })?;
                self.publish();
            }
            Message::Input(packet) => {
                ensure!(
                    self.ownership
                        .claim()
                        .is_some_and(|c| c.origin == id && c.nonce == packet.session)
                        && packet.source == id
                        && self.incoming_epoch == Some(packet.epoch),
                    "未授权或过期的输入"
                );
                if self.receiver_command(ReceiverCommand::Input(queued, packet))? {
                    update(&self.status, |s| s.received += 1);
                }
            }
            Message::Release { epoch } => {
                ensure!(
                    self.ownership.claim().is_some_and(|c| c.origin == id)
                        && release_matches(epoch, self.incoming_epoch, self.prepared_epoch),
                    "释放请求与控制者或控制代际不符"
                );
                if self.receiver.is_some() {
                    let (reply, done) = tokio::sync::oneshot::channel();
                    if !self.receiver_command(ReceiverCommand::Release(reply))? {
                        return Ok(());
                    }
                    let result = timeout(Duration::from_millis(200), done)
                        .await
                        .context("释放输入超时")
                        .and_then(|r| r.context("释放确认已结束"))
                        .and_then(|r| r);
                    if self.receiver_result(result)?.is_none() {
                        return Ok(());
                    }
                }
                self.receiver.take();
                self.incoming_epoch = None;
                self.prepared_epoch = None;
                self.send(id, Message::Released { epoch })?;
                self.publish();
            }
            Message::Ready { epoch }
                if self
                    .pending
                    .as_ref()
                    .is_some_and(|(t, e, _)| t == id && *e == epoch) =>
            {
                self.release_old()?;
            }
            Message::Released { epoch }
                if self.waiting_release.as_deref() == Some(id) && self.epoch == epoch =>
            {
                self.waiting_release = None;
                self.ready()?;
            }
            Message::Activated { epoch } if self.active == id && self.epoch == epoch => {}
            Message::Heartbeat { epoch, nonce } => {
                ensure!(
                    queued.elapsed() < Duration::from_millis(self.config.tuning.lease_ms),
                    "过期心跳不能续租"
                );
                if self.incoming_epoch == Some(epoch)
                    && self.ownership.claim().is_some_and(|c| c.origin == id)
                    && self.receiver.is_some()
                    && !self.receiver_command(ReceiverCommand::Heartbeat(
                        id.into(),
                        self.ownership.claim().unwrap().nonce,
                        epoch,
                    ))?
                {
                    return Ok(());
                }
                self.send(
                    id,
                    Message::Pong {
                        nonce,
                        cursor: platform::cursor()?,
                    },
                )?;
            }
            Message::Pong { nonce, cursor } => {
                let peer = self.peers.get_mut(id).context("未知心跳来源")?;
                let rtt = peer.pings.accept(nonce, cursor, &peer.topology)?;
                update(&self.status, |s| s.rtt_ms = Some(rtt));
                if self.active == id {
                    if let Some(n) = &self.source {
                        n.commands.try_send(SourceCommand::Cursor(cursor))?;
                    }
                }
            }
            Message::Error { reason } => {
                bail!("对方已释放控制：{reason}");
            }
            _ => bail!("收到不属于当前会话的控制消息"),
        }
        Ok(())
    }
    fn actions(&mut self, queued: Instant, batch: Vec<Action>) -> Result<()> {
        let actions = crate::outbound_delivery::prepare_actions(
            (queued, batch),
            || self.actions.try_recv().ok(),
            Instant::now(),
            self.config.tuning.lease_ms,
        )?;
        for (queued, action) in actions {
            match action {
                Action::Prepare {
                    target,
                    epoch,
                    position,
                } => {
                    self.pending = Some((target, epoch, position));
                    if self.ownership.claim().is_none() {
                        self.reserve_pending()?;
                    } else {
                        self.prepare_target()?;
                    }
                }
                Action::Activate {
                    target,
                    epoch,
                    position,
                } => {
                    crate::native::ensure_cleanup_confirmed()?;
                    self.active = target.clone();
                    self.epoch = epoch;
                    self.sequence = 0;
                    self.pending = None;
                    if target == self.id {
                        platform::inject(&InputEvent::Move {
                            position,
                            topology_version: self.layout().topology(&self.id).unwrap().version,
                        })?;
                        // Keep this source session alive to consume the switching Ctrl-up.
                        if let Some(c) = self.ownership.claim().cloned() {
                            let effects = self.ownership.cancel(&c);
                            self.remember_cancelled(&c, &effects);
                            for effect in effects {
                                if let Effect::Cancel(id, c) = effect {
                                    let _ = self.send(&id, Message::Cancel(c));
                                }
                            }
                        }
                    } else {
                        self.send(
                            &target,
                            Message::Activate {
                                epoch,
                                topology_version: self.peers[&target].topology.version,
                                position,
                            },
                        )?;
                    }
                    self.publish();
                }
                Action::Deliver { target, event } if target != self.id => {
                    self.sequence += 1;
                    let session = self.ownership.claim().context("控制权已失效")?.nonce;
                    self.send_at(
                        &target,
                        queued,
                        Message::Input(InputPacket {
                            source: self.id.clone(),
                            session,
                            epoch: self.epoch,
                            sequence: self.sequence,
                            event,
                        }),
                    )?;
                }
                Action::Recovered => bail!(
                    "已安全暂停：紧急释放、输入后台异常或切换超时。松开按键后可点击恢复共享。"
                ),
                _ => {}
            }
        }
        Ok(())
    }
}

pub async fn run(
    identity: Arc<Identity>,
    config: Config,
    status: SharedStatus,
    stop: Arc<AtomicBool>,
    observe_only: bool,
    mut updates: mpsc::Receiver<Configure>,
) -> Result<()> {
    let trusted = load_trust(&data_dir())?;
    let listener = TcpListener::bind((std::net::Ipv4Addr::UNSPECIFIED, config.port))
        .await
        .with_context(|| format!("无法监听端口 {}，请关闭重复运行的 CrossDesk", config.port))?;
    let acceptor = TlsAcceptor::from(identity.server_config(&trusted)?);
    let (tx, mut rx) = mpsc::channel(512);
    let mut tasks = Tasks(vec![]);
    let sender = tx.clone();
    let id = identity.clone();
    let s = status.clone();
    let trust = trusted.clone();
    tasks.0.push(tokio::spawn(async move {
        let limit=Arc::new(tokio::sync::Semaphore::new(8));
        let mut handshakes=tokio::task::JoinSet::new();
        loop {
            tokio::select! {
                accepted=listener.accept()=>{
                    let Ok((socket,address))=accepted else{break;};
                    if crate::network_policy::check(address.ip(),s.lock().unwrap().config.same_subnet_only).is_err(){continue;}
                    let Ok(permit)=limit.clone().try_acquire_owned() else{continue;};
                    let acceptor=acceptor.clone();let id=id.clone();let s=s.clone();let tx=sender.clone();let trust=trust.clone();
                    handshakes.spawn(async move {
                        let _permit=permit;
                        let result:Result<()>=async {socket.set_nodelay(true)?;let stream=timeout(Duration::from_secs(5),acceptor.accept(socket)).await.context("TLS 认证超时")??;
                            let peer=verify_peer(stream.get_ref().1.peer_certificates(),&trust)?.clone();establish(stream,id,peer,s,tx,false).await
                        }.await;
                        // Unauthenticated clients cannot overwrite the user's connection state.
                        let _=result;
                    });
                },
                _=handshakes.join_next(),if !handshakes.is_empty()=>{},
            }
        }
    }));
    for peer in trusted {
        let tx = tx.clone();
        let id = identity.clone();
        let s = status.clone();
        let configured = config
            .endpoints
            .iter()
            .find(|e| e.peer == peer.id)
            .map(|e| e.address.clone());
        tasks.0.push(tokio::spawn(async move {
            let mut timer = tokio::time::interval(Duration::from_secs(2));
            loop {
                timer.tick().await;
                let address = {
                    let snapshot = s.lock().unwrap();
                    if snapshot
                        .topology
                        .iter()
                        .any(|t| t.device.id == peer.id && t.device.online)
                    {
                        continue;
                    }
                    snapshot
                        .discovered
                        .iter()
                        .find(|p| p.id == peer.id)
                        .and_then(|p| p.addresses.first().cloned())
                        .or_else(|| configured.clone())
                };
                let Some(address) = address else {
                    continue;
                };
                let result: Result<()> = async {
                    let endpoint: std::net::SocketAddr = address.parse()?;
                    crate::network_policy::check(
                        endpoint.ip(),
                        s.lock().unwrap().config.same_subnet_only,
                    )?;
                    let socket = timeout(Duration::from_secs(3), TcpStream::connect(&address))
                        .await
                        .context("连接超时，请检查对方是否运行、防火墙是否允许专用网络")??;
                    socket.set_nodelay(true)?;
                    let connector = TlsConnector::from(id.client_config(&peer)?);
                    let stream = timeout(
                        Duration::from_secs(5),
                        connector.connect("crossdesk.local".try_into()?, socket),
                    )
                    .await
                    .context("TLS 认证超时，请更新两端程序")??;
                    verify_peer(
                        stream.get_ref().1.peer_certificates(),
                        std::slice::from_ref(&peer),
                    )?;
                    establish(
                        stream,
                        id.clone(),
                        peer.clone(),
                        s.clone(),
                        tx.clone(),
                        true,
                    )
                    .await
                }
                .await;
                if let Err(e) = result {
                    if tx
                        .send(Event::Failed(peer.id.clone(), format!("{e:#}")))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
            }
        }));
    }
    let (_, actions) = mpsc::channel(1);
    let mut manager = Manager {
        id: identity.id.clone(),
        config,
        status: status.clone(),
        peers: BTreeMap::new(),
        source: None,
        receiver: None,
        actions,
        ownership: Ownership::default(),
        cancelled: BTreeMap::new(),
        pending: None,
        active: identity.id.clone(),
        epoch: 0,
        sequence: 0,
        waiting_release: None,
        incoming_epoch: None,
        prepared_epoch: None,
        observe_only,
        started: Instant::now(),
        renewal: 0,
        retry_after: Instant::now(),
        restart_requested: false,
    };
    manager.publish();
    let mut tick = tokio::time::interval(Duration::from_millis(50));
    let mut pulse = Instant::now();
    let result:Result<()>=async {loop {tokio::select! {
        Some(request)=updates.recv()=>{
            let result=manager.configure(request.config).map_err(|e|format!("{e:#}"));
            let _=request.done.send(result);
        },
        _=tick.tick()=>{
            if stop.load(Ordering::Acquire)||manager.restart_requested{break;}
            if let Some(n)=&manager.source {ensure!(!n.fault.load(Ordering::Acquire),"输入安全保护已触发。已暂停共享，请松开按键后恢复。");n.alive.store(n.started.elapsed().as_millis() as u64,Ordering::Release);}
            if manager.receiver.as_ref().is_some_and(|n|n.interrupted.load(Ordering::Acquire)){manager.cancel()?;}
            let effects=manager.ownership.expire(manager.now());manager.effects(effects)?;
            let expired:Vec<_>=manager.peers.iter().filter(|(_,p)|p.last_seen.elapsed()>Duration::from_secs(2)).map(|(id,_)|id.clone()).collect();
            for id in expired {manager.cancel()?;manager.peers.remove(&id);manager.cancelled.remove(&id);update(&status,|s|{s.connection_issues.insert(id.clone(),"连接响应超时".into());});manager.publish();}
            manager.start_source()?;
            if pulse.elapsed()>=Duration::from_millis(manager.config.tuning.heartbeat_ms){pulse=Instant::now();
                if let Some(c)=manager.ownership.claim().filter(|c|c.origin==manager.id).cloned(){manager.renewal+=1;for id in manager.ownership.renew(None,&c,manager.renewal,manager.now()){manager.send(&id,Message::Renew{claim:c.clone(),sequence:manager.renewal})?;}}
                let nonce=manager.now();
                let ids:Vec<_>=manager.peers.keys().cloned().collect();
                for id in ids{manager.send(&id,Message::Heartbeat{epoch:manager.epoch,nonce})?;manager.peers.get_mut(&id).unwrap().pings.sent(nonce);}
            }
        },
        action=manager.actions.recv(),if manager.source.is_some()=>{if let Some((queued,batch))=action{manager.actions(queued,batch)?;}else{bail!("原生输入线程已停止");}},
        event=rx.recv()=>{match event.context("网络后台已停止")? {
            Event::Connected(id,peer)=>{
                if manager.peers.get(&id).is_some_and(|old|old.preferred||!peer.preferred){continue;}
                // End old control before applying any remote route change. Native
                // release acknowledgement is still required before the next source.
                manager.cancel()?;
                let reconciled:Result<()>= (|| {
                    let mut s=status.lock().unwrap();
                    let mut topologies=s.topology.clone();
                    topologies.retain(|t|t.device.id!=id);topologies.push(peer.topology.clone());
                    if let Some(next)=crate::layout_sync::merge(&s.config,&manager.id,&id,&peer.pair_layout,topologies)? {
                        next.save()?;s.config=next.clone();manager.config=next;
                    }
                    if let Some(next)=crate::settings_sync::merge(&s.config,&peer.settings)? {
                        manager.restart_requested=next.same_subnet_only!=manager.config.same_subnet_only||next.tuning.lease_ms!=manager.config.tuning.lease_ms;
                        next.save()?;s.config=next.clone();manager.config=next;
                    }
                    Ok(())
                })();
                if let Err(e)=reconciled {update(&status,|s|{s.connection_issues.insert(id,format!("布局未同步：{e:#}"));});continue;}
                update(&status,|s|{s.connection_issues.remove(&id);});manager.cancelled.remove(&id);manager.peers.insert(id,peer);
                let settings=crate::settings_sync::snapshot(&manager.config);
                for id in manager.peers.keys() {manager.send(id,Message::Settings(settings.clone()))?;}
                manager.publish();
            },
            Event::Message(id,generation,queued,result)=>{
                if !manager.peers.get(&id).is_some_and(|p|p.generation==generation){continue;}
                manager.peers.get_mut(&id).unwrap().last_seen=Instant::now();
                let handled=match result {Ok(m)=>manager.message(&id,queued,m).await,Err(e)=>Err(e)};
                if let Err(e)=handled {manager.cancel()?;manager.peers.remove(&id);manager.cancelled.remove(&id);update(&status,|s|{s.connection_issues.insert(id.clone(),format!("{e:#}"));});manager.publish();}
            },
            Event::Failed(id,error)=>{if !manager.peers.contains_key(&id){update(&status,|s|{s.connection_issues.insert(id,error);});}},
        }}
    }}Ok(())}.await;
    let closing_claim = manager.ownership.claim().cloned();
    let closing_target = if manager.active != manager.id {
        Some(manager.active.clone())
    } else {
        None
    };
    let _ = manager.cancel();
    // Let queued cancellation reach the previous target and wait for its native
    // cleanup acknowledgement. If disconnected, its independent lease still expires.
    if let (Some(claim), Some(target)) = (closing_claim, closing_target) {
        let _ = timeout(Duration::from_millis(800), async {
            while let Some(event) = rx.recv().await {
                if let Event::Message(id, generation, _, Ok(Message::Cancelled(c))) = event {
                    if id == target
                        && c == claim
                        && manager
                            .peers
                            .get(&id)
                            .is_some_and(|p| p.generation == generation)
                    {
                        break;
                    }
                }
            }
        })
        .await;
    } else {
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    manager.peers.clear();
    manager.publish();
    drop(tasks);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn test_manager() -> (Manager, mpsc::Receiver<(Instant, Message)>) {
        let config = Config {
            sharing: true,
            ..Config::default()
        };
        let status = Arc::new(std::sync::Mutex::new(Status {
            local_addresses: vec![],
            local_addresses_error: None,
            diagnostics_path: None,
            peer_sharing: BTreeMap::new(),
            connection_issues: BTreeMap::new(),
            effective_links: vec![],
            topology_error: None,
            observe_only: true,
            pairing: None,
            discovery_error: None,
            last_command: None,
            enabled: true,
            state: String::new(),
            source: None,
            active: None,
            topology: vec![topology("B")],
            identity: "B".into(),
            certificate: vec![],
            trusted: vec![],
            rtt_ms: None,
            last_error: None,
            received: 0,
            config: config.clone(),
            discovered: vec![],
        }));
        let (tx, sent) = mpsc::channel(32);
        let peer = Peer {
            sharing: true,
            generation: 1,
            preferred: true,
            topology: topology("A"),
            pair_layout: PairLayout {
                revision: 0,
                author: "A".into(),
                links: vec![],
            },
            settings: crate::settings_sync::snapshot(&config),
            tx,
            last_seen: Instant::now(),
            pings: Pings::default(),
            _tasks: Tasks(vec![]),
        };
        let (_, actions) = mpsc::channel(1);
        (
            Manager {
                id: "B".into(),
                config,
                status,
                peers: BTreeMap::from([("A".into(), peer)]),
                source: None,
                receiver: None,
                actions,
                ownership: Ownership::default(),
                cancelled: BTreeMap::new(),
                pending: None,
                active: "B".into(),
                epoch: 0,
                sequence: 0,
                waiting_release: None,
                incoming_epoch: None,
                prepared_epoch: None,
                observe_only: true,
                started: Instant::now(),
                renewal: 0,
                retry_after: Instant::now(),
                restart_requested: false,
            },
            sent,
        )
    }
    #[tokio::test]
    async fn takeover_during_native_command_or_ack_is_not_a_connection_failure() {
        for command_stage in [true, false] {
            let (mut manager, _sent) = test_manager();
            let claim = Claim {
                origin: "A".into(),
                nonce: 42,
            };
            manager
                .ownership
                .request("A", claim, manager.neighbors(), 0);
            manager.incoming_epoch = Some(3);
            manager.receiver = Some(interrupted_receiver_for_test());
            if command_stage {
                let (reply, _) = tokio::sync::oneshot::channel();
                assert!(!manager
                    .receiver_command(ReceiverCommand::Release(reply))
                    .unwrap());
            } else {
                // Models an outstanding Activate/Release reply being dropped
                // after physical movement has interrupted the native worker.
                assert!(manager
                    .receiver_result::<()>(Err(anyhow::anyhow!("reply closed")))
                    .unwrap()
                    .is_none());
            }
            assert!(manager.ownership.claim().is_none());
            assert!(manager.receiver.is_none());
            assert!(manager.peers.contains_key("A"));
            assert!(manager.config.sharing);
            assert!(manager
                .receiver_result::<()>(Err(anyhow::anyhow!("unrelated error")))
                .is_err());
        }
    }
    #[tokio::test]
    async fn local_takeover_before_network_tick_keeps_connection() {
        let (mut manager, _sent) = test_manager();
        let claim = Claim {
            origin: "A".into(),
            nonce: 42,
        };
        manager
            .ownership
            .request("A", claim, manager.neighbors(), 0);
        manager.incoming_epoch = Some(3);
        // The native worker has observed local movement and closed its queue,
        // but the 50 ms manager tick has not run yet. No OS hooks in this test.
        manager.receiver = Some(interrupted_receiver_for_test());
        let packet = InputPacket {
            source: "A".into(),
            session: 42,
            epoch: 3,
            sequence: 7,
            event: InputEvent::Move {
                position: Point { x: 100., y: 100. },
                topology_version: 1,
            },
        };
        manager
            .message("A", Instant::now(), Message::Input(packet))
            .await
            .unwrap();
        assert!(manager.peers.contains_key("A"));
        assert!(manager.config.sharing);
        assert_eq!(manager.status.lock().unwrap().received, 0);
        assert!(manager.ownership.claim().is_none());
    }
    #[tokio::test]
    async fn local_takeover_drains_old_input_without_losing_peer_connection() {
        let (mut manager, mut sent) = test_manager();
        let claim = Claim {
            origin: "A".into(),
            nonce: 42,
        };
        manager
            .ownership
            .request("A", claim.clone(), manager.neighbors(), 0);
        manager.incoming_epoch = Some(3);
        manager.cancel().unwrap();
        assert!(matches!(sent.recv().await.unwrap().1, Message::Cancel(c) if c == claim));
        let packet = InputPacket {
            source: "A".into(),
            session: 42,
            epoch: 3,
            sequence: 7,
            event: InputEvent::Move {
                position: Point { x: 100., y: 100. },
                topology_version: 1,
            },
        };
        // This packet was already on the wire when B moved its own mouse.
        manager
            .message(
                "A",
                Instant::now() - Duration::from_secs(2),
                Message::Input(packet.clone()),
            )
            .await
            .unwrap();
        manager
            .message(
                "A",
                Instant::now(),
                Message::Activate {
                    epoch: 3,
                    topology_version: 1,
                    position: Point::default(),
                },
            )
            .await
            .unwrap();
        assert!(manager.peers.contains_key("A"));
        assert_eq!(manager.status.lock().unwrap().received, 0);
        assert!(manager.config.sharing);
        assert!(manager.source.is_none() && manager.receiver.is_none());
        let mut wrong = packet.clone();
        wrong.source = "other".into();
        assert!(manager
            .message("A", Instant::now(), Message::Input(wrong))
            .await
            .is_err());
        let mut wrong = packet.clone();
        wrong.session = 43;
        assert!(manager
            .message("A", Instant::now(), Message::Input(wrong))
            .await
            .is_err());
        manager
            .message(
                "A",
                Instant::now(),
                Message::Cancelled(Claim {
                    origin: "A".into(),
                    nonce: 41,
                }),
            )
            .await
            .unwrap();
        assert!(!manager.cancelled.is_empty());
        manager
            .message("A", Instant::now(), Message::Cancelled(claim))
            .await
            .unwrap();
        assert!(manager.cancelled.is_empty());
        // Traffic after the ordered cancellation ACK is not part of the drain.
        assert!(manager
            .message("A", Instant::now(), Message::Input(packet))
            .await
            .is_err());
    }
    #[tokio::test]
    async fn new_crossing_waits_for_previous_cancellation_acknowledgement() {
        let (mut manager, mut sent) = test_manager();
        let old = Claim {
            origin: "B".into(),
            nonce: 17,
        };
        manager.ownership.begin(old.clone(), manager.neighbors(), 0);
        manager.active = "A".into();
        manager.epoch = 2;
        manager.cancel().unwrap();
        sent.recv().await.unwrap();
        manager.pending = Some(("A".into(), 1, Point::default()));
        manager.reserve_pending().unwrap();
        assert!(manager.ownership.claim().is_none());
        manager
            .message("A", Instant::now(), Message::Activated { epoch: 2 })
            .await
            .unwrap();
        assert!(manager
            .message("A", Instant::now(), Message::Activated { epoch: 99 })
            .await
            .is_err());
        manager
            .message("A", Instant::now(), Message::Cancelled(old))
            .await
            .unwrap();
        assert!(matches!(sent.recv().await.unwrap().1, Message::Claim(c) if c.origin == "B"));
        assert!(manager.ownership.claim().is_some());
    }
    #[tokio::test]
    async fn paused_connected_peer_does_not_block_local_takeover_or_next_crossing() {
        let (mut manager, mut sent) = test_manager();
        let (mut extra, _unused) = test_manager();
        let mut paused = extra.peers.remove("A").unwrap();
        paused.topology = topology("C");
        paused.sharing = false;
        manager.peers.insert("C".into(), paused);
        let old = Claim {
            origin: "B".into(),
            nonce: 18,
        };
        manager.ownership.begin(old.clone(), manager.neighbors(), 0);
        manager.cancel().unwrap();
        assert_eq!(
            manager.cancelled.keys().cloned().collect::<Vec<_>>(),
            vec!["A"]
        );
        sent.recv().await.unwrap();
        manager.pending = Some(("A".into(), 1, Point::default()));
        manager
            .message("A", Instant::now(), Message::Cancelled(old))
            .await
            .unwrap();
        assert!(manager.cancelled.is_empty());
        assert!(manager.ownership.claim().is_some());
        assert!(matches!(sent.recv().await.unwrap().1, Message::Claim(c) if c.origin == "B"));
    }
    #[test]
    fn pong_must_match_a_request_and_an_enabled_screen() {
        let topology = topology("B");
        let mut pings = Pings::default();
        pings.sent(10);
        assert!(pings.accept(11, Point { x: 5., y: 5. }, &topology).is_err());
        assert!(pings
            .accept(10, Point { x: f64::NAN, y: 5. }, &topology)
            .is_err());
        assert!(pings
            .accept(10, Point { x: 90000., y: 5. }, &topology)
            .is_err());
        assert!(pings
            .accept(10, Point { x: -100., y: 5. }, &topology)
            .is_ok());
        assert!(pings
            .accept(10, Point { x: -100., y: 5. }, &topology)
            .is_err());
        for i in 0..100 {
            pings.sent(i);
        }
        assert_eq!(pings.0.len(), 32);
    }
    #[test]
    fn stale_release_cannot_end_a_new_control_epoch() {
        assert!(release_matches(3, Some(3), None));
        assert!(release_matches(3, None, Some(3)));
        assert!(!release_matches(2, Some(3), None));
        assert!(!release_matches(2, Some(3), Some(2)));
        assert!(!release_matches(3, None, None));
    }
    fn topology(id: &str) -> DeviceTopology {
        let monitor = |name: &str, x: f64| Monitor {
            id: name.into(),
            device: id.into(),
            name: name.into(),
            primary: x == 0.,
            rect: Rect {
                x,
                y: 0.,
                width: 1920.,
                height: 1080.,
            },
            resolution: [1920, 1080],
            scale: 1.,
            rotation: 0,
            enabled: true,
            mirror_of: None,
            unit: CoordinateUnit::PhysicalPixels,
        };
        DeviceTopology {
            device: Device {
                id: id.into(),
                name: id.into(),
                os: OperatingSystem::Windows,
                online: true,
                authorized: true,
                permission: PermissionState::Granted,
            },
            version: 1,
            monitors: vec![monitor("left", -1920.), monitor("main", 0.)],
        }
    }
    #[test]
    fn automatic_edges_are_reciprocal_and_do_not_block_native_screens() {
        let topologies = vec![topology("B"), topology("A")];
        let links = default_links(&topologies);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].from.device, "A");
        assert_eq!(links[0].from.monitor, "main");
        assert_eq!(links[0].to.monitor, "left");
        assert_eq!(links[1].from, links[0].to);
        assert_eq!(links[1].to, links[0].from);
        let layout = Layout { topologies, links };
        layout.validate().unwrap();
        assert!(layout
            .route(
                "A",
                Point { x: 1919., y: 400. },
                Point { x: 5., y: 0. },
                false
            )
            .is_none());
        assert_eq!(
            layout
                .route(
                    "A",
                    Point { x: 1919., y: 400. },
                    Point { x: 5., y: 0. },
                    true
                )
                .unwrap()
                .device,
            "B"
        );
        assert!(layout
            .route("A", Point { x: -1., y: 400. }, Point { x: 5., y: 0. }, true)
            .is_none());
    }
    #[test]
    fn automatic_edges_ignore_mirrors_offline_devices_and_input_order() {
        let mut a = topology("A");
        let mut mirror = a.monitors[0].clone();
        mirror.id = "mirror".into();
        mirror.mirror_of = Some("left".into());
        a.monitors.push(mirror);
        let mut offline = topology("offline");
        offline.device.online = false;
        let first = vec![a.clone(), topology("B"), topology("C"), offline];
        let second = vec![topology("C"), topology("B"), a];
        assert_eq!(default_links(&first), default_links(&second));
        Layout {
            links: default_links(&first),
            topologies: first,
        }
        .validate()
        .unwrap();
    }
}
