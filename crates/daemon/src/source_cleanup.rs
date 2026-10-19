//! SendInput must not run (or be waited on) on the low-level hook thread.
//! A release-only worker acknowledges completion; activation stays behind this barrier.
use anyhow::{bail, ensure, Result};
use crossdesk_core::{Action, ControlSession, InputEvent};
use std::{sync::mpsc, thread::JoinHandle};

const CLEANUP_TIMEOUT_MS: u64 = 150;

pub struct LocalCleanup {
    commands: mpsc::SyncSender<(u64, Vec<InputEvent>)>,
    replies: mpsc::Receiver<(u64, bool)>,
    serial: u64,
}
impl LocalCleanup {
    pub fn spawn(
        mut inject: impl FnMut(&InputEvent) -> Result<()> + Send + 'static,
    ) -> (Self, JoinHandle<()>) {
        let (commands, requests) = mpsc::sync_channel::<(u64, Vec<InputEvent>)>(2);
        let (replies, completed) = mpsc::sync_channel(2);
        let join = std::thread::spawn(move || {
            while let Ok((id, events)) = requests.recv() {
                let mut success = true;
                for event in events {
                    // Even if one release fails, attempt the rest of our ledger.
                    if inject(&event).is_err() {
                        success = false;
                    }
                }
                if replies.try_send((id, success)).is_err() {
                    break;
                }
            }
        });
        (
            Self {
                commands,
                replies: completed,
                serial: 0,
            },
            join,
        )
    }
    pub fn submit(&mut self, action: Action) -> Result<Option<u64>> {
        let Action::LocalRelease { keys, buttons } = action else {
            bail!("local cleanup accepts release actions only");
        };
        let events: Vec<_> = keys
            .into_iter()
            .map(|key| InputEvent::Key {
                key,
                down: false,
                repeat: false,
            })
            .chain(buttons.into_iter().map(|button| InputEvent::Button {
                button,
                down: false,
            }))
            .collect();
        if events.is_empty() {
            return Ok(None);
        }
        self.serial += 1;
        self.commands.try_send((self.serial, events))?;
        Ok(Some(self.serial))
    }
    fn poll(&self) -> Result<Option<(u64, bool)>> {
        match self.replies.try_recv() {
            Ok(reply) => Ok(Some(reply)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => bail!("local release worker stopped"),
        }
    }
}

#[derive(Default)]
pub struct ActivationBarrier {
    pending: Option<(u64, String, u64, u64)>,
}
impl ActivationBarrier {
    pub fn ready(
        &mut self,
        session: &mut ControlSession,
        cleanup: &mut LocalCleanup,
        target: String,
        epoch: u64,
        now: u64,
    ) -> Result<Vec<Action>> {
        if let Some((_, expected, generation, _)) = &self.pending {
            ensure!(
                expected == &target && *generation == epoch,
                "conflicting activation during cleanup"
            );
            return Ok(vec![]); // Duplicate readiness cannot bypass the release acknowledgement.
        }
        if let Some(action) = session.take_local_release_for_prepared(&target, epoch) {
            if let Some(id) = cleanup.submit(action)? {
                self.pending = Some((id, target, epoch, now + CLEANUP_TIMEOUT_MS));
                return Ok(vec![]);
            }
        }
        Ok(session.prepared(&target, epoch, now))
    }
    pub fn poll(
        &mut self,
        session: &mut ControlSession,
        cleanup: &LocalCleanup,
        now: u64,
    ) -> Result<Vec<Action>> {
        let mut actions = vec![];
        while let Some((id, success)) = cleanup.poll()? {
            ensure!(success, "local key release failed");
            if self
                .pending
                .as_ref()
                .is_some_and(|(expected, _, _, _)| *expected == id)
            {
                let (_, target, epoch, deadline) = self.pending.take().unwrap();
                ensure!(now < deadline, "local key release acknowledgement expired");
                // prepared also checks the session epoch, topology and original prepare deadline.
                actions.extend(session.prepared(&target, epoch, now));
            }
        }
        ensure!(
            self.pending
                .as_ref()
                .is_none_or(|(_, _, _, deadline)| now < *deadline),
            "local key release timed out"
        );
        Ok(actions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossdesk_core::*;
    use std::{
        sync::{Arc, Mutex},
        time::Duration,
    };

    fn preparing() -> ControlSession {
        let mut layout = Layout::default();
        for id in ["A", "B"] {
            layout.topologies.push(DeviceTopology {
                device: Device {
                    id: id.into(),
                    name: id.into(),
                    os: OperatingSystem::Windows,
                    online: true,
                    authorized: true,
                    permission: PermissionState::Granted,
                },
                version: 1,
                monitors: vec![Monitor {
                    id: "main".into(),
                    device: id.into(),
                    name: "Main".into(),
                    primary: true,
                    rect: Rect {
                        x: 0.,
                        y: 0.,
                        width: 1000.,
                        height: 1000.,
                    },
                    resolution: [1000, 1000],
                    scale: 1.,
                    rotation: 0,
                    enabled: true,
                    mirror_of: None,
                    unit: CoordinateUnit::PhysicalPixels,
                }],
            });
        }
        layout.links.push(EdgeLink {
            from: EdgeEnd {
                device: "A".into(),
                monitor: "main".into(),
                edge: Edge::Right,
                start: 0.,
                end: 1.,
            },
            to: EdgeEnd {
                device: "B".into(),
                monitor: "main".into(),
                edge: Edge::Left,
                start: 0.,
                end: 1.,
            },
            enabled: true,
            require_ctrl: true,
        });
        let mut session = ControlSession::new("A".into(), layout).unwrap();
        session.input(InputEvent::Key {
            key: PhysicalKey::LEFT_CTRL,
            down: true,
            repeat: false,
        });
        session.motion(Point { x: 999., y: 50. }, Point { x: 2., y: 0. }, 0);
        assert_eq!(session.phase, Phase::PreparingSwitch);
        session
    }

    #[test]
    fn worker_release_is_off_thread_and_activation_waits_for_ack() {
        let (proceed, wait) = mpsc::channel();
        let (entered, started) = mpsc::channel();
        let caller = std::thread::current().id();
        let (mut cleanup, join) = LocalCleanup::spawn(move |event| {
            assert_ne!(caller, std::thread::current().id());
            assert!(matches!(event, InputEvent::Key { down: false, .. }));
            entered.send(()).unwrap();
            wait.recv().unwrap();
            Ok(())
        });
        let mut session = preparing();
        let mut barrier = ActivationBarrier::default();
        assert!(barrier
            .ready(&mut session, &mut cleanup, "B".into(), 1, 1)
            .unwrap()
            .is_empty());
        started.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(barrier
            .ready(&mut session, &mut cleanup, "B".into(), 1, 2)
            .unwrap()
            .is_empty());
        assert!(barrier.poll(&mut session, &cleanup, 2).unwrap().is_empty());
        assert!(!session.remote());
        // A key pressed while cleanup is pending is quarantined, never replayed remotely.
        let key = InputEvent::Key {
            key: PhysicalKey(4),
            down: true,
            repeat: false,
        };
        assert!(session.input(key).actions.is_empty());
        proceed.send(()).unwrap();
        let mut actions = vec![];
        for _ in 0..200 {
            actions.extend(barrier.poll(&mut session, &cleanup, 3).unwrap());
            if session.remote() {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(actions.iter().any(|a| matches!(a, Action::Activate { .. })));
        assert!(session
            .input(InputEvent::Key {
                key: PhysicalKey(4),
                down: false,
                repeat: false
            })
            .actions
            .is_empty());
        drop(cleanup);
        join.join().unwrap();
    }

    #[test]
    fn missing_or_late_ack_cannot_activate_after_recovery() {
        let (commands, requests) = mpsc::sync_channel(2);
        let (reply, replies) = mpsc::sync_channel(2);
        let mut cleanup = LocalCleanup {
            commands,
            replies,
            serial: 0,
        };
        let mut session = preparing();
        let mut barrier = ActivationBarrier::default();
        barrier
            .ready(&mut session, &mut cleanup, "B".into(), 1, 1)
            .unwrap();
        assert!(barrier.poll(&mut session, &cleanup, 151).is_err());
        session.recover();
        let (id, _) = requests.try_recv().unwrap();
        reply.send((id, true)).unwrap();
        assert!(barrier.poll(&mut session, &cleanup, 152).is_err());
        assert!(barrier.pending.is_none());
        assert_eq!(session.phase, Phase::Paused);
    }

    #[test]
    fn failed_release_still_attempts_every_owned_key_and_button() {
        let seen = Arc::new(Mutex::new(vec![]));
        let output = seen.clone();
        let (mut cleanup, join) = LocalCleanup::spawn(move |event| {
            output.lock().unwrap().push(event.clone());
            bail!("simulated OS rejection")
        });
        cleanup
            .submit(Action::LocalRelease {
                keys: vec![PhysicalKey::LEFT_CTRL, PhysicalKey::RIGHT_CTRL],
                buttons: vec![1],
            })
            .unwrap();
        let reply = cleanup
            .replies
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        assert!(!reply.1);
        assert_eq!(seen.lock().unwrap().len(), 3);
        drop(cleanup);
        join.join().unwrap();
    }
}
