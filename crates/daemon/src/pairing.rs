use crate::{config::Endpoint, status::*};
use anyhow::{bail, ensure, Context, Result};
use crossdesk_transport::{
    pairing::{self, Frame, Hello},
    *,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc,
    task::JoinHandle,
    time::timeout,
};

const WINDOW: Duration = Duration::from_secs(120);
#[derive(Clone, Serialize, Deserialize, Default)]
pub struct PairingStatus {
    pub phase: String,
    pub session_id: String,
    pub code: Option<String>,
    pub peer_name: Option<String>,
    pub peer_address: Option<String>,
    pub expires_at_ms: u64,
    pub error: Option<String>,
}
struct Approval {
    session: String,
    accepted: bool,
}
#[derive(Default)]
pub struct PairingService {
    task: Option<JoinHandle<()>>,
    approvals: Option<mpsc::Sender<Approval>>,
}
fn session_id() -> String {
    format!("{:016x}", rand::random::<u64>())
}
fn expiry() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
        + WINDOW.as_millis() as u64
}
fn local_hello(identity: &Identity, status: &SharedStatus) -> Hello {
    let s = status.lock().unwrap();
    Hello {
        certificate: identity.certificate.clone(),
        name: s.topology[0]
            .device
            .name
            .chars()
            .filter(|c| !c.is_control())
            .take(40)
            .collect(),
        input_port: s.config.port,
    }
}
pub fn parse_address(address: &str) -> Result<SocketAddr> {
    let address = address.trim();
    let input = if let Ok(ip) = address.parse::<IpAddr>() {
        SocketAddr::new(ip, crossdesk_core::profile::DEFAULT_PORT)
    } else {
        address
            .parse::<SocketAddr>()
            .context("请输入对方 IP，例如 192.168.1.20；自定义端口可写 IP:端口")?
    };
    ensure!(
        input.is_ipv4(),
        "当前配对请使用 IPv4 地址，例如 192.168.1.20"
    );
    ensure!(
        input.port() >= 1024 && input.port() < u16::MAX,
        "输入端口必须在 1024–65534 之间"
    );
    Ok(input)
}
impl PairingService {
    pub fn busy(&self) -> bool {
        self.task.as_ref().is_some_and(|t| !t.is_finished())
    }
    pub async fn cancel(&mut self, status: &SharedStatus) {
        if let Some(task) = self.task.take() {
            task.abort();
            let _ = task.await;
        }
        self.approvals = None;
        update(status, |s| s.pairing = None);
    }
    pub fn confirm(&self, status: &SharedStatus, session: String, accepted: bool) -> Result<()> {
        {
            let s = status.lock().unwrap();
            let p = s.pairing.as_ref().context("没有等待确认的配对")?;
            ensure!(
                p.phase == "confirming" && p.session_id == session,
                "配对已过期或确认对象已经改变"
            );
        }
        self.approvals
            .as_ref()
            .context("配对已结束")?
            .try_send(Approval { session, accepted })
            .map_err(|_| anyhow::anyhow!("配对确认队列繁忙或已经结束"))?;
        update(status, |s| {
            if let Some(p) = &mut s.pairing {
                p.phase = "waiting".into();
            }
        });
        Ok(())
    }
    pub async fn open(&mut self, identity: Arc<Identity>, status: SharedStatus) -> Result<()> {
        self.cancel(&status).await;
        let hello = local_hello(&identity, &status);
        let port = hello
            .input_port
            .checked_add(1)
            .context("配对端口超出范围")?;
        let listener = TcpListener::bind((std::net::Ipv4Addr::UNSPECIFIED, port))
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "无法开启配对端口 {port}：{e}。请检查是否已有另一份 CrossDesk 正在运行"
                )
            })?;
        let code = format!("{:08}", rand::rng().random_range(0..100_000_000u32));
        let expires = expiry();
        update(&status, |s| {
            s.pairing = Some(PairingStatus {
                phase: "listening".into(),
                session_id: session_id(),
                code: Some(code.clone()),
                expires_at_ms: expires,
                ..Default::default()
            })
        });
        let (tx, mut approvals) = mpsc::channel(4);
        self.approvals = Some(tx);
        self.task = Some(tokio::spawn(async move {
            let result = timeout(WINDOW, async {
                for attempt in 0..5 {
                    let (socket, address) = loop {
                        let (socket, address) = listener.accept().await?;
                        let restricted = status.lock().unwrap().config.same_subnet_only;
                        if crate::network_policy::check(address.ip(), restricted).is_ok() {
                            break (socket, address);
                        }
                    };
                    socket.set_nodelay(true)?;
                    let exchange = timeout(
                        Duration::from_secs(8),
                        pairing::establish(socket, &code, hello.clone(), false),
                    )
                    .await;
                    match exchange {
                        Ok(Ok((tx, rx, peer))) => {
                            return finish(
                                tx,
                                rx,
                                peer,
                                &status,
                                &mut approvals,
                                PairContext {
                                    ip: address.ip(),
                                    expires,
                                    code: &code,
                                },
                            )
                            .await
                        }
                        _ => update(&status, |s| {
                            if let Some(p) = &mut s.pairing {
                                p.error = Some(format!(
                                    "配对认证未通过，剩余 {} 次尝试。请核对配对码。",
                                    4 - attempt
                                ));
                            }
                        }),
                    }
                }
                bail!("配对尝试次数已用完，请重新显示配对码")
            })
            .await;
            complete(&status, result);
        }));
        Ok(())
    }
    pub async fn connect(
        &mut self,
        identity: Arc<Identity>,
        status: SharedStatus,
        address: String,
        code: String,
    ) -> Result<()> {
        let endpoint = parse_address(&address)?;
        crate::network_policy::check(
            endpoint.ip(),
            status.lock().unwrap().config.same_subnet_only,
        )?;
        pairing::validate_code(&code)?;
        self.cancel(&status).await;
        let hello = local_hello(&identity, &status);
        let expires = expiry();
        update(&status, |s| {
            s.pairing = Some(PairingStatus {
                phase: "connecting".into(),
                session_id: session_id(),
                peer_address: Some(endpoint.to_string()),
                expires_at_ms: expires,
                ..Default::default()
            })
        });
        let (tx, mut approvals) = mpsc::channel(4);
        self.approvals = Some(tx);
        self.task = Some(tokio::spawn(async move {
            let result = timeout(WINDOW, async {
                let socket = timeout(
                    Duration::from_secs(5),
                    TcpStream::connect(SocketAddr::new(endpoint.ip(), endpoint.port() + 1)),
                )
                .await
                .context("连接超时：请检查 IP、防火墙以及对方是否已显示配对码")??;
                socket.set_nodelay(true)?;
                let (tx, rx, peer) = timeout(
                    Duration::from_secs(8),
                    pairing::establish(socket, &code, hello, true),
                )
                .await
                .context("配对握手超时")??;
                ensure!(
                    peer.input_port == endpoint.port(),
                    "对方公布的输入端口与连接地址不一致"
                );
                finish(
                    tx,
                    rx,
                    peer,
                    &status,
                    &mut approvals,
                    PairContext {
                        ip: endpoint.ip(),
                        expires,
                        code: &code,
                    },
                )
                .await
            })
            .await;
            complete(&status, result);
        }));
        Ok(())
    }
}
fn complete(status: &SharedStatus, result: Result<Result<()>, tokio::time::error::Elapsed>) {
    let error = match result {
        Ok(Ok(())) => None,
        Ok(Err(e)) => Some(e.to_string()),
        Err(_) => Some("配对已超时，请重新显示配对码并在两边确认".into()),
    };
    update(status, |s| {
        if let Some(p) = &mut s.pairing {
            p.code = None;
            p.phase = if error.is_some() {
                "error"
            } else {
                "completed"
            }
            .into();
            p.error = error;
        }
    });
}
struct PairContext<'a> {
    ip: IpAddr,
    expires: u64,
    code: &'a str,
}
async fn finish(
    mut tx: pairing::Sender<tokio::io::WriteHalf<TcpStream>>,
    mut rx: pairing::Receiver<tokio::io::ReadHalf<TcpStream>>,
    peer: Hello,
    status: &SharedStatus,
    approvals: &mut mpsc::Receiver<Approval>,
    context: PairContext<'_>,
) -> Result<()> {
    let PairContext { ip, expires, code } = context;
    let session = session_id();
    let peer_id = peer.id();
    let address = SocketAddr::new(ip, peer.input_port).to_string();
    update(status, |s| {
        s.pairing = Some(PairingStatus {
            phase: "confirming".into(),
            session_id: session.clone(),
            code: Some(code.into()),
            peer_name: Some(peer.name.clone()),
            peer_address: Some(address.clone()),
            expires_at_ms: expires,
            error: None,
        })
    });
    // Confirmations are local user actions, bound to this authenticated exchange, never discovery hints.
    let send_approval = async {
        loop {
            let approval = approvals.recv().await.context("配对确认已取消")?;
            if approval.session != session {
                continue;
            }
            tx.send(&Frame::Approval(approval.accepted)).await?;
            ensure!(approval.accepted, "已拒绝配对");
            return Ok::<(), anyhow::Error>(());
        }
    };
    let read_approval = async {
        ensure!(
            matches!(rx.receive().await?, Frame::Approval(true)),
            "对方取消了配对"
        );
        Ok::<(), anyhow::Error>(())
    };
    tokio::try_join!(send_approval, read_approval)?;
    // Neither trust nor the endpoint is saved before BOTH local users approve.
    trust(&data_dir(), peer.certificate, &peer_id, peer.name)?;
    let peers = load_trust(&data_dir())?;
    let mut config = status.lock().unwrap().config.clone();
    config.endpoints.retain(|e| e.peer != peer_id);
    config.endpoints.push(Endpoint {
        peer: peer_id,
        address,
    });
    config.save()?;
    update(status, |s| {
        s.trusted = peers;
        s.config = config;
        s.last_error = None;
    });
    tx.send(&Frame::Saved).await?;
    ensure!(
        matches!(rx.receive().await?, Frame::Saved),
        "对方尚未完成保存，请重试配对"
    );
    let mut config = status.lock().unwrap().config.clone();
    config.sharing = true;
    config.save()?;
    update(status, |s| s.config = config);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manual_ip_defaults_and_invalid_ports() {
        assert_eq!(
            parse_address("192.168.1.2").unwrap().port(),
            crossdesk_core::profile::DEFAULT_PORT
        );
        assert_eq!(parse_address("192.168.1.2:30000").unwrap().port(), 30000);
        assert!(parse_address("192.168.1.2:65535").is_err());
        assert!(parse_address("192.168.1.2:0").is_err());
        assert!(parse_address("not-an-ip").is_err());
    }
}
