use crate::*;
use std::collections::BTreeSet;

/// Only software-owned presses are released. A new activation never inherits a previous ledger.
#[derive(Default)]
pub struct Receiver {
    owner: Option<(DeviceId, u64, u64)>,
    sequence: u64,
    deadline: u64,
    lease_ms: u64,
    pub keys: BTreeSet<PhysicalKey>,
    pub buttons: BTreeSet<u8>,
}
impl Receiver {
    fn lease_duration(&self) -> u64 {
        if self.lease_ms == 0 {
            750
        } else {
            self.lease_ms
        }
    }
    pub fn set_lease(&mut self, lease: u64) -> Result<(), String> {
        if !(500..=2000).contains(&lease) {
            return Err("unsafe lease duration".into());
        }
        self.lease_ms = lease;
        Ok(())
    }
    pub fn activate(
        &mut self,
        source: DeviceId,
        session: u64,
        epoch: u64,
        now: u64,
    ) -> Result<(), String> {
        if self.owner.is_some() {
            return Err("already controlled".into());
        }
        self.owner = Some((source, session, epoch));
        self.sequence = 0;
        self.deadline = now + self.lease_duration();
        Ok(())
    }
    pub fn heartbeat(&mut self, source: &str, session: u64, epoch: u64, now: u64) -> bool {
        if self
            .owner
            .as_ref()
            .is_some_and(|(s, i, e)| s == source && *i == session && *e == epoch)
            && now < self.deadline
        {
            self.deadline = now + self.lease_duration();
            true
        } else {
            false
        }
    }
    pub fn validate(
        &mut self,
        p: &InputPacket,
        now: u64,
        queued_ms: u64,
        topology: u64,
    ) -> Result<(), String> {
        if now >= self.deadline
            || queued_ms >= self.lease_duration()
            || !self
                .owner
                .as_ref()
                .is_some_and(|(s, i, e)| s == &p.source && *i == p.session && *e == p.epoch)
            || p.sequence <= self.sequence
        {
            return Err("stale or unauthorized input".into());
        }
        match p.event {
            InputEvent::Key {
                key, down: false, ..
            } if !self.keys.contains(&key) => return Err("unmatched key release".into()),
            InputEvent::Button {
                button,
                down: false,
            } if !self.buttons.contains(&button) => return Err("unmatched button release".into()),
            InputEvent::Move {
                position,
                topology_version,
            } if topology_version != topology
                || !position.x.is_finite()
                || !position.y.is_finite() =>
            {
                return Err("invalid topology or coordinate".into())
            }
            InputEvent::Button { button, .. } if !(1..=5).contains(&button) => {
                return Err("invalid button".into())
            }
            _ => {}
        }
        self.sequence = p.sequence;
        Ok(())
    }
    /// Called after successful native injection, not after merely receiving a packet.
    pub fn committed(&mut self, event: &InputEvent) {
        match event {
            InputEvent::Key { key, down, .. } => {
                if *down {
                    self.keys.insert(*key);
                } else {
                    self.keys.remove(key);
                }
            }
            InputEvent::Button { button, down } => {
                if *down {
                    self.buttons.insert(*button);
                } else {
                    self.buttons.remove(button);
                }
            }
            _ => {}
        }
    }
    pub fn expired(&self, now: u64) -> bool {
        self.owner.is_some() && now >= self.deadline
    }
    pub fn active(&self) -> bool {
        self.owner.is_some()
    }
    pub fn release(&mut self) -> Vec<InputEvent> {
        self.owner = None;
        self.sequence = 0;
        let mut result: Vec<_> = std::mem::take(&mut self.keys)
            .into_iter()
            .map(|key| InputEvent::Key {
                key,
                down: false,
                repeat: false,
            })
            .collect();
        result.extend(std::mem::take(&mut self.buttons).into_iter().map(|button| {
            InputEvent::Button {
                button,
                down: false,
            }
        }));
        result
    }
}
