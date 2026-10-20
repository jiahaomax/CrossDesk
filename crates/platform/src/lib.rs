pub const INJECTION_TAG: usize = 0x4352444b;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct InputPermissions {
    pub accessibility: bool,
    pub input_monitoring: bool,
    pub post_event: bool,
}
impl InputPermissions {
    pub fn ready(&self) -> bool {
        self.accessibility && self.input_monitoring && self.post_event
    }
    pub fn check(&self) -> anyhow::Result<()> {
        let mut missing = vec![];
        if !self.accessibility || !self.post_event {
            missing.push("辅助功能");
        }
        if !self.input_monitoring {
            missing.push("输入监控");
        }
        anyhow::ensure!(
            missing.is_empty(),
            "请先开启 Mac 的{}权限，再启用共享",
            missing.join("和")
        );
        Ok(())
    }
}
#[cfg(windows)]
pub fn input_permissions() -> Option<InputPermissions> {
    None
}
#[cfg(windows)]
pub fn open_input_permission(_kind: &str) -> anyhow::Result<()> {
    anyhow::bail!("此权限设置仅适用于 macOS")
}
pub fn ensure_input_permissions() -> anyhow::Result<()> {
    if let Some(permissions) = input_permissions() {
        permissions.check()?;
    }
    Ok(())
}

#[cfg(test)]
mod permission_tests {
    use super::*;
    #[test]
    fn all_native_checks_are_required_and_the_missing_permission_is_named() {
        for (accessibility, input_monitoring, post_event) in [
            (false, false, false),
            (true, false, true),
            (false, true, false),
            (true, true, false),
        ] {
            let checks = InputPermissions {
                accessibility,
                input_monitoring,
                post_event,
            };
            assert!(!checks.ready());
            let error = checks.check().unwrap_err().to_string();
            assert_eq!(error.contains("输入监控"), !input_monitoring);
            assert_eq!(error.contains("辅助功能"), !accessibility || !post_event);
        }
        let granted = InputPermissions {
            accessibility: true,
            input_monitoring: true,
            post_event: true,
        };
        assert!(granted.ready());
        assert!(granted.check().is_ok());
    }
}
#[derive(Debug, Clone)]
pub enum NativeEvent {
    Input(crossdesk_core::InputEvent),
    Motion {
        delta: crossdesk_core::Point,
        local: crossdesk_core::Point,
    },
    Started,
    Tick,
    Shutdown,
    LocalActivity,
    UnsupportedKey,
    TopologyChanged,
}
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(windows)]
pub mod windows;
#[cfg(target_os = "macos")]
pub use macos::*;
#[cfg(windows)]
pub use windows::*;
#[cfg(not(any(windows, target_os = "macos")))]
compile_error!("CrossDesk supports Windows and macOS only");
