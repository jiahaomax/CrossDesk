use crate::*;
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum SwitchModifier {
    #[default]
    Ctrl,
    Shift,
    None,
}
impl SwitchModifier {
    pub fn matches(self, key: PhysicalKey) -> bool {
        match self {
            Self::Ctrl => key.is_ctrl(),
            Self::Shift => key.0 == 0xe1 || key.0 == 0xe5,
            Self::None => false,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Ctrl => "Ctrl + 边缘",
            Self::Shift => "Shift + 边缘",
            Self::None => "直接移向边缘",
        }
    }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Appearance {
    pub dark: Option<bool>,
    pub debug_info: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Tuning {
    pub mouse_hz: u32,
    pub mouse_speed: f64,
    pub switch_modifier: SwitchModifier,
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
            mouse_speed: 1.,
            switch_modifier: SwitchModifier::Ctrl,
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
        if !self.mouse_speed.is_finite()
            || !(0.25..=4.).contains(&self.mouse_speed)
            || ![125, 250, 500].contains(&self.mouse_hz)
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
