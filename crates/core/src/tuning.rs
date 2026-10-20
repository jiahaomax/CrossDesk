use crate::*;
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum SwitchModifier {
    #[default]
    Ctrl,
    Shift,
    None,
    Keys(Vec<PhysicalKey>),
}
impl SwitchModifier {
    pub fn matches(&self, key: PhysicalKey) -> bool {
        match self {
            Self::Ctrl => key.is_ctrl(),
            Self::Shift => key.0 == 0xe1 || key.0 == 0xe5,
            Self::None => false,
            Self::Keys(keys) => keys
                .iter()
                .any(|expected| canonical_key(*expected) == canonical_key(key)),
        }
    }
    pub fn held(&self, physical: &std::collections::BTreeSet<PhysicalKey>) -> bool {
        match self {
            Self::None => false,
            Self::Keys(keys) => {
                !keys.is_empty()
                    && keys.iter().all(|expected| {
                        physical
                            .iter()
                            .any(|key| canonical_key(*key) == canonical_key(*expected))
                    })
            }
            _ => physical.iter().any(|key| self.matches(*key)),
        }
    }
    pub fn label(&self) -> String {
        match self {
            Self::Ctrl => "Ctrl + 边缘".into(),
            Self::Shift => "Shift + 边缘".into(),
            Self::None => "直接移向边缘".into(),
            Self::Keys(keys) => format!(
                "{} + 边缘",
                keys.iter()
                    .map(|key| key_label(*key))
                    .collect::<Vec<_>>()
                    .join(" + ")
            ),
        }
    }
}
fn canonical_key(key: PhysicalKey) -> PhysicalKey {
    if (0xe4..=0xe7).contains(&key.0) {
        PhysicalKey(key.0 - 4)
    } else {
        key
    }
}
fn key_label(key: PhysicalKey) -> String {
    match canonical_key(key).0 {
        n @ 4..=29 => ((b'A' + (n - 4) as u8) as char).to_string(),
        n @ 30..=38 => (n - 29).to_string(),
        39 => "0".into(),
        n @ 58..=69 => format!("F{}", n - 57),
        0xe0 => "Ctrl".into(),
        0xe1 => "Shift".into(),
        0xe2 => "Alt / Option".into(),
        0xe3 => "Win / Command".into(),
        40 => "Enter".into(),
        41 => "Esc".into(),
        42 => "Backspace".into(),
        43 => "Tab".into(),
        44 => "Space".into(),
        79 => "→".into(),
        80 => "←".into(),
        81 => "↓".into(),
        82 => "↑".into(),
        n => format!("键 {n}"),
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
        if let SwitchModifier::Keys(keys) = &self.switch_modifier {
            let unique: std::collections::BTreeSet<_> =
                keys.iter().map(|key| canonical_key(*key)).collect();
            if keys.is_empty()
                || keys.len() > 6
                || unique.len() != keys.len()
                || keys
                    .iter()
                    .any(|key| !matches!(key.0, 4..=69 | 71 | 73..=101 | 224..=231))
            {
                return Err("切换按键应为 1–6 个不同的常规键盘按键".into());
            }
            if unique.contains(&PhysicalKey::LEFT_CTRL)
                && unique.contains(&PhysicalKey::LEFT_ALT)
                && unique.contains(&self.emergency_key)
            {
                return Err("切换按键不能包含当前紧急释放快捷键".into());
            }
        }
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

/// Only these preferences travel between peers. Mouse speed belongs to the input source.
/// Old JSON configuration snapshots may contain mouse_speed; serde ignores that legacy field.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SharedTuning {
    pub mouse_hz: u32,
    pub switch_modifier: SwitchModifier,
    pub motion_wait_ms: u64,
    pub edge_wait_ms: u64,
    pub heartbeat_ms: u64,
    pub lease_ms: u64,
    pub emergency_key: PhysicalKey,
}
impl From<&Tuning> for SharedTuning {
    fn from(value: &Tuning) -> Self {
        Self {
            mouse_hz: value.mouse_hz,
            switch_modifier: value.switch_modifier.clone(),
            motion_wait_ms: value.motion_wait_ms,
            edge_wait_ms: value.edge_wait_ms,
            heartbeat_ms: value.heartbeat_ms,
            lease_ms: value.lease_ms,
            emergency_key: value.emergency_key,
        }
    }
}
impl SharedTuning {
    pub fn with_local_speed(&self, mouse_speed: f64) -> Tuning {
        Tuning {
            mouse_speed,
            mouse_hz: self.mouse_hz,
            switch_modifier: self.switch_modifier.clone(),
            motion_wait_ms: self.motion_wait_ms,
            edge_wait_ms: self.edge_wait_ms,
            heartbeat_ms: self.heartbeat_ms,
            lease_ms: self.lease_ms,
            emergency_key: self.emergency_key,
        }
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
