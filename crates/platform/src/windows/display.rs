//! Read the current GDI desktop mode for coordinates, CCD paths for stable identity.
//! Never rotate or DPI-divide an already oriented Windows desktop rectangle.
use super::{dpi_awareness, input_desktop_available, string};
use anyhow::{bail, ensure, Context, Result};
use crossdesk_core::*;
use std::{
    collections::BTreeMap,
    mem::{size_of, zeroed},
    ptr::null_mut,
};
use windows_sys::Win32::UI::WindowsAndMessaging::MONITORINFOF_PRIMARY;
use windows_sys::Win32::{Devices::Display::*, Foundation::RECT, Graphics::Gdi::*, UI::HiDpi::*};

pub fn topology(id: &str, version: u64) -> Result<DeviceTopology> {
    dpi_awareness()?;
    // Display configuration can change between buffer sizing, CCD and GDI calls.
    // Retry a fresh snapshot, never combine new geometry with an old monitor name.
    let mut last = None;
    for _ in 0..3 {
        match unsafe { snapshot(id, version) } {
            Ok(topology) => return Ok(topology),
            Err(error) => last = Some(error),
        }
    }
    Err(last.unwrap()).context("无法读取一致的 Windows 显示器配置；请检查系统显示设置后重试")
}

fn desktop_rect(x: i32, y: i32, width: u32, height: u32, bounds: RECT) -> Result<Rect> {
    let rect = Rect {
        x: x as f64,
        y: y as f64,
        width: width as f64,
        height: height as f64,
    };
    ensure!(rect.valid(), "invalid desktop mode dimensions");
    ensure!(
        bounds.left == x
            && bounds.top == y
            && i64::from(bounds.right) - i64::from(x) == i64::from(width)
            && i64::from(bounds.bottom) - i64::from(y) == i64::from(height),
        "显示模式与光标桌面范围不一致，屏幕配置可能正在变化"
    );
    Ok(rect)
}

fn monitor_identity(
    device_path: String,
    adapter_high: i32,
    adapter_low: u32,
    target: u32,
) -> String {
    if !device_path.is_empty() {
        return device_path;
    }
    // Virtual/forced display paths can have no PnP monitor name. The active CCD
    // adapter+target still identifies a real system path. It may change after an
    // adapter restart; normal topology invalidation then discards old control.
    format!("ccd:{adapter_high:08x}:{adapter_low:08x}:{target}")
}

