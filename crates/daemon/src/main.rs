mod config;
mod discovery;
mod display_state;
mod guard;
mod input_delivery;
mod layout_sync;
mod native;
mod network_policy;
mod outbound_delivery;
mod pairing;
mod peers;
mod settings_sync;
mod source_cleanup;
mod status;
mod test_edition;
use anyhow::{bail, Result};
use crossdesk_platform as platform;
use crossdesk_transport::*;
use serde::Deserialize;
use status::*;
use std::{
    io::{BufRead, Read, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

#[derive(Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
enum Command {
    Status,
    Configure {
        config: config::Config,
    },
    Trust {
        certificate: Vec<u8>,
        fingerprint: String,
        name: String,
    },
    Revoke {
        id: String,
    },
    StartSource,
    StartReceiver,
    Resume,
    Pause,
    Quit,
    Refresh,
    Discover,
    PairOpen,
    PairConnect {
        address: String,
        code: String,
    },
    PairConfirm {
        session: String,
        accepted: bool,
    },
    PairCancel,
    ExportDiagnostics,
}
struct Running {
    stop: Arc<AtomicBool>,
    task: tokio::task::JoinHandle<()>,
    configure: tokio::sync::mpsc::Sender<peers::Configure>,
}
async fn stop_running(running: &mut Option<Running>) {
    if let Some(r) = running.take() {
        r.stop.store(true, Ordering::Release);
        let _ = r.task.await;
    }
}
#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("CrossDesk: {e:#}");
        std::process::exit(1);
    }
}
async fn run() -> Result<()> {
    platform::dpi_awareness()?;
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).is_some_and(|a| a == "--profile-info") {
        println!(
            "{}",
            serde_json::json!({
                "test": crossdesk_core::profile::IS_TEST,
                "data_directory": crossdesk_core::profile::DATA_DIRECTORY,
                "bundle_id": crossdesk_core::profile::BUNDLE_ID,
                "keychain_service": crossdesk_core::profile::KEYCHAIN_SERVICE,
                "port": crossdesk_core::profile::DEFAULT_PORT,
                "protocol": crossdesk_transport::protocol::VERSION,
            })
        );
        return Ok(());
    }
    if args.get(1).is_some_and(|a| a == "--remove-test-data") {
        return test_edition::remove();
    }
    let observe_only = args.iter().any(|a| a == "--observe-only");
    if args.get(1).is_some_and(|a| a == "--version") {
        println!("CrossDesk {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    #[cfg(windows)]
    if args.get(1).is_some_and(|a| a == "inspect-input-safety") {
        println!(
            "{}",
            serde_json::json!({
                "input_desktop_available": platform::input_desktop_available(),
                "read_only": true,
                "hooks_installed": false,
                "input_injected": false
            })
        );
        return Ok(());
    }
    if args.get(1).is_some_and(|a| a == "--input-guard") {
        return guard::run(
            args.get(2)
                .ok_or_else(|| anyhow::anyhow!("missing guardian lease"))?
                .parse()?,
        );
    }
    if args.get(1).is_some_and(|a| a == "inspect") {
        let topo = platform::topology("local-inspection", 1)?;
        println!("{}", serde_json::to_string_pretty(&topo)?);
        return Ok(());
    }
    let dir = data_dir();
    let _test_session = test_edition::lock()?;
    let identity = Arc::new(Identity::load_or_create(&dir)?);
    if args.get(1).is_some_and(|a| a == "identity") {
        println!(
            "Fingerprint: {}\nCertificate: {}",
            identity.id,
            dir.join("identity.der").display()
        );
        return Ok(());
    }
    if !args.get(1).is_some_and(|a| a == "--stdio") {
        bail!("Use inspect (read-only), identity, or the CrossDesk desktop application. Sharing requires an explicit Start action.");
    }
    let loaded = config::Config::load().and_then(|c| {
        c.validate()?;
        Ok(c)
    });
    let mut startup_error = loaded
        .as_ref()
        .err()
        .map(|e| format!("配置读取失败，已安全暂停并保留原文件：{e}"));
    let config = loaded.unwrap_or_default();
    let trusted = match load_trust(&dir) {
        Ok(t) => t,
        Err(e) => {
            startup_error = Some(e.to_string());
            vec![]
        }
    };
    let status = Arc::new(Mutex::new(Status {
        local_addresses: vec![],
        local_addresses_error: None,
        diagnostics_path: None,
        peer_sharing: Default::default(),
        connection_issues: Default::default(),
        effective_links: vec![],
        topology_error: None,
        observe_only,
        pairing: None,
        discovery_error: None,
        last_command: None,
        enabled: false,
        state: "已暂停 · 未捕获输入".into(),
        source: None,
        active: None,
        topology: vec![platform::topology(&identity.id, 1)?],
        identity: identity.id.clone(),
        certificate: identity.certificate.clone(),
        trusted,
        rtt_ms: None,
        last_error: startup_error,
        received: 0,
        config,
        discovered: vec![],
    }));
    // Anonymous inherited pipes: only the parent process owns the command handle. No localhost control port.
    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(32);
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        let mut reader = stdin.lock();
        loop {
            let mut line = String::new();
            match reader.by_ref().take(262145).read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    if line.len() > 262144 || tx.blocking_send(line).is_err() {
                        break;
                    }
                }
            }
        }
    });
    refresh_addresses(&status);
    let mut running: Option<Running> = None;
    let mut pairing = pairing::PairingService::default();
    let mut discovery = match discovery::start(&identity.id, status.clone()) {
        Ok(d) => Some(d),
        Err(e) => {
            update(&status, |s| {
                s.discovery_error = Some(format!("自动发现暂不可用：{e}。仍可使用 IP 配对。"))
            });
            None
        }
    };
    let mut tick = tokio::time::interval(Duration::from_millis(500));
    let mut refresh = tokio::time::interval(Duration::from_secs(3));
    let mut version = 1u64;
    loop {
        tokio::select! {
            _=tick.tick()=>{
                if running.as_ref().is_some_and(|r|r.task.is_finished()){stop_running(&mut running).await;}
                let can_connect = { let s=status.lock().unwrap(); !s.trusted.is_empty() && s.topology_error.is_none() };
                if running.is_none() && !pairing.busy() && can_connect {
                    let config=status.lock().unwrap().config.clone();let id=identity.clone();let shared=status.clone();
                    let stop=Arc::new(AtomicBool::new(false));let stopped=stop.clone();
                    update(&status,|s|s.enabled=config.sharing);
                    let (configure, updates)=tokio::sync::mpsc::channel(8);
                    let task=tokio::spawn(async move{
                        let result=peers::run(id,config,shared.clone(),stopped,observe_only,updates).await;
                        if let Err(e)=result {update(&shared,|s|{s.enabled=false;s.config.sharing=false;s.active=None;s.last_error=Some(format!("{e:#}"));s.state="已安全暂停 · 可以恢复共享".into();});let paused=shared.lock().unwrap().config.clone();let _=paused.save();}
                    });
                    running=Some(Running{stop,task,configure});
                }
                let s=status.lock().unwrap().clone();println!("{}",serde_json::to_string(&s)?);std::io::stdout().flush()?;
            },
            _=refresh.tick()=>{
                // Errors are recorded by refresh; never continue using an unverified layout.
                let _ = display_state::refresh(&status,&mut running,&identity.id,&mut version).await;
            },
            line=rx.recv()=>{
                let Some(line)=line else {break;};
                let request_id = serde_json::from_str::<serde_json::Value>(&line).ok()
                    .and_then(|v| v.get("request_id").and_then(|id| id.as_str()).map(str::to_owned));
                let result:Result<()>=async{
                    let command:Command=serde_json::from_str(&line)?;
                    if matches!(command, Command::Configure{..}|Command::Trust{..}|Command::Revoke{..}|Command::StartSource|Command::StartReceiver|Command::Resume) && pairing.busy() {
                        bail!("请先完成或取消当前配对，再修改配置或启用输入共享");
                    }
                    match command {
                        Command::PairOpen=>{refresh_addresses(&status);stop_running(&mut running).await;update(&status,|s|{s.enabled=false;s.active=None;s.state="正在配对 · 键鼠共享暂停".into();});pairing.open(identity.clone(),status.clone()).await?;},
                        Command::PairConnect{address,code}=>{stop_running(&mut running).await;update(&status,|s|{s.enabled=false;s.active=None;s.state="正在配对 · 键鼠共享暂停".into();});pairing.connect(identity.clone(),status.clone(),address,code).await?;},
                        Command::PairConfirm{session,accepted}=>pairing.confirm(&status,session,accepted)?,
                        Command::PairCancel=>pairing.cancel(&status).await,
                        Command::Status=>{},
                        Command::Quit=>{stop_running(&mut running).await;return Err(anyhow::anyhow!("__quit"));},
                        Command::Pause=>{pairing.cancel(&status).await;stop_running(&mut running).await;let mut c=status.lock().unwrap().config.clone();c.sharing=false;c.save()?;update(&status,|s|{s.config=c;s.enabled=false;s.last_error=None;s.state="已暂停 · 输入已释放".into();s.active=None;});},
                        Command::Configure{mut config}=>{
                            config.validate()?;
                            let old=status.lock().unwrap().config.clone();
                            // Sharing is a live operation, never restored from an old UI edit.
                            config.sharing=old.sharing;
                            let port_changed=old.port!=config.port;
                            let reconnect=port_changed||old.endpoints!=config.endpoints||old.same_subnet_only!=config.same_subnet_only||old.tuning.lease_ms!=config.tuning.lease_ms;
                            let topologies=status.lock().unwrap().topology.clone();
                            crossdesk_core::Layout{topologies,links:config.links.clone()}.validate().map_err(anyhow::Error::msg)?;
                            if !reconnect {
                                if let Some(r)=running.as_ref().filter(|r|!r.task.is_finished()) {
                                    let (done,result)=tokio::sync::oneshot::channel();
                                    r.configure.send(peers::Configure{config,done}).await.map_err(|_|anyhow::anyhow!("连接后台已停止，请重试"))?;
                                    tokio::time::timeout(Duration::from_secs(5),result).await.map_err(|_|anyhow::anyhow!("设置应用尚未确认，请稍后重试"))??.map_err(anyhow::Error::msg)?;
                                    return Ok(());
                                }
                            }
                            stop_running(&mut running).await;
                            let old=status.lock().unwrap().config.clone();
                            let peers=status.lock().unwrap().trusted.iter().map(|p|p.id.clone()).collect::<Vec<_>>();
                            layout_sync::record_changes(&mut config,&old,&identity.id,&peers)?;
                            settings_sync::record_changes(&mut config,&old,&identity.id)?;
                            config.save()?;
                            update(&status,|s|{s.config=config;s.last_error=None;s.state="配置已保存 · 正在重新连接".into();});
                            refresh_addresses(&status);
                            if port_changed {
                                if let Some(d)=discovery.take(){let _=d.shutdown();}
                                discovery=match discovery::start(&identity.id,status.clone()) {
                                    Ok(d)=>{update(&status,|s|s.discovery_error=None);Some(d)},
                                    Err(e)=>{update(&status,|s|s.discovery_error=Some(format!("自动发现暂不可用：{e}。仍可使用 IP 配对。")));None}
                                };
                            }
                        },
                        Command::Trust{certificate,fingerprint,name}=>{stop_running(&mut running).await;trust(&dir,certificate,&fingerprint,name)?;let peers=load_trust(&dir)?;update(&status,|s|s.trusted=peers);},
                        Command::Revoke{id}=>{stop_running(&mut running).await;revoke(&dir,&id)?;let peers=load_trust(&dir)?;update(&status,|s|{s.trusted=peers;s.connection_issues.remove(&id);});},
                        Command::Refresh=>{refresh_addresses(&status);display_state::refresh(&status,&mut running,&identity.id,&mut version).await?;},
                        Command::Discover=>{refresh_addresses(&status);if let Some(d)=discovery.take(){let _=d.shutdown();}discovery=Some(discovery::start(&identity.id,status.clone())?);update(&status,|s|s.discovery_error=None);},
                        Command::ExportDiagnostics=>{
                            let s=status.lock().unwrap();let sanitized=serde_json::json!({"version":env!("CARGO_PKG_VERSION"),"platform":std::env::consts::OS,"state":s.state,"connected_devices":s.topology.len(),"display_counts":s.topology.iter().map(|t|t.monitors.len()).collect::<Vec<_>>(),"rtt_ms":s.rtt_ms,"received_count":s.received,"sharing":s.enabled});
                            atomic_save(&dir.join("diagnostics.json"),&serde_json::to_vec_pretty(&sanitized)?)?;drop(s);update(&status,|s|s.diagnostics_path=Some(dir.join("diagnostics.json").to_string_lossy().into_owned()));
                        },
                        Command::StartSource|Command::StartReceiver|Command::Resume=>{
                            anyhow::ensure!(status.lock().unwrap().topology_error.is_none(), "请先成功读取本机显示器，再恢复共享");
                            stop_running(&mut running).await;
                            let mut config=status.lock().unwrap().config.clone();config.sharing=true;config.save()?;
                            update(&status,|s|{s.config=config;s.enabled=true;s.last_error=None;s.state="正在恢复双向共享".into();});
                        }
                    }Ok(())
                }.await;
                if result.as_ref().is_err_and(|e| e.to_string()=="__quit") {break;}
                let error = result.err().map(|e|e.to_string());
                update(&status, |s| {
                    if let Some(e) = &error { s.last_error = Some(e.clone()); }
                    if let Some(id) = request_id { s.last_command = Some(CommandOutcome { id, error }); }
                });
            }
        }
    }
    stop_running(&mut running).await;
    pairing.cancel(&status).await;
    if let Some(d) = discovery {
        let _ = d.shutdown();
    }
    Ok(())
}
