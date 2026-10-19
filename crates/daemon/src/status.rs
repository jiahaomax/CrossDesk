use crossdesk_core::*;
use crossdesk_transport::TrustedPeer;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandOutcome {
    pub id: String,
    pub error: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Status {
    pub local_addresses: Vec<LocalAddress>,
    pub local_addresses_error: Option<String>,
    pub diagnostics_path: Option<String>,
    pub peer_sharing: std::collections::BTreeMap<String, bool>,
    // Passive reconnect failures are device diagnostics, not failed user commands.
    #[serde(default)]
    pub connection_issues: std::collections::BTreeMap<String, String>,
    pub effective_links: Vec<EdgeLink>,
    pub topology_error: Option<String>,
    pub observe_only: bool,
    pub pairing: Option<crate::pairing::PairingStatus>,
    pub discovery_error: Option<String>,
    pub last_command: Option<CommandOutcome>,
    pub enabled: bool,
    pub state: String,
    pub source: Option<String>,
    pub active: Option<String>,
    pub topology: Vec<DeviceTopology>,
    pub identity: String,
    pub certificate: Vec<u8>,
    pub trusted: Vec<TrustedPeer>,
    pub rtt_ms: Option<u64>,
    pub last_error: Option<String>,
    pub received: u64,
    pub config: crate::config::Config,
    pub discovered: Vec<crate::discovery::DiscoveredPeer>,
}
pub type SharedStatus = Arc<Mutex<Status>>;
#[derive(Clone, Serialize, Deserialize)]
pub struct LocalAddress {
    pub name: String,
    pub address: String,
}
pub fn refresh_addresses(status: &SharedStatus) {
    let result = if_addrs::get_if_addrs();
    update(status, |s| match result {
        Ok(interfaces) => {
            let mut addresses: Vec<_> = interfaces
                .into_iter()
                .filter(|i| i.is_oper_up() && !i.is_loopback())
                .filter_map(|i| match i.ip() {
                    std::net::IpAddr::V4(ip) if !ip.is_unspecified() => Some(LocalAddress {
                        name: i.name,
                        address: if s.config.port == 24872 {
                            ip.to_string()
                        } else {
                            format!("{ip}:{}", s.config.port)
                        },
                    }),
                    _ => None,
                })
                .collect();
            addresses.sort_by(|a, b| a.address.cmp(&b.address));
            addresses.dedup_by(|a, b| a.address == b.address);
            s.local_addresses = addresses;
            s.local_addresses_error = None;
        }
        Err(e) => {
            s.local_addresses.clear();
            s.local_addresses_error = Some(format!("无法枚举本机 IPv4 地址：{e}"));
        }
    });
}
pub fn update(status: &SharedStatus, f: impl FnOnce(&mut Status)) {
    if let Ok(mut s) = status.lock() {
        f(&mut s);
    }
}