unsafe fn snapshot(id: &str, version: u64) -> Result<DeviceTopology> {
    let mut paths_len = 0;
    let mut modes_len = 0;
    let code = GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut paths_len, &mut modes_len);
    ensure!(code == 0, "GetDisplayConfigBufferSizes failed ({code})");
    let mut paths: Vec<DISPLAYCONFIG_PATH_INFO> = vec![zeroed(); paths_len as usize];
    let mut modes: Vec<DISPLAYCONFIG_MODE_INFO> = vec![zeroed(); modes_len as usize];
    let code = QueryDisplayConfig(
        QDC_ONLY_ACTIVE_PATHS,
        &mut paths_len,
        paths.as_mut_ptr(),
        &mut modes_len,
        modes.as_mut_ptr(),
        null_mut(),
    );
    ensure!(code == 0, "QueryDisplayConfig failed ({code})");
    paths.truncate(paths_len as usize);
    let mut monitors = vec![];
    let mut source_ids = BTreeMap::new();
    for path in paths {
        let mut target: DISPLAYCONFIG_TARGET_DEVICE_NAME = zeroed();
        target.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME;
        target.header.size = size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32;
        target.header.adapterId = path.targetInfo.adapterId;
        target.header.id = path.targetInfo.id;
        ensure!(
            DisplayConfigGetDeviceInfo(&mut target.header) == 0,
            "cannot obtain stable monitor identity"
        );
        let monitor_id = monitor_identity(
            string(&target.monitorDevicePath),
            path.targetInfo.adapterId.HighPart,
            path.targetInfo.adapterId.LowPart,
            path.targetInfo.id,
        );

        let mut source: DISPLAYCONFIG_SOURCE_DEVICE_NAME = zeroed();
        source.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME;
        source.header.size = size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32;
        source.header.adapterId = path.sourceInfo.adapterId;
        source.header.id = path.sourceInfo.id;
        ensure!(
            DisplayConfigGetDeviceInfo(&mut source.header) == 0,
            "cannot map monitor to desktop source"
        );
        let mut mode: DEVMODEW = zeroed();
        mode.dmSize = size_of::<DEVMODEW>() as u16;
        // CURRENT, not registry or preferred panel resolution. Always physical pixels.
        ensure!(
            EnumDisplaySettingsExW(
                source.viewGdiDeviceName.as_ptr(),
                ENUM_CURRENT_SETTINGS,
                &mut mode,
                0
            ) != 0,
            "cannot read current display mode"
        );
        ensure!(
            mode.dmFields & (DM_POSITION | DM_PELSWIDTH | DM_PELSHEIGHT)
                == (DM_POSITION | DM_PELSWIDTH | DM_PELSHEIGHT),
            "incomplete current display mode"
        );
        let pos = mode.Anonymous1.Anonymous2.dmPosition;
        let bounds = RECT {
            left: pos.x,
            top: pos.y,
            right: pos
                .x
                .checked_add(mode.dmPelsWidth.try_into()?)
                .context("invalid display width")?,
            bottom: pos
                .y
                .checked_add(mode.dmPelsHeight.try_into()?)
                .context("invalid display height")?,
        };
        let handle = MonitorFromRect(&bounds, MONITOR_DEFAULTTONULL);
        ensure!(!handle.is_null(), "active desktop monitor missing");
        let mut info: MONITORINFOEXW = zeroed();
        info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
        ensure!(
            GetMonitorInfoW(handle, &mut info.monitorInfo) != 0,
            "GetMonitorInfo failed"
        );
        ensure!(
            string(&info.szDevice).eq_ignore_ascii_case(&string(&source.viewGdiDeviceName)),
            "display source changed during query"
        );
        let rect = desktop_rect(
            pos.x,
            pos.y,
            mode.dmPelsWidth,
            mode.dmPelsHeight,
            info.monitorInfo.rcMonitor,
        )?;
        let (mut dx, mut dy) = (0, 0);
        ensure!(
            GetDpiForMonitor(handle, MDT_EFFECTIVE_DPI, &mut dx, &mut dy) >= 0 && dx > 0 && dy > 0,
            "cannot read monitor scaling"
        );
        let key = (
            path.sourceInfo.adapterId.HighPart,
            path.sourceInfo.adapterId.LowPart,
            path.sourceInfo.id,
        );
        let mirror_of = source_ids.get(&key).cloned();
        source_ids.entry(key).or_insert(monitor_id.clone());
        let name = string(&target.monitorFriendlyDeviceName);
        monitors.push(Monitor {
            id: monitor_id,
            device: id.into(),
            name: if name.is_empty() {
                string(&source.viewGdiDeviceName)
            } else {
                name
            },
            primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
            rect,
            resolution: [mode.dmPelsWidth, mode.dmPelsHeight],
            scale: dx as f64 / 96.,
            rotation: match path.targetInfo.rotation {
                DISPLAYCONFIG_ROTATION_IDENTITY => 0,
                DISPLAYCONFIG_ROTATION_ROTATE90 => 90,
                DISPLAYCONFIG_ROTATION_ROTATE180 => 180,
                DISPLAYCONFIG_ROTATION_ROTATE270 => 270,
                _ => bail!("unknown monitor rotation"),
            },
            enabled: true,
            mirror_of,
            unit: CoordinateUnit::PhysicalPixels,
        });
    }
    ensure!(!monitors.is_empty(), "no active displays");
    // Enumeration order isn't a layout change. Keep UI numbering stable between polls.
    monitors.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(DeviceTopology {
        device: Device {
            id: id.into(),
            name: std::env::var("COMPUTERNAME").unwrap_or_else(|_| "Windows".into()),
            os: OperatingSystem::Windows,
            online: true,
            authorized: true,
            permission: if input_desktop_available() {
                PermissionState::Granted
            } else {
                PermissionState::Denied
            },
        },
        version,
        monitors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unnamed_active_targets_keep_distinct_system_identities() {
        assert_eq!(
            monitor_identity("pnp-monitor".into(), 0, 1, 2),
            "pnp-monitor"
        );
        assert_eq!(
            monitor_identity(String::new(), 0, 1, 2),
            "ccd:00000000:00000001:2"
        );
        assert_ne!(
            monitor_identity(String::new(), 0, 1, 2),
            monitor_identity(String::new(), 0, 1, 3)
        );
        assert_ne!(
            monitor_identity(String::new(), 0, 1, 2),
            monitor_identity(String::new(), 0, 2, 2)
        );
    }
    #[test]
    fn oriented_physical_mode_is_not_rotated_or_scaled_again() {
        for (x, y, w, h) in [
            (0, 0, 3840, 2160),
            (3840, -600, 1440, 2560),
            (-2160, -800, 2160, 3840),
        ] {
            let r = desktop_rect(
                x,
                y,
                w,
                h,
                RECT {
                    left: x,
                    top: y,
                    right: x + w as i32,
                    bottom: y + h as i32,
                },
            )
            .unwrap();
            assert_eq!(
                (r.x, r.y, r.width, r.height),
                (x as f64, y as f64, w as f64, h as f64)
            );
        }
    }
    #[test]
    fn mismatched_rotated_or_dpi_virtualized_snapshots_are_rejected() {
        assert!(desktop_rect(
            3840,
            0,
            1440,
            2560,
            RECT {
                left: 3840,
                top: 0,
                right: 6400,
                bottom: 1440
            }
        )
        .is_err());
        assert!(desktop_rect(
            0,
            0,
            3840,
            2160,
            RECT {
                left: 0,
                top: 0,
                right: 2560,
                bottom: 1440
            }
        )
        .is_err());
    }
}
