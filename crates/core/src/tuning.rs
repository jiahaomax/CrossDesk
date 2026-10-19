use crate::*;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Tuning {
    pub mouse_hz: u32,
    pub motion_wait_ms: u64,
    pub edge_wait_ms: u64,
    pub heartbeat_ms: u64,
    pub lease_ms: u64,
    pub emergency_key: PhysicalKey,
}
impl Default for Tuning {
    fn default() -> Self {
        Self {
            mouse_hz: 250,
            motion_wait_ms: 0,
            edge_wait_ms: 0,
            heartbeat_ms: 100,
            lease_ms: 750,
            emergency_key: PhysicalKey::ESC,
        }
    }
}
impl Tuning {
    pub fn validate(&self) -> Result<(), String> {
        if ![125, 250, 500].contains(&self.mouse_hz)
            || self.motion_wait_ms > 20
            || self.edge_wait_ms > 300
            || !(50..=200).contains(&self.heartbeat_ms)
            || !(500..=2000).contains(&self.lease_ms)
            || ![PhysicalKey::ESC, PhysicalKey(0x45)].contains(&self.emergency_key)
        {
            return Err("invalid timing or emergency-key settings".into());
        }
        Ok(())
    }
}
/// Every raw delta updates the session cursor before scheduling. Only adjacent absolute moves coalesce.
pub struct MotionBatcher {
    tuning: Tuning,
    pending: Option<(Action, u64)>,
    last_send: u64,
}
impl MotionBatcher {
    pub fn new(tuning: Tuning) -> Self {
        Self {
            tuning,
            pending: None,
            last_send: 0,
        }
    }
    fn flush(&mut self, now: u64, out: &mut Vec<Action>) {
        if let Some((a, _)) = self.pending.take() {
            out.push(a);
            self.last_send = now;
        }
    }
    pub fn process(&mut self, actions: Vec<Action>, now: u64) -> Vec<Action> {
        let mut out = vec![];
        for action in actions {
            if matches!(action, Action::Recovered) {
                self.pending = None;
                out.clear();
                out.push(action);
                continue;
            }
            if let Action::Deliver {
                target,
                event: InputEvent::Move { .. },
            } = &action
            {
                let same=self.pending.as_ref().is_some_and(|(a,_)|matches!(a,Action::Deliver{target:t,event:InputEvent::Move{..}} if t==target));
                if !same {
                    self.flush(now, &mut out);
                    self.pending = Some((action, now));
                } else if let Some((old, _)) = &mut self.pending {
                    *old = action;
                }
            } else {
                self.flush(now, &mut out);
                out.push(action);
            }
        }
        if self.pending.as_ref().is_some_and(|(_, first)| {
            now.saturating_sub(*first) >= self.tuning.motion_wait_ms
                && now.saturating_sub(self.last_send) >= 1000 / self.tuning.mouse_hz as u64
        }) {
            self.flush(now, &mut out);
        }
        out
    }
}
