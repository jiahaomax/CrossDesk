use crate::{status::*, stop_running, Running};
use anyhow::Result;
use crossdesk_core::DeviceTopology;

fn changed(old: &DeviceTopology, next: &DeviceTopology) -> bool {
    let sorted = |topology: &DeviceTopology| {
        let mut monitors = topology.monitors.clone();
        monitors.sort_by(|a, b| a.id.cmp(&b.id));
        monitors
    };
    sorted(old) != sorted(next) || old.device.permission != next.device.permission
}

/// Automatic and manual refresh must use exactly the same invalidation path.
pub async fn refresh(
    status: &SharedStatus,
    running: &mut Option<Running>,
    id: &str,
    version: &mut u64,
) -> Result<()> {
    let read = crossdesk_platform::topology(id, *version);
    reconcile(status, running, id, version, read).await
}

async fn reconcile(
    status: &SharedStatus,
    running: &mut Option<Running>,
    id: &str,
    version: &mut u64,
    read: Result<DeviceTopology>,
) -> Result<()> {
    match read {
        Ok(mut next) => {
            let old = status
                .lock()
                .unwrap()
                .topology
                .iter()
                .find(|t| t.device.id == id)
                .cloned();
            let invalidated = old.as_ref().is_none_or(|old| changed(old, &next));
            if invalidated {
                // End old captures/leases before publishing a new coordinate space.
                stop_running(running).await;
                *version += 1;
            }
            next.version = *version;
            update(status, |s| {
                let recovered = s.topology_error.is_some();
                if s.topology_error.is_some() && s.last_error == s.topology_error {
                    s.last_error = None;
                }
                s.topology_error = None;
                s.topology.retain(|t| t.device.id != id);
                s.topology.insert(0, next);
                if invalidated {
                    s.active = None;
                    s.source = None;
                    s.effective_links.clear();
                    s.state = "显示器布局已更新 · 正在重新同步连接".into();
                } else if recovered {
                    s.state = "显示器已重新读取 · 等待同步连接".into();
                }
            });
            Ok(())
        }
        Err(error) => {
            stop_running(running).await;
            let message = format!("显示器读取失败，已停止控制；画布保留上次读取结果。{error:#}");
            update(status, |s| {
                s.topology_error = Some(message.clone());
                s.last_error = Some(message);
                s.active = None;
                s.source = None;
                s.enabled = false;
                s.effective_links.clear();
                s.state = "等待重新读取显示器 · 控制已停止".into();
            });
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossdesk_core::*;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    };
    fn status(local: DeviceTopology) -> SharedStatus {
        Arc::new(Mutex::new(Status {
            local_addresses: vec![],
            local_addresses_error: None,
            diagnostics_path: None,
            peer_sharing: Default::default(),
            connection_issues: Default::default(),
            effective_links: vec![],
            topology_error: None,
            observe_only: true,
            pairing: None,
            discovery_error: None,
            last_command: None,
            enabled: false,
            state: String::new(),
            source: None,
            active: None,
            identity: local.device.id.clone(),
            topology: vec![local],
            certificate: vec![],
            trusted: vec![],
            rtt_ms: None,
            last_error: None,
            received: 0,
            config: Default::default(),
            discovered: vec![],
        }))
    }
    #[tokio::test]
    async fn refresh_ends_old_session_before_publishing_new_coordinates_and_version() {
        let old = topology();
        let status = status(old.clone());
        let shared = status.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let task = tokio::spawn(async move {
            while !stopped.load(Ordering::Acquire) {
                tokio::task::yield_now().await;
            }
            assert_eq!(shared.lock().unwrap().topology[0], old);
        });
        let (configure, _updates) = tokio::sync::mpsc::channel(1);
        let mut running = Some(Running {
            stop,
            task,
            configure,
        });
        let mut version = 1;
        reconcile(&status, &mut running, "local", &mut version, Ok(topology()))
            .await
            .unwrap();
        assert!(running.is_some());
        assert_eq!(version, 1);
        let mut rotated = topology();
        rotated.monitors[0].rect.width = 1440.;
        rotated.monitors[0].rect.height = 2560.;
        rotated.monitors[0].resolution = [1440, 2560];
        rotated.monitors[0].rotation = 90;
        reconcile(&status, &mut running, "local", &mut version, Ok(rotated))
            .await
            .unwrap();
        assert!(running.is_none());
        assert_eq!(version, 2);
        let s = status.lock().unwrap();
        assert_eq!(s.topology[0].version, 2);
        assert_eq!(s.topology[0].monitors[0].resolution, [1440, 2560]);
        assert!(s.active.is_none() && s.effective_links.is_empty());
    }
    #[tokio::test]
    async fn failed_read_marks_cached_geometry_until_fresh_read_and_preserves_unrelated_errors() {
        let status = status(topology());
        let mut running = None;
        let mut version = 1;
        assert!(reconcile(
            &status,
            &mut running,
            "local",
            &mut version,
            Err(anyhow::anyhow!("test display unavailable"))
        )
        .await
        .is_err());
        assert!(status.lock().unwrap().topology_error.is_some());
        assert!(!status.lock().unwrap().enabled);
        status.lock().unwrap().last_error = Some("unrelated error".into());
        reconcile(&status, &mut running, "local", &mut version, Ok(topology()))
            .await
            .unwrap();
        let s = status.lock().unwrap();
        assert!(s.topology_error.is_none());
        assert_eq!(s.last_error.as_deref(), Some("unrelated error"));
    }
    fn topology() -> DeviceTopology {
        DeviceTopology {
            device: Device {
                id: "local".into(),
                name: "local".into(),
                os: OperatingSystem::Windows,
                online: true,
                authorized: true,
                permission: PermissionState::Granted,
            },
            version: 1,
            monitors: vec![Monitor {
                id: "display".into(),
                device: "local".into(),
                name: "screen".into(),
                primary: true,
                rect: Rect {
                    x: 0.,
                    y: 0.,
                    width: 2560.,
                    height: 1440.,
                },
                resolution: [2560, 1440],
                scale: 1.25,
                rotation: 0,
                enabled: true,
                mirror_of: None,
                unit: CoordinateUnit::PhysicalPixels,
            }],
        }
    }
    #[test]
    fn geometry_rotation_scale_and_permission_changes_invalidate_session() {
        let old = topology();
        for edit in 0..5 {
            let mut next = old.clone();
            match edit {
                0 => {
                    next.monitors[0].rect.width = 1440.;
                    next.monitors[0].rect.height = 2560.;
                    next.monitors[0].resolution = [1440, 2560];
                    next.monitors[0].rotation = 90;
                }
                1 => next.monitors[0].rect.x = -2560.,
                2 => next.monitors[0].scale = 1.75,
                3 => next.monitors[0].enabled = false,
                _ => next.device.permission = PermissionState::Denied,
            }
            assert!(changed(&old, &next));
        }
        assert!(!changed(&old, &old));
        let mut multi = old;
        let mut second = multi.monitors[0].clone();
        second.id = "second".into();
        multi.monitors.push(second);
        let mut reordered = multi.clone();
        reordered.monitors.reverse();
        assert!(!changed(&multi, &reordered));
    }
}
