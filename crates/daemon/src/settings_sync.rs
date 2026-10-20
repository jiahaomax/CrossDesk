//! Last saved preference revision converges across authenticated peers, without wall clocks.
//! Mouse speed, device addresses, trust, sharing state and permissions stay on this computer.
use crate::config::Config;
use anyhow::{ensure, Result};
use crossdesk_transport::SharedSettings;

pub fn snapshot(config: &Config) -> SharedSettings {
    SharedSettings {
        revision: config.shared_settings.as_ref().map_or(0, |s| s.revision),
        author: config
            .shared_settings
            .as_ref()
            .map_or_else(String::new, |s| s.author.clone()),
        tuning: (&config.tuning).into(),
        same_subnet_only: config.same_subnet_only,
        appearance: config.appearance.clone(),
    }
}
pub fn record_changes(next: &mut Config, old: &Config, local: &str) -> Result<()> {
    next.shared_settings = old.shared_settings.clone();
    if crossdesk_core::SharedTuning::from(&next.tuning)
        != crossdesk_core::SharedTuning::from(&old.tuning)
        || next.same_subnet_only != old.same_subnet_only
        || next.appearance != old.appearance
    {
        let mut value = snapshot(next);
        value.revision = value
            .revision
            .checked_add(1)
            .filter(|n| *n < u64::MAX)
            .ok_or_else(|| anyhow::anyhow!("设置版本已达上限"))?;
        value.author = local.to_owned();
        next.shared_settings = Some(value);
    }
    Ok(())
}
pub fn merge(config: &Config, incoming: &SharedSettings) -> Result<Option<Config>> {
    ensure!(
        incoming.revision < u64::MAX && incoming.author.len() <= 128,
        "设置版本无效"
    );
    incoming
        .tuning
        .with_local_speed(config.tuning.mouse_speed)
        .validate()
        .map_err(anyhow::Error::msg)?;
    if incoming.revision == 0 {
        return Ok(None);
    } // Unedited defaults do not overwrite saved local preferences.
    ensure!(!incoming.author.is_empty(), "设置缺少作者");
    let previous = snapshot(config);
    let key = |s: &SharedSettings| (s.revision, s.author.clone());
    if key(incoming) < key(&previous) {
        return Ok(None);
    }
    if key(incoming) == key(&previous) {
        ensure!(incoming == &previous, "相同设置版本的内容不一致");
        return Ok(None);
    }
    let mut next = config.clone();
    next.tuning = incoming.tuning.with_local_speed(config.tuning.mouse_speed);
    next.same_subnet_only = incoming.same_subnet_only;
    next.appearance = incoming.appearance.clone();
    next.shared_settings = Some(incoming.clone());
    next.validate()?;
    Ok(Some(next))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_preferences_converge_relay_and_preserve_local_connection_data() {
        let original = Config::default();
        let mut a = original.clone();
        a.tuning.mouse_speed = 2.;
        a.tuning.switch_modifier = crossdesk_core::SwitchModifier::Shift;
        a.appearance.dark = Some(true);
        record_changes(&mut a, &original, "A").unwrap();
        let b = Config {
            port: 30123,
            sharing: true,
            ..original.clone()
        };
        let merged = merge(&b, &snapshot(&a)).unwrap().unwrap();
        assert_eq!(merged.port, 30123);
        assert!(merged.sharing);
        assert_eq!(merged.tuning.mouse_speed, 1.);
        assert_eq!(snapshot(&merged).tuning, snapshot(&a).tuning);
        assert_eq!(merged.appearance.dark, Some(true));
        let c = merge(&original, &snapshot(&merged)).unwrap().unwrap();
        assert_eq!(snapshot(&c), snapshot(&a));
        assert!(merge(&c, &snapshot(&original)).unwrap().is_none());
        let mut edit = merged.clone();
        edit.tuning.mouse_speed = 1.5;
        record_changes(&mut edit, &merged, "B").unwrap();
        assert_eq!(
            snapshot(&edit),
            snapshot(&merged),
            "local speed does not create a shared revision"
        );
        assert!(merge(&a, &snapshot(&edit)).unwrap().is_none());
        let restored: Config = serde_json::from_slice(&serde_json::to_vec(&edit).unwrap()).unwrap();
        assert_eq!(snapshot(&restored), snapshot(&edit));
        assert_eq!(restored.tuning.mouse_speed, 1.5);
        assert!(serde_json::to_value(snapshot(&edit)).unwrap()["tuning"]
            .get("mouse_speed")
            .is_none());
    }
    #[test]
    fn simultaneous_edits_have_one_winner_and_invalid_settings_are_rejected() {
        let base = Config::default();
        let mut a = base.clone();
        a.tuning.mouse_speed = 2.;
        a.tuning.switch_modifier = crossdesk_core::SwitchModifier::Shift;
        let mut b = base.clone();
        b.tuning.mouse_speed = 3.;
        b.tuning.switch_modifier = crossdesk_core::SwitchModifier::Keys(vec![
            crossdesk_core::PhysicalKey(0xe2),
            crossdesk_core::PhysicalKey(0x0e),
        ]);
        record_changes(&mut a, &base, "A").unwrap();
        record_changes(&mut b, &base, "B").unwrap();
        assert!(merge(&b, &snapshot(&a)).unwrap().is_none());
        assert_eq!(
            merge(&a, &snapshot(&b))
                .unwrap()
                .unwrap()
                .tuning
                .mouse_speed,
            2.
        );
        let mut bad = snapshot(&b);
        bad.tuning.heartbeat_ms = 0;
        assert!(merge(&a, &bad).is_err());
        let mut forged = base.clone();
        forged.shared_settings = b.shared_settings;
        record_changes(&mut forged, &base, "A").unwrap();
        assert!(forged.shared_settings.is_none());
    }
    #[test]
    fn legacy_shared_snapshot_does_not_restore_another_computers_speed() {
        let mut config = Config::default();
        config.tuning.mouse_speed = 2.25;
        let mut json = serde_json::to_value(&config).unwrap();
        json["shared_settings"] = serde_json::json!({
            "revision": 8, "author": "old-peer", "tuning": config.tuning,
            "same_subnet_only": true, "appearance": config.appearance
        });
        json["shared_settings"]["tuning"]["mouse_speed"] = serde_json::json!(3.5);
        let restored: Config = serde_json::from_value(json).unwrap();
        assert_eq!(restored.tuning.mouse_speed, 2.25);
        let mut incoming = snapshot(&restored);
        incoming.revision += 1;
        incoming.tuning.switch_modifier = crossdesk_core::SwitchModifier::None;
        let merged = merge(&restored, &incoming).unwrap().unwrap();
        assert_eq!(merged.tuning.mouse_speed, 2.25);
        assert_eq!(
            merged.tuning.switch_modifier,
            crossdesk_core::SwitchModifier::None
        );
    }
}
