//! Serializes callbacks that Windows can re-enter while SendInput/CallNextHookEx
//! pumps sent messages. No handler borrow is held across application code.
use crate::NativeEvent;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    panic::{catch_unwind, AssertUnwindSafe},
    time::{Duration, Instant},
};

pub(super) type Handler = Box<dyn FnMut(NativeEvent) -> bool + Send>;
const MAX_REENTRANT_EVENTS: usize = 256;

#[derive(Default)]
pub(super) struct Dispatcher {
    handler: RefCell<Option<Handler>>,
    running: Cell<bool>,
    suppress: Cell<bool>,
    pending: RefCell<VecDeque<NativeEvent>>,
    fault: Cell<Option<&'static str>>,
}

impl Dispatcher {
    pub fn install(&self, handler: Handler) {
        *self.handler.borrow_mut() = Some(handler);
        self.pending.borrow_mut().clear();
        self.running.set(false);
        self.suppress.set(false);
        self.fault.set(None);
    }

    pub fn clear(&self) {
        self.handler.borrow_mut().take();
        self.pending.borrow_mut().clear();
        self.suppress.set(false);
    }

    pub fn set_suppression(&self, suppress: bool) {
        self.suppress.set(suppress);
    }

    pub fn fault(&self) -> Option<&'static str> {
        self.fault.get()
    }

    fn fail(&self, reason: &'static str) {
        self.fault.set(Some(reason));
        self.suppress.set(false);
        self.pending.borrow_mut().clear();
    }

    pub fn dispatch(&self, event: NativeEvent) -> bool {
        if self.fault.get().is_some() && !matches!(event, NativeEvent::Shutdown) {
            return false; // Restore physical local input until hooks are removed.
        }
        // Shutdown runs after hooks are removed. Even a failed handler gets one
        // bounded, panic-isolated opportunity to release its local ledger.
        if self.running.get() {
            if self.pending.borrow().len() >= MAX_REENTRANT_EVENTS {
                self.fail("input callback re-entry queue exceeded its safety bound");
                return false;
            }
            self.pending.borrow_mut().push_back(event);
            return self.suppress.get();
        }
        let Some(mut handler) = self.handler.borrow_mut().take() else {
            return false;
        };
        self.running.set(true);
        // Never unwind through a Win32 callback. The capture loop reports the
        // fault and removes hooks; the daemon then takes its existing pause path.
        let result = catch_unwind(AssertUnwindSafe(|| {
            let decision = handler(event);
            let mut drained = 0;
            while self.fault.get().is_none() {
                let next = self.pending.borrow_mut().pop_front();
                let Some(next) = next else { break };
                if drained == MAX_REENTRANT_EVENTS {
                    self.fail("input callback re-entry did not settle");
                    break;
                }
                handler(next);
                drained += 1;
            }
            decision
        }));
        self.running.set(false);
        *self.handler.borrow_mut() = Some(handler);
        match result {
            Ok(decision) if self.fault.get().is_none() => decision,
            Ok(_) => false,
            Err(_) => {
                self.fail("input callback panicked; sharing must be paused");
                false
            }
        }
    }
}

// WM_TIMER is low priority and can be starved by continuous input. Service the
// deadline between messages instead, without a burst of catch-up ticks.
pub(super) struct TickSchedule {
    next: Instant,
    interval: Duration,
}
impl TickSchedule {
    pub fn new(now: Instant) -> Self {
        let interval = Duration::from_millis(2);
        Self {
            next: now + interval,
            interval,
        }
    }
    pub fn due(&mut self, now: Instant) -> bool {
        if now < self.next {
            return false;
        }
        self.next = now + self.interval;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossdesk_core::{InputEvent, PhysicalKey};
    use std::sync::{Arc, Mutex};
    thread_local! { static TEST_DISPATCH: Dispatcher = Dispatcher::default(); }
    fn key(down: bool) -> NativeEvent {
        NativeEvent::Input(InputEvent::Key {
            key: PhysicalKey::LEFT_CTRL,
            down,
            repeat: false,
        })
    }

    #[test]
    fn reentry_during_release_preserves_order_and_suppression() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let out = seen.clone();
        TEST_DISPATCH.with(|d| {
            d.install(Box::new(move |e| {
                match e {
                    NativeEvent::Tick => {
                        out.lock().unwrap().push("release-start");
                        TEST_DISPATCH.with(|d| {
                            d.set_suppression(true);
                            // Models physical messages arriving during native cleanup.
                            assert!(d.dispatch(key(true)));
                            assert!(d.dispatch(key(false)));
                            assert!(d.dispatch(NativeEvent::Motion {
                                delta: Default::default(),
                                local: Default::default()
                            }));
                        });
                        out.lock().unwrap().push("activate");
                    }
                    NativeEvent::Input(InputEvent::Key { down, .. }) => {
                        out.lock().unwrap().push(if down { "down" } else { "up" })
                    }
                    NativeEvent::Motion { .. } => out.lock().unwrap().push("move"),
                    _ => {}
                }
                true
            }));
            assert!(d.dispatch(NativeEvent::Tick));
            assert_eq!(
                *seen.lock().unwrap(),
                ["release-start", "activate", "down", "up", "move"]
            );
            assert_eq!(d.fault(), None);
            d.clear();
        });
    }

