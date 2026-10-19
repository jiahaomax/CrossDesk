//! Build-time product separation. Environment variables cannot select production
//! storage from a test binary (or turn a production binary into a cleanup tool).
pub const IS_TEST: bool = cfg!(feature = "test-profile");
pub const DATA_DIRECTORY: &str = if IS_TEST {
    "CrossDesk Test"
} else {
    "CrossDesk"
};
pub const KEYCHAIN_SERVICE: &str = if IS_TEST {
    "dev.crossdesk.test.identity"
} else {
    "dev.crossdesk.identity"
};
pub const DEFAULT_PORT: u16 = if IS_TEST { 24972 } else { 24872 };
pub const DISCOVERY_SERVICE: &str = if IS_TEST {
    "_crossdesk-test._tcp.local."
} else {
    "_crossdesk._tcp.local."
};
pub const PAIRING_CONTEXT: &[u8] = if IS_TEST {
    b"CrossDesk/test/pairing/v1"
} else {
    b"CrossDesk/pairing/v1"
};
pub const BUNDLE_ID: &str = if IS_TEST {
    "dev.crossdesk.desktop.test"
} else {
    "dev.crossdesk.desktop"
};
