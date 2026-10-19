mod capture;
mod desktop;
mod dispatch;
mod display;
mod keys;
use anyhow::{bail, Context, Result};
pub use capture::*;
use crossdesk_core::*;
pub use desktop::input_desktop_available;
pub use display::topology;
use std::{
    mem::{size_of, zeroed},
    path::Path,
    ptr::{null, null_mut},
};
use windows_sys::Win32::{
    Foundation::*,
    Security::Cryptography::*,
    Storage::FileSystem::*,
    UI::{HiDpi::*, Input::KeyboardAndMouse::*},
};

pub fn dpi_awareness() -> Result<()> {
    unsafe {
        if SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) == 0 {
            // Tauri may already have set it; verify the actual thread context.
            if AreDpiAwarenessContextsEqual(
                GetThreadDpiAwarenessContext(),
                DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
            ) == 0
            {
                bail!("Per-Monitor V2 DPI awareness unavailable");
            }
        }
    }
    Ok(())
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn string(s: &[u16]) -> String {
    String::from_utf16_lossy(&s[..s.iter().position(|c| *c == 0).unwrap_or(s.len())])
}
pub fn cursor() -> Result<Point> {
    unsafe {
        let mut p: POINT = zeroed();
        if windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut p) == 0 {
            bail!("GetCursorPos failed");
        }
        Ok(Point {
            x: p.x as f64,
            y: p.y as f64,
        })
    }
}
/// Queried outside hooks only, before acquiring target control; never used to infer the current hook event.
pub fn physical_idle() -> bool {
    input_desktop_available() && unsafe { (1..=254).all(|vk| GetAsyncKeyState(vk) & i16::MIN == 0) }
}
pub fn inject(event: &InputEvent) -> Result<()> {
    unsafe {
        let mut input: INPUT = zeroed();
        match event {
            InputEvent::Key { key, down, .. } => {
                let (scan, extended) =
                    keys::hid_to_scan(*key).context("unsupported physical key")?;
                input.r#type = INPUT_KEYBOARD;
                input.Anonymous.ki = KEYBDINPUT {
                    wVk: 0,
                    wScan: scan,
                    dwFlags: KEYEVENTF_SCANCODE
                        | if extended { KEYEVENTF_EXTENDEDKEY } else { 0 }
                        | if *down { 0 } else { KEYEVENTF_KEYUP },
                    time: 0,
                    dwExtraInfo: crate::INJECTION_TAG,
                };
            }
            InputEvent::Move { position, .. } => {
                use windows_sys::Win32::UI::WindowsAndMessaging::*;
                let x = GetSystemMetrics(SM_XVIRTUALSCREEN);
                let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
                let w = GetSystemMetrics(SM_CXVIRTUALSCREEN);
                let h = GetSystemMetrics(SM_CYVIRTUALSCREEN);
                if w < 2 || h < 2 || !position.x.is_finite() || !position.y.is_finite() {
                    bail!("invalid virtual desktop");
                }
                let dx = ((position.x - x as f64) * 65535. / (w - 1) as f64)
                    .round()
                    .clamp(0., 65535.) as i32;
                let dy = ((position.y - y as f64) * 65535. / (h - 1) as f64)
                    .round()
                    .clamp(0., 65535.) as i32;
                input.r#type = INPUT_MOUSE;
                input.Anonymous.mi = MOUSEINPUT {
                    dx,
                    dy,
                    mouseData: 0,
                    dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                    time: 0,
                    dwExtraInfo: crate::INJECTION_TAG,
                };
            }
            InputEvent::Button { button, down } => {
                let flags = match (*button, *down) {
                    (1, true) => MOUSEEVENTF_LEFTDOWN,
                    (1, false) => MOUSEEVENTF_LEFTUP,
                    (2, true) => MOUSEEVENTF_RIGHTDOWN,
                    (2, false) => MOUSEEVENTF_RIGHTUP,
                    (3, true) => MOUSEEVENTF_MIDDLEDOWN,
                    (3, false) => MOUSEEVENTF_MIDDLEUP,
                    (4 | 5, true) => MOUSEEVENTF_XDOWN,
                    (4 | 5, false) => MOUSEEVENTF_XUP,
                    _ => bail!("unsupported button"),
                };
                input.r#type = INPUT_MOUSE;
                input.Anonymous.mi = MOUSEINPUT {
                    dx: 0,
                    dy: 0,
                    mouseData: if *button >= 4 {
                        (*button - 3) as u32
                    } else {
                        0
                    },
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: crate::INJECTION_TAG,
                };
            }
            InputEvent::Wheel { horizontal, delta } => {
                input.r#type = INPUT_MOUSE;
                input.Anonymous.mi = MOUSEINPUT {
                    dx: 0,
                    dy: 0,
                    mouseData: *delta as u32,
                    dwFlags: if *horizontal {
                        MOUSEEVENTF_HWHEEL
                    } else {
                        MOUSEEVENTF_WHEEL
                    },
                    time: 0,
                    dwExtraInfo: crate::INJECTION_TAG,
                };
            }
        }
        if SendInput(1, &input, size_of::<INPUT>() as i32) != 1 {
            bail!("SendInput failed (UIPI, desktop or permission boundary); sharing stopped");
        }
    }
    Ok(())
}
pub fn protect(bytes: &[u8]) -> Result<Vec<u8>> {
    crypt(bytes, true)
}
pub fn unprotect(bytes: &[u8]) -> Result<Vec<u8>> {
    crypt(bytes, false)
}
fn crypt(bytes: &[u8], protect: bool) -> Result<Vec<u8>> {
    unsafe {
        let input = CRYPT_INTEGER_BLOB {
            cbData: bytes.len() as u32,
            pbData: bytes.as_ptr() as *mut u8,
        };
        let mut out: CRYPT_INTEGER_BLOB = zeroed();
        let ok = if protect {
            CryptProtectData(
                &input,
                null(),
                null(),
                null_mut(),
                null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out,
            )
        } else {
            CryptUnprotectData(
                &input,
                null_mut(),
                null(),
                null_mut(),
                null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out,
            )
        };
        if ok == 0 {
            bail!("Windows user DPAPI failed");
        }
        let result = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        std::ptr::write_bytes(out.pbData, 0, out.cbData as usize);
        LocalFree(out.pbData as _);
        Ok(result)
    }
}
pub fn atomic_replace(from: &Path, to: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    let a: Vec<_> = from.as_os_str().encode_wide().chain(Some(0)).collect();
    let b: Vec<_> = to.as_os_str().encode_wide().chain(Some(0)).collect();
    if unsafe {
        MoveFileExW(
            a.as_ptr(),
            b.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    } == 0
    {
        bail!(
            "atomic configuration save failed: {}",
            std::io::Error::last_os_error()
        );
    }
    Ok(())
}