    #[test]
    fn local_reentry_passes_through_without_losing_events() {
        let seen = Arc::new(Mutex::new(0));
        let out = seen.clone();
        TEST_DISPATCH.with(|d| {
            d.install(Box::new(move |event| {
                if matches!(event, NativeEvent::Tick) {
                    TEST_DISPATCH.with(|d| assert!(!d.dispatch(key(false))));
                } else {
                    *out.lock().unwrap() += 1;
                }
                false
            }));
            assert!(!d.dispatch(NativeEvent::Tick));
            assert_eq!(*seen.lock().unwrap(), 1);
            d.clear();
        });
    }

    #[test]
    fn callback_panic_cannot_escape_and_local_input_passes_through() {
        let d = Dispatcher::default();
        d.install(Box::new(|_| panic!("simulated handler failure")));
        d.set_suppression(true);
        assert!(!d.dispatch(NativeEvent::Tick));
        assert!(d.fault().is_some());
        assert!(!d.dispatch(key(false)));
        d.clear();
        d.install(Box::new(|_| true));
        assert!(d.dispatch(NativeEvent::Tick));
        assert_eq!(d.fault(), None);
    }

    #[test]
    fn failure_still_runs_shutdown_cleanup_without_replaying_input() {
        let cleaned = Arc::new(Mutex::new(false));
        let out = cleaned.clone();
        let d = Dispatcher::default();
        d.install(Box::new(move |event| {
            if matches!(event, NativeEvent::Shutdown) {
                *out.lock().unwrap() = true;
            } else {
                panic!("simulated callback failure");
            }
            false
        }));
        assert!(!d.dispatch(NativeEvent::Tick));
        assert!(!d.dispatch(NativeEvent::Shutdown));
        assert!(*cleaned.lock().unwrap());
        assert!(d.fault().is_some());
    }

    #[test]
    fn nested_flood_is_bounded_and_does_not_replay() {
        TEST_DISPATCH.with(|d| {
            d.install(Box::new(|_| {
                TEST_DISPATCH.with(|d| {
                    d.set_suppression(true);
                    for _ in 0..=MAX_REENTRANT_EVENTS {
                        d.dispatch(key(true));
                    }
                });
                true
            }));
            assert!(!d.dispatch(NativeEvent::Tick));
            assert!(d.fault().is_some());
            assert!(d.pending.borrow().is_empty());
            d.clear();
        });
    }

    #[test]
    fn continuously_reentrant_handler_cannot_hang_the_capture_thread() {
        TEST_DISPATCH.with(|d| {
            d.install(Box::new(|_| {
                TEST_DISPATCH.with(|d| d.dispatch(NativeEvent::LocalActivity));
                true
            }));
            assert!(!d.dispatch(NativeEvent::Tick));
            assert_eq!(d.fault(), Some("input callback re-entry did not settle"));
            d.clear();
        });
    }

    #[test]
    fn ticks_continue_under_eight_thousand_messages_per_second() {
        let start = Instant::now();
        let mut schedule = TickSchedule::new(start);
        let ticks = (1..=8000)
            .filter(|i| schedule.due(start + Duration::from_micros(i * 125)))
            .count();
        assert_eq!(ticks, 500);
        assert!(schedule.due(start + Duration::from_secs(5)));
        assert!(!schedule.due(start + Duration::from_secs(5)));
    }
}
