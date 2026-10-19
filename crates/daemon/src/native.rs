use crossdesk_core::*;
use crossdesk_platform::{self as platform, NativeEvent};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, SyncSender},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::sync::mpsc::Sender;

// An unconfirmed old OS operation must not overlap a new input owner. This latch
// intentionally lasts for the daemon lifetime, even if the old worker returns late.
static UNCONFIRMED_CLEANUP: AtomicBool = AtomicBool::new(false);
pub fn mark_unconfirmed_cleanup() {
    UNCONFIRMED_CLEANUP.store(true, Ordering::Release);
}
pub fn ensure_cleanup_confirmed() -> anyhow::Result<()> {
    anyhow::ensure!(
        !UNCONFIRMED_CLEANUP.load(Ordering::Acquire),
        "上一次原生输入清理未能确认，已阻止新的控制。请完全退出 CrossDesk 后重新打开；不要仅关闭设置窗口。"
    );
    Ok(())
}
#[derive(Debug)]
pub enum SourceCommand {
    Ready(String, u64),
    Stop,
    Cursor(Point),
}
pub struct SourceHandle {
    pub commands: SyncSender<SourceCommand>,
    pub stop: Arc<AtomicBool>,
    pub fault: Arc<AtomicBool>,
    pub alive: Arc<AtomicU64>,
    pub started: Instant,
    pub join: Option<std::thread::JoinHandle<()>>,
}
impl Drop for SourceHandle {
    fn drop(&mut self) {
        let _ = self.commands.try_send(SourceCommand::Stop);
        self.stop.store(true, Ordering::Release);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}
pub fn source(mut session: ControlSession, events: Sender<(Instant, Vec<Action>)>) -> SourceHandle {
    let (tx, rx) = mpsc::sync_channel(32);
    let stop = Arc::new(AtomicBool::new(false));
    let s = stop.clone();
    let alive = Arc::new(AtomicU64::new(0));
    let a = alive.clone();
    let start = Instant::now();
    let fault = Arc::new(AtomicBool::new(false));
    let failed = fault.clone();
    let capture_failure = fault.clone();
    let join = std::thread::spawn(move || {
        let mut batcher = MotionBatcher::new(session.tuning.clone());
        let (mut cleanup, cleanup_join) =
            crate::source_cleanup::LocalCleanup::spawn(platform::inject);
        let mut activation = crate::source_cleanup::ActivationBarrier::default();
        let mut stopped = false;
        let mut last_motion = Instant::now();
        let handler = move |event: NativeEvent| -> bool {
            let now = start.elapsed().as_millis() as u64;
            let mut actions = vec![];
            let mut suppress = session.remote() || session.phase == Phase::PreparingSwitch;
            if matches!(event, NativeEvent::Shutdown) {
                actions.extend(session.recover());
                stopped = true;
            }
            if matches!(event, NativeEvent::Tick) {
                while let Ok(command) = rx.try_recv() {
                    match command {
                        SourceCommand::Ready(id, epoch) => {
                            if !stopped {
                                match activation.ready(&mut session, &mut cleanup, id, epoch, now) {
                                    Ok(ready) => actions.extend(ready),
                                    Err(error) => {
                                        eprintln!("local input cleanup failed: {error:#}");
                                        actions.extend(session.recover());
                                        stopped = true;
                                    }
                                }
                            }
                        }
                        SourceCommand::Stop => {
                            actions.extend(session.recover());
                            stopped = true;
                        }
                        SourceCommand::Cursor(p) => {
                            if session.remote()
                                && last_motion.elapsed() > Duration::from_millis(100)
                            {
                                session.position = p;
                            }
                        }
                    }
                }
                if now.saturating_sub(a.load(Ordering::Acquire)) > 500 && !stopped {
                    actions.extend(session.recover());
                    stopped = true;
                }
                if !stopped {
                    match activation.poll(&mut session, &cleanup, now) {
                        Ok(ready) => actions.extend(ready),
                        Err(error) => {
                            eprintln!("local input cleanup failed: {error:#}");
                            actions.extend(session.recover());
                            stopped = true;
                        }
                    }
                }
                actions.extend(session.tick(now));
            }
            if !stopped {
                match event {
                    NativeEvent::Input(e) => {
                        let d = session.input(e);
                        suppress = d.suppress;
                        actions.extend(d.actions);
                    }
                    NativeEvent::Motion { delta, local } => {
                        last_motion = Instant::now();
                        let p = if session.remote() {
                            session.position
                        } else {
                            local
                        };
                        let d = session.motion(p, delta, now);
                        suppress = d.suppress;
                        actions.extend(d.actions);
                    }
                    NativeEvent::UnsupportedKey => {
                        suppress = session.remote();
                        actions.extend(session.recover());
                        stopped = true;
                    }
                    NativeEvent::TopologyChanged => {
                        actions.extend(session.recover());
                        stopped = true;
                    }
                    _ => {}
                }
            } else {
                suppress = false;
            }
            // Only enqueue releases here. SendInput and waiting for it both stay off this thread.
            actions.retain(|act| {
                if matches!(act, Action::LocalRelease { .. }) {
                    if let Err(error) = cleanup.submit(act.clone()) {
                        eprintln!("local input cleanup could not be queued: {error:#}");
                        stopped = true;
                    }
                    false
                } else {
                    !matches!(act,Action::Deliver{target,..} if target==&session.source)
                }
            });
            #[cfg(windows)]
            platform::set_capture_suppression(
                !stopped && (session.remote() || session.phase == Phase::PreparingSwitch),
            );
            if stopped {
                // Failed local modifier cleanup must never be followed by activating another machine.
                actions.clear();
                actions.push(Action::Recovered);
            }
            let actions = batcher.process(actions, now);
            if !actions.is_empty() && events.try_send((Instant::now(), actions)).is_err() {
                let recovery = session.recover();
                for act in recovery {
                    if matches!(act, Action::LocalRelease { .. }) {
                        if let Err(error) = cleanup.submit(act) {
                            eprintln!("local input recovery could not be queued: {error:#}");
                        }
                    }
                }
                stopped = true;
                suppress = false;
            }
            if stopped {
                failed.store(true, Ordering::Release);
            }
            #[cfg(windows)]
            platform::set_capture_suppression(
                !stopped && (session.remote() || session.phase == Phase::PreparingSwitch),
            );
            suppress
        };
        if let Err(error) = platform::capture(Box::new(handler), s) {
            eprintln!("native capture stopped: {error:#}");
            capture_failure.store(true, Ordering::Release);
        }
        // capture has uninstalled its hooks and dropped the sender. A blocked OS call
        // cannot trap the hook thread or make shutdown wait indefinitely.
        let deadline = Instant::now() + Duration::from_millis(150);
        while !cleanup_join.is_finished() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(2));
        }
        if cleanup_join.is_finished() {
            if cleanup_join.join().is_err() {
                capture_failure.store(true, Ordering::Release);
            }
        } else {
            eprintln!("local release worker did not stop within its deadline");
            mark_unconfirmed_cleanup();
            capture_failure.store(true, Ordering::Release);
        }
    });
    SourceHandle {
        commands: tx,
        stop,
        fault,
        alive,
        started: start,
        join: Some(join),
    }
}

