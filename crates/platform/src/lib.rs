pub const INJECTION_TAG: usize = 0x4352444b;
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
