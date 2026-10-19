use crate::*;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Phase {
    LocalActive,
    PreparingSwitch,
    RemoteActive,
    Paused,
}
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Deliver {
        target: DeviceId,
        event: InputEvent,
    },
    Prepare {
        target: DeviceId,
        epoch: u64,
        position: Point,
    },
    Activate {
        target: DeviceId,
        epoch: u64,
        position: Point,
    },
    Release {
        target: DeviceId,
        epoch: u64,
    },
    LocalRelease {
        keys: Vec<PhysicalKey>,
        buttons: Vec<u8>,
    },
    Recovered,
}
#[derive(Debug, Default)]
pub struct Decision {
    pub suppress: bool,
    pub actions: Vec<Action>,
}
pub struct ControlSession {
    pub source: DeviceId,
    pub active: DeviceId,
    pub phase: Phase,
    pub epoch: u64,
    pub position: Point,
    pub layout: Layout,
    physical: BTreeSet<PhysicalKey>,
    buttons: BTreeSet<u8>,
    delivered_local: BTreeSet<PhysicalKey>,
    quarantined_keys: BTreeSet<PhysicalKey>,
    quarantined_buttons: BTreeSet<u8>,
    consumed_switch: bool,
    pending: Option<(Route, u64)>,
    last_switch: Option<u64>,
    pub tuning: Tuning,
    /// Includes group reservation and cold receiver preparation; watchdogs remain independent.
    pub prepare_timeout_ms: u64,
    edge_candidate: Option<(DeviceId, u64)>,
}
impl ControlSession {
    pub fn new(source: DeviceId, layout: Layout) -> Result<Self, String> {
        layout.validate()?;
        if !layout.ready(&source) {
            return Err("source not ready".into());
        }
        Ok(Self {
            active: source.clone(),
            source,
            phase: Phase::LocalActive,
            epoch: 0,
            position: Point::default(),
            layout,
            physical: BTreeSet::new(),
            buttons: BTreeSet::new(),
            delivered_local: BTreeSet::new(),
            quarantined_keys: BTreeSet::new(),
            quarantined_buttons: BTreeSet::new(),
            consumed_switch: false,
            pending: None,
            last_switch: None,
            tuning: Tuning::default(),
            prepare_timeout_ms: 500,
            edge_candidate: None,
        })
    }
    pub fn remote(&self) -> bool {
        self.active != self.source
    }
    fn ctrl(&self) -> bool {
        self.physical.iter().any(|k| k.is_ctrl())
    }
    fn switch_held(&self) -> bool {
        self.physical
            .iter()
            .any(|k| self.tuning.switch_modifier.matches(*k))
    }
    pub fn held(&self) -> bool {
        !self.physical.is_empty() || !self.buttons.is_empty()
    }
    pub fn input(&mut self, mut event: InputEvent) -> Decision {
        if let InputEvent::Key { key, down, repeat } = &mut event {
            *repeat = *down && self.physical.contains(key);
        }
        let mut d = Decision {
            suppress: self.remote() || self.phase == Phase::PreparingSwitch,
            actions: vec![],
        };
        match &event {
            InputEvent::Key { key, down, .. } => {
                if *down {
                    self.physical.insert(*key);
                } else {
                    self.physical.remove(key);
                }
                if *down
                    && *key == self.tuning.emergency_key
                    && self.ctrl()
                    && self.physical.iter().any(|k| k.is_alt())
                {
                    d.actions = self.recover();
                    d.suppress = true;
                    return d;
                }
                if self.tuning.switch_modifier.matches(*key) && self.consumed_switch {
                    d.suppress = true;
                    if !self.switch_held() {
                        self.consumed_switch = false;
                    }
                    return d;
                }
                if self.quarantined_keys.contains(key) {
                    if !*down {
                        self.quarantined_keys.remove(key);
                    }
                    d.suppress = true;
                    return d;
                }
                if self.phase == Phase::PreparingSwitch
                    && *down
                    && !self.tuning.switch_modifier.matches(*key)
                {
                    self.quarantined_keys.insert(*key);
                }
            }
            InputEvent::Button { button, down } => {
                if *down {
                    self.buttons.insert(*button);
                } else {
                    self.buttons.remove(button);
                }
                if self.quarantined_buttons.contains(button) {
                    if !*down {
                        self.quarantined_buttons.remove(button);
                    }
                    d.suppress = true;
                    return d;
                }
                if self.phase == Phase::PreparingSwitch && *down {
                    self.quarantined_buttons.insert(*button);
                }
            }
            _ => {}
        }
        if self.phase == Phase::PreparingSwitch {
            return d;
        }
        if self.phase == Phase::Paused {
            d.suppress = false;
            return d;
        }
        if !self.remote() {
            if let InputEvent::Key { key, down, .. } = &event {
                if *down {
                    self.delivered_local.insert(*key);
                } else {
                    self.delivered_local.remove(key);
                }
            }
        }
        d.actions.push(Action::Deliver {
            target: self.active.clone(),
            event,
        });
        d
    }
    pub fn motion(&mut self, from: Point, delta: Point, now_ms: u64) -> Decision {
        let mut d = Decision {
            suppress: self.remote() || self.phase == Phase::PreparingSwitch,
            actions: vec![],
        };
        if matches!(self.phase, Phase::Paused | Phase::PreparingSwitch) {
            return d;
        }
        self.position = from;
        let delta = if self.remote() {
            Point {
                x: delta.x * self.tuning.mouse_speed,
                y: delta.y * self.tuning.mouse_speed,
            }
        } else {
            delta
        };
        let allowed = (self.tuning.switch_modifier == SwitchModifier::None || self.switch_held())
            && self
                .physical
                .iter()
                .all(|k| self.tuning.switch_modifier.matches(*k))
            && self.buttons.is_empty()
            && self
                .last_switch
                .is_none_or(|t| now_ms.saturating_sub(t) >= 180);
        if let Some(route) = self.layout.route(&self.active, from, delta, allowed) {
            if self.tuning.edge_wait_ms > 0 {
                let (candidate, since) = self
                    .edge_candidate
                    .get_or_insert((route.device.clone(), now_ms));
                if candidate != &route.device {
                    *candidate = route.device.clone();
                    *since = now_ms;
                }
                if now_ms.saturating_sub(*since) < self.tuning.edge_wait_ms {
                    return d;
                }
            }
            self.edge_candidate = None;
            self.phase = Phase::PreparingSwitch;
            self.epoch += 1;
            self.pending = Some((
                route.clone(),
                now_ms + self.prepare_timeout_ms.clamp(500, 1500),
            ));
            d.suppress = true;
            d.actions.push(Action::Prepare {
                target: route.device,
                epoch: self.epoch,
                position: route.position,
            });
            return d;
        }
        self.edge_candidate = None;
        if self.remote() {
            if let Some(p) = self.layout.move_within(&self.active, from, delta) {
                self.position = p;
                d.actions.push(Action::Deliver {
                    target: self.active.clone(),
                    event: InputEvent::Move {
                        position: p,
                        topology_version: self.layout.topology(&self.active).unwrap().version,
                    },
                });
            }
        }
        d
    }
    /// Hand the local release ledger to a worker without activating the target.
    /// While its completion is pending, PreparingSwitch keeps new input quarantined.
    pub fn take_local_release_for_prepared(&mut self, target: &str, epoch: u64) -> Option<Action> {
        if self.remote()
            || self.phase != Phase::PreparingSwitch
            || epoch != self.epoch
            || self
                .pending
                .as_ref()
                .is_none_or(|(route, _)| route.device != target)
        {
            return None;
        }
        Some(Action::LocalRelease {
            keys: std::mem::take(&mut self.delivered_local)
                .into_iter()
                .collect(),
            buttons: vec![],
        })
    }
    pub fn prepared(&mut self, target: &str, epoch: u64, now_ms: u64) -> Vec<Action> {
        let Some((route, deadline)) = self.pending.clone() else {
            return vec![];
        };
        if route.device != target || epoch != self.epoch {
            return vec![];
        }
        if now_ms >= deadline || !self.layout.ready(target) {
            return self.recover();
        }
        let mut actions = vec![Action::Release {
            target: self.active.clone(),
            epoch: self.epoch.saturating_sub(1),
        }];
        if !self.remote() {
            actions.push(Action::LocalRelease {
                keys: self.delivered_local.iter().copied().collect(),
                buttons: vec![],
            });
            self.delivered_local.clear();
        }
        self.consumed_switch = self.switch_held();
        self.active = target.to_owned();
        self.position = route.position;
        self.phase = if self.remote() {
            Phase::RemoteActive
        } else {
            Phase::LocalActive
        };
        self.pending = None;
        self.last_switch = Some(now_ms);
        actions.push(Action::Activate {
            target: target.to_owned(),
            epoch,
            position: route.position,
        });
        actions
    }
    pub fn tick(&mut self, now_ms: u64) -> Vec<Action> {
        if self.pending.as_ref().is_some_and(|(_, t)| now_ms >= *t) {
            self.recover()
        } else {
            vec![]
        }
    }
    pub fn recover(&mut self) -> Vec<Action> {
        if self.remote() || self.phase == Phase::PreparingSwitch {
            self.quarantined_keys.extend(
                self.physical
                    .iter()
                    .copied()
                    .filter(|k| !self.tuning.switch_modifier.matches(*k)),
            );
            self.quarantined_buttons
                .extend(self.buttons.iter().copied());
        }
        let mut actions = vec![Action::Release {
            target: self.active.clone(),
            epoch: self.epoch,
        }];
        if let Some((r, _)) = self.pending.take() {
            actions.push(Action::Release {
                target: r.device,
                epoch: self.epoch,
            });
        }
        actions.push(Action::LocalRelease {
            keys: self.delivered_local.iter().copied().collect(),
            buttons: vec![],
        });
        self.delivered_local.clear();
        self.consumed_switch = self.switch_held();
        self.active = self.source.clone();
        self.phase = Phase::Paused;
        self.epoch += 1;
        actions.push(Action::Recovered);
        actions
    }
    pub fn resume(&mut self) -> Result<(), String> {
        if self.held() {
            return Err("release all physical keys and buttons before resuming".into());
        }
        self.phase = Phase::LocalActive;
        self.consumed_switch = false;
        Ok(())
    }
}