pub enum ReceiverCommand {
    Activate {
        source: String,
        session: u64,
        epoch: u64,
        position: Point,
        reply: tokio::sync::oneshot::Sender<anyhow::Result<()>>,
    },
    Input(Instant, InputPacket),
    Heartbeat(String, u64, u64),
    Release(tokio::sync::oneshot::Sender<anyhow::Result<()>>),
}
pub struct ReceiverHandle {
    pub tx: SyncSender<ReceiverCommand>,
    pub ready: Arc<AtomicBool>,
    pub interrupted: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
    capture_join: Option<std::thread::JoinHandle<()>>,
}
impl Drop for ReceiverHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
        if let Some(j) = self.capture_join.take() {
            let _ = j.join();
        }
    }
}
pub fn receiver(topology: DeviceTopology, lease_ms: u64) -> ReceiverHandle {
    let (tx, rx) = mpsc::sync_channel(512);
    let stop = Arc::new(AtomicBool::new(false));
    let interrupted = Arc::new(AtomicBool::new(false));
    let active = Arc::new(AtomicBool::new(false));
    let local = interrupted.clone();
    let capture_stop = stop.clone();
    let capture_active = active.clone();
    let ready = Arc::new(AtomicBool::new(false));
    let capture_ready = ready.clone();
    let capture_failure = interrupted.clone();
    let capture_join = std::thread::spawn(move || {
        let failed = platform::capture(
            Box::new(move |e| {
                if matches!(e, NativeEvent::Started) {
                    capture_ready.store(true, Ordering::Release);
                }
                let physical = !matches!(
                    e,
                    NativeEvent::Started | NativeEvent::Tick | NativeEvent::Shutdown
                ) && capture_active.load(Ordering::Acquire);
                if physical {
                    local.store(true, Ordering::Release);
                }
                physical
            }),
            capture_stop,
        );
        if failed.is_err() {
            capture_failure.store(true, Ordering::Release);
        }
    });
    let s = stop.clone();
    let abort = interrupted.clone();
    let initialized = Arc::new(AtomicBool::new(false));
    let initialized_worker = initialized.clone();
    let join = std::thread::spawn(move || {
        let start = Instant::now();
        let mut guardian = match crate::guard::Guard::spawn(lease_ms) {
            Ok(g) => g,
            Err(_) => {
                abort.store(true, Ordering::Release);
                return;
            }
        };
        let mut ledger = crossdesk_core::receiver::Receiver::default();
        if ledger.set_lease(lease_ms).is_err() {
            abort.store(true, Ordering::Release);
            return;
        }
        while !ready.load(Ordering::Acquire)
            && !s.load(Ordering::Acquire)
            && !abort.load(Ordering::Acquire)
        {
            std::thread::sleep(Duration::from_millis(4));
        }
        if guardian.pulse().is_err() {
            abort.store(true, Ordering::Release);
        }
        initialized_worker.store(!abort.load(Ordering::Acquire), Ordering::Release);
        let release = |ledger: &mut crossdesk_core::receiver::Receiver,
                       guardian: &mut crate::guard::Guard|
         -> bool {
            let mut success = true;
            for event in ledger.release() {
                if guardian.inject(&event).is_err() {
                    success = false;
                    match event {
                        InputEvent::Key { key, .. } => ledger.committed(&InputEvent::Key {
                            key,
                            down: true,
                            repeat: false,
                        }),
                        InputEvent::Button { button, .. } => {
                            ledger.committed(&InputEvent::Button { button, down: true })
                        }
                        _ => {}
                    }
                }
            }
            success
        };
        let mut following = None;
        while !s.load(Ordering::Acquire) {
            let now = start.elapsed().as_millis() as u64;
            if ledger.expired(now) || abort.load(Ordering::Acquire) {
                release(&mut ledger, &mut guardian);
                active.store(false, Ordering::Release);
                abort.store(true, Ordering::Release);
                break; // A terminated lease requires fresh preparation, never a late revival.
            }
            let Some(cmd) = following
                .take()
                .or_else(|| rx.recv_timeout(Duration::from_millis(8)).ok())
            else {
                continue;
            };
            let now = start.elapsed().as_millis() as u64;
            match cmd {
                ReceiverCommand::Activate {
                    source,
                    session,
                    epoch,
                    position,
                    reply,
                } => {
                    let result = if abort.load(Ordering::Acquire)
                        || !ready.load(Ordering::Acquire)
                        || !platform::physical_idle()
                    {
                        Err(anyhow::anyhow!("receiver not ready or local input is held; release keys/buttons and restart sharing"))
                    } else {
                        ledger
                            .activate(source, session, epoch, now)
                            .map_err(anyhow::Error::msg)
                            .and_then(|_| {
                                guardian.pulse().and_then(|_| {
                                    guardian.inject(&InputEvent::Move {
                                        position,
                                        topology_version: topology.version,
                                    })
                                })
                            })
                    };
                    if result.is_err() {
                        release(&mut ledger, &mut guardian);
                    } else {
                        active.store(true, Ordering::Release);
                    }
                    let _ = reply.send(result);
                }
                ReceiverCommand::Input(queued, p) => {
                    let result = crate::input_delivery::prepare_input(
                        queued,
                        p,
                        || rx.try_recv().ok(),
                        |queued, p| {
                            anyhow::ensure!(
                                !s.load(Ordering::Acquire) && !abort.load(Ordering::Acquire),
                                "receiver stopped"
                            );
                            ledger
                                .validate(
                                    p,
                                    start.elapsed().as_millis() as u64,
                                    queued.elapsed().as_millis() as u64,
                                    topology.version,
                                )
                                .map_err(anyhow::Error::msg)?;
                            if let InputEvent::Move { position, .. } = &p.event {
                                anyhow::ensure!(
                                    topology.monitors.iter().any(|m| m.enabled
                                        && m.mirror_of.is_none()
                                        && m.rect.contains(*position)),
                                    "invalid target screen position"
                                );
                            }
                            Ok(())
                        },
                    )
                    .and_then(|ready| {
                        following = ready.following;
                        anyhow::ensure!(
                            !s.load(Ordering::Acquire) && !abort.load(Ordering::Acquire),
                            "receiver stopped"
                        );
                        guardian.inject(&ready.packet.event)?;
                        ledger.committed(&ready.packet.event);
                        Ok(())
                    });
                    if result.is_err() {
                        following = None;
                        release(&mut ledger, &mut guardian);
                        abort.store(true, Ordering::Release);
                    }
                }
                ReceiverCommand::Heartbeat(source, session, epoch) => {
                    if ledger.heartbeat(&source, session, epoch, now) && guardian.pulse().is_err() {
                        abort.store(true, Ordering::Release);
                    }
                }
                ReceiverCommand::Release(reply) => {
                    let ok = release(&mut ledger, &mut guardian);
                    if !ok {
                        abort.store(true, Ordering::Release);
                    }
                    active.store(false, Ordering::Release);
                    let _ = reply.send(if ok {
                        Ok(())
                    } else {
                        Err(anyhow::anyhow!(
                            "native key release failed; refusing control transfer"
                        ))
                    });
                }
            }
        }
        release(&mut ledger, &mut guardian);
    });
    ReceiverHandle {
        tx,
        ready: initialized,
        interrupted,
        stop,
        join: Some(join),
        capture_join: Some(capture_join),
    }
}
