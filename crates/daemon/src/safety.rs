//! Bounded diagnostics, containing causes and timings, never captured input or peer data.
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    path::Path,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Cause {
    EmergencyShortcut,
    SwitchTimeout,
    CoordinatorStalled,
    UnsupportedRemoteInput,
    TopologyChanged,
    LocalReleaseFailed,
    SourceQueueUnavailable,
    CaptureStopped,
    CleanupUnconfirmed,
    OutboundExpired,
    SessionStopped,
}
impl Cause {
    pub fn message(self) -> &'static str {
        match self {
            Self::EmergencyShortcut => "已按下紧急释放快捷键，共享已暂停。",
            Self::SwitchTimeout => "切换未及时完成，共享已暂停。松开按键后可重新启用共享。",
            Self::CoordinatorStalled => {
                "输入调度未及时响应，共享已暂停。松开按键后可重新启用共享。"
            }
            Self::UnsupportedRemoteInput => {
                "遇到暂不支持转发的输入，共享已暂停。松开按键后可重新启用共享。"
            }
            Self::TopologyChanged => "显示器布局发生变化，共享已暂停。确认布局后可重新启用共享。",
            Self::LocalReleaseFailed => {
                "未能确认本机按键已释放，共享已暂停。请完全退出 CrossDesk 后重新打开。"
            }
            Self::SourceQueueUnavailable => {
                "输入传递队列不可用，共享已暂停。松开按键后可重新启用共享。"
            }
            Self::CaptureStopped => "系统输入捕获已停止。请确认电脑已解锁，再重新启用共享。",
            Self::CleanupUnconfirmed => "输入清理尚未确认。请完全退出 CrossDesk 后重新打开。",
            Self::OutboundExpired => "输入处理停顿超过控制期限。松开按键后可重新启用共享。",
            Self::SessionStopped => "控制会话已停止，请查看界面中的原始错误详情。",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Fault {
    pub cause: Cause,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elapsed_ms: Option<u64>,
}
impl Fault {
    pub fn new(cause: Cause) -> Self {
        Self {
            cause,
            elapsed_ms: None,
        }
    }
    pub fn elapsed(cause: Cause, elapsed_ms: u64) -> Self {
        Self {
            cause,
            elapsed_ms: Some(elapsed_ms),
        }
    }
}
impl fmt::Display for Fault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.cause.message())?;
        if let Some(ms) = self.elapsed_ms {
            write!(f, "（等待 {ms} ms）")?;
        }
        Ok(())
    }
}
impl std::error::Error for Fault {}

#[derive(Clone, Default)]
pub struct FaultSlot(Arc<Mutex<Option<Fault>>>);
impl FaultSlot {
    pub fn record(&self, fault: Fault) {
        // Preserve the original cause instead of overwriting it during cleanup.
        self.0.lock().unwrap().get_or_insert(fault);
    }
    pub fn get(&self) -> Option<Fault> {
        self.0.lock().unwrap().clone()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Incident {
    pub time_unix_ms: u64,
    pub fault: Fault,
}
pub fn history(dir: &Path) -> anyhow::Result<Vec<Incident>> {
    let file = dir.join("safety-events.json");
    if !file.exists() {
        return Ok(vec![]);
    }
    anyhow::ensure!(std::fs::metadata(&file)?.len() <= 32768, "故障记录文件过大");
    let mut incidents: Vec<Incident> = serde_json::from_slice(&std::fs::read(file)?)?;
    if incidents.len() > 16 {
        incidents.drain(..incidents.len() - 16);
    }
    Ok(incidents)
}
pub fn record(dir: &Path, error: &anyhow::Error) -> anyhow::Result<()> {
    let fault = error
        .downcast_ref::<Fault>()
        .cloned()
        .unwrap_or_else(|| Fault::new(Cause::SessionStopped));
    let mut incidents = history(dir)?;
    incidents.push(Incident {
        time_unix_ms: SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64,
        fault,
    });
    if incidents.len() > 16 {
        incidents.remove(0);
    }
    crossdesk_transport::atomic_save(
        &dir.join("safety-events.json"),
        &serde_json::to_vec_pretty(&incidents)?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_cause_survives_cleanup() {
        let slot = FaultSlot::default();
        slot.record(Fault::elapsed(Cause::CoordinatorStalled, 612));
        slot.record(Fault::new(Cause::CaptureStopped));
        assert_eq!(
            slot.get(),
            Some(Fault::elapsed(Cause::CoordinatorStalled, 612))
        );
    }

    #[test]
    fn history_is_bounded_persistent_and_does_not_copy_raw_errors() {
        let dir = std::env::temp_dir().join(format!("crossdesk-faults-{}", rand::random::<u64>()));
        std::fs::create_dir_all(&dir).unwrap();
        for _ in 0..20 {
            record(
                &dir,
                &anyhow::Error::new(Fault::new(Cause::UnsupportedRemoteInput)),
            )
            .unwrap();
        }
        record(
            &dir,
            &anyhow::anyhow!("private peer name 192.168.1.234 secret input"),
        )
        .unwrap();
        let saved = history(&dir).unwrap();
        assert_eq!(saved.len(), 16);
        assert_eq!(saved[14].fault.cause, Cause::UnsupportedRemoteInput);
        assert_eq!(saved[15].fault.cause, Cause::SessionStopped);
        let json = std::fs::read_to_string(dir.join("safety-events.json")).unwrap();
        assert!(!json.contains("private"));
        assert!(!json.contains("192.168"));
        assert!(!json.contains("secret"));
        std::fs::remove_file(dir.join("safety-events.json")).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}
