pub mod layout;
pub mod ownership;
pub mod profile;
pub mod queue;
pub mod receiver;
pub mod session;
pub mod tuning;
pub use layout::*;
use serde::{Deserialize, Serialize};
pub use session::*;
pub use tuning::*;

pub type DeviceId = String;
pub type MonitorId = String;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OperatingSystem {
    Windows,
    MacOS,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PermissionState {
    Granted,
    Denied,
    Unverified,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Device {
    pub id: DeviceId,
    pub name: String,
    pub os: OperatingSystem,
    pub online: bool,
    pub authorized: bool,
    pub permission: PermissionState,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CoordinateUnit {
    PhysicalPixels,
    DesktopPoints,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Monitor {
    pub id: MonitorId,
    pub device: DeviceId,
    pub name: String,
    pub primary: bool,
    pub rect: Rect,
    pub resolution: [u32; 2],
    pub scale: f64,
    pub rotation: u32,
    pub enabled: bool,
    pub mirror_of: Option<MonitorId>,
    pub unit: CoordinateUnit,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeviceTopology {
    pub device: Device,
    pub version: u64,
    pub monitors: Vec<Monitor>,
}

/// USB HID usage page 0x07, never Windows VK values.
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysicalKey(pub u16);
impl PhysicalKey {
    pub const LEFT_CTRL: Self = Self(0xe0);
    pub const RIGHT_CTRL: Self = Self(0xe4);
    pub const LEFT_ALT: Self = Self(0xe2);
    pub const RIGHT_ALT: Self = Self(0xe6);
    pub const ESC: Self = Self(0x29);
    pub fn is_ctrl(self) -> bool {
        self == Self::LEFT_CTRL || self == Self::RIGHT_CTRL
    }
    pub fn is_alt(self) -> bool {
        self == Self::LEFT_ALT || self == Self::RIGHT_ALT
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum InputEvent {
    Key {
        key: PhysicalKey,
        down: bool,
        repeat: bool,
    },
    Button {
        button: u8,
        down: bool,
    },
    Move {
        position: Point,
        topology_version: u64,
    },
    Wheel {
        horizontal: bool,
        delta: i32,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InputPacket {
    pub source: DeviceId,
    pub session: u64,
    pub epoch: u64,
    pub sequence: u64,
    pub event: InputEvent,
}
