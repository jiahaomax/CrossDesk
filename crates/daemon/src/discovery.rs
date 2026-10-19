use crate::status::*;
use anyhow::Result;
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredPeer {
    pub service: String,
    pub id: String,
    pub name: String,
    pub addresses: Vec<String>,
}
pub fn start(id: &str, status: SharedStatus) -> Result<ServiceDaemon> {
    let (name, port) = {
        let s = status.lock().unwrap();
        (s.topology[0].device.name.clone(), s.config.port)
    };
    let daemon = ServiceDaemon::new()?;
    let service = ServiceInfo::new(
        crossdesk_core::profile::DISCOVERY_SERVICE,
        &format!("CrossDesk-{}", &id[..12]),
        &format!("crossdesk-{}.local.", &id[..12]),
        "",
        port,
        &[
            ("id", id),
            ("protocol", "1"),
            ("pairing", "1"),
            ("name", &name),
        ][..],
    )?
    .enable_addr_auto();
    daemon.register(service)?;
    let receiver = daemon.browse(crossdesk_core::profile::DISCOVERY_SERVICE)?;
    let local_id = id.to_string();
    update(&status, |s| s.discovered.clear());
    std::thread::spawn(move || {
        while let Ok(event) = receiver.recv() {
            match event {
                ServiceEvent::ServiceResolved(info) => {
                    let id = info.get_property_val_str("id").unwrap_or("");
                    if id == local_id
                        || id.len() != 64
                        || !id.bytes().all(|b| b.is_ascii_hexdigit())
                        || info.get_property_val_str("pairing") != Some("1")
                    {
                        continue;
                    }
                    let mut addresses: Vec<_> = info
                        .get_addresses_v4()
                        .iter()
                        .filter(|ip| !ip.is_loopback() && !ip.is_unspecified())
                        .map(|ip| format!("{ip}:{}", info.get_port()))
                        .collect();
                    addresses.sort();
                    addresses.truncate(8);
                    if addresses.is_empty() {
                        continue;
                    }
                    let peer = DiscoveredPeer {
                        service: info.get_fullname().into(),
                        id: id.into(),
                        name: info
                            .get_property_val_str("name")
                            .unwrap_or("CrossDesk 电脑")
                            .chars()
                            .filter(|c| !c.is_control())
                            .take(40)
                            .collect(),
                        addresses,
                    };
                    update(&status, |s| {
                        s.discovered.retain(|v| v.service != peer.service);
                        if s.discovered.len() < 64 {
                            s.discovered.push(peer);
                        }
                    });
                }
                ServiceEvent::ServiceRemoved(_, name) => {
                    update(&status, |s| s.discovered.retain(|v| v.service != name))
                }
                _ => {}
            }
        }
    });
    Ok(daemon)
}
