pub const INJECTION_TAG: usize = 0x4352444b;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct InputPermissions {
    pub accessibility: bool,
    pub input_monitoring: bool,
    pub post_event: bool,
}
impl InputPermissions {
    // Accessibility subsumes Quartz event listening and posting. The three
    // preflights describe overlapping grants, not three required switches.
    // https://developer.apple.com/forums/thread/828052
    pub fn can_listen(&self) -> bool {
        self.accessibility || self.input_monitoring
    }
    pub fn can_post(&self) -> bool {
        self.accessibility || self.post_event
    }
    pub fn ready(&self) -> bool {
        self.can_listen() && self.can_post()
    }
    pub fn check(&self) -> anyhow::Result<()> {
        let mut missing = vec![];
        if !self.can_post() {
            missing.push("辅助功能");
        }
        if !self.can_listen() {
            missing.push("输入监控");
        }
        anyhow::ensure!(
            missing.is_empty(),
            "当前键鼠后台尚未取得{}权限",
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
    fn quartz_listen_and_post_grants_are_ready_without_ax_trust() {
        // rc22 on the user's Mac after repairing stale TCC records.
        let permissions = InputPermissions {
            accessibility: false,
            input_monitoring: true,
            post_event: true,
        };
        assert!(permissions.can_listen());
        assert!(permissions.can_post());
        assert!(permissions.ready());
        assert!(permissions.check().is_ok());
    }
    #[test]
    fn accessibility_covers_listening_and_posting_without_separate_grants() {
        for accessibility in [true, false] {
            for input_monitoring in [false, true] {
                for post_event in [false, true] {
                    let checks = InputPermissions {
                        accessibility,
                        input_monitoring,
                        post_event,
                    };
                    let ready = accessibility || (input_monitoring && post_event);
                    assert_eq!(checks.ready(), ready, "{checks:?}");
                    assert_eq!(checks.check().is_ok(), ready, "{checks:?}");
                    if let Err(error) = checks.check() {
                        let error = error.to_string();
                        assert_eq!(
                            error.contains("输入监控"),
                            !accessibility && !input_monitoring
                        );
                        assert_eq!(error.contains("辅助功能"), !accessibility && !post_event);
                    }
                }
            }
        }
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
