use anyhow::{bail, Result};
use crossdesk_core::*;
use std::{
    mem::{size_of, zeroed},
    ptr::{null, null_mut},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    System::LibraryLoader::*,
    UI::{Input::*, WindowsAndMessaging::*},
};

use super::dispatch::{Dispatcher, Handler, TickSchedule};
use crate::NativeEvent;
thread_local! {static DISPATCHER:Dispatcher=Dispatcher::default();}
fn dispatch(event: NativeEvent) -> bool {
    DISPATCHER.with(|d| d.dispatch(event))
}
/// Publish the current local/remote policy before an OS call can re-enter a hook.
/// Nested events wait for the handler, but Windows still needs an immediate answer.
pub fn set_capture_suppression(suppress: bool) {
    DISPATCHER.with(|d| d.set_suppression(suppress));
}
unsafe extern "system" fn keyboard(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
    if code >= 0 {
        let k = &*(l as *const KBDLLHOOKSTRUCT);
        if k.flags & LLKHF_INJECTED == 0 && k.dwExtraInfo != crate::INJECTION_TAG {
            let down = w as u32 == WM_KEYDOWN || w as u32 == WM_SYSKEYDOWN;
            let mapped = if k.vkCode == 0x13 {
                None
            } else {
                super::keys::scan_to_hid(k.scanCode as u16, k.flags & LLKHF_EXTENDED != 0)
            };
            let event = match mapped {
                Some(key) => NativeEvent::Input(InputEvent::Key {
                    key,
                    down,
                    repeat: false,
                }),
                None => NativeEvent::UnsupportedKey,
            };
            if dispatch(event) {
                return 1;
            }
        }
    }
    CallNextHookEx(null_mut(), code, w, l)
}
unsafe extern "system" fn mouse(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
    if code >= 0 {
        let m = &*(l as *const MSLLHOOKSTRUCT);
        if m.flags & LLMHF_INJECTED == 0 && m.dwExtraInfo != crate::INJECTION_TAG {
            let event = match w as u32 {
                WM_LBUTTONDOWN => Some(InputEvent::Button {
                    button: 1,
                    down: true,
                }),
                WM_LBUTTONUP => Some(InputEvent::Button {
                    button: 1,
                    down: false,
                }),
                WM_RBUTTONDOWN => Some(InputEvent::Button {
                    button: 2,
                    down: true,
                }),
                WM_RBUTTONUP => Some(InputEvent::Button {
                    button: 2,
                    down: false,
                }),
                WM_MBUTTONDOWN => Some(InputEvent::Button {
                    button: 3,
                    down: true,
                }),
                WM_MBUTTONUP => Some(InputEvent::Button {
                    button: 3,
                    down: false,
                }),
                WM_XBUTTONDOWN | WM_XBUTTONUP => Some(InputEvent::Button {
                    button: 3 + (m.mouseData >> 16) as u8,
                    down: w as u32 == WM_XBUTTONDOWN,
                }),
                WM_MOUSEWHEEL | WM_MOUSEHWHEEL => Some(InputEvent::Wheel {
                    horizontal: w as u32 == WM_MOUSEHWHEEL,
                    delta: (m.mouseData >> 16) as i16 as i32,
                }),
                _ => None,
            };
            // Mouse movement is sent only through Raw Input, never counted twice.
            if dispatch(
                event
                    .map(NativeEvent::Input)
                    .unwrap_or(NativeEvent::LocalActivity),
            ) {
                return 1;
            }
        }
    }
    CallNextHookEx(null_mut(), code, w, l)
}
unsafe extern "system" fn window(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match msg {
        WM_INPUT => {
            let mut raw: RAWINPUT = zeroed();
            let mut size = size_of::<RAWINPUT>() as u32;
            if GetRawInputData(
                l as HRAWINPUT,
                RID_INPUT,
                &mut raw as *mut _ as _,
                &mut size,
                size_of::<RAWINPUTHEADER>() as u32,
            ) != u32::MAX
                && raw.header.dwType == RIM_TYPEMOUSE
                && !raw.header.hDevice.is_null()
            {
                let m = raw.data.mouse;
                if m.ulExtraInformation == crate::INJECTION_TAG as u32 {
                    return DefWindowProcW(hwnd, msg, w, l);
                }
                if m.usFlags & 1 == 0 && (m.lLastX != 0 || m.lLastY != 0) {
                    if let Ok(local) = super::cursor() {
                        dispatch(NativeEvent::Motion {
                            delta: Point {
                                x: m.lLastX as f64,
                                y: m.lLastY as f64,
                            },
                            local,
                        });
                    }
                } else if m.usFlags & 1 != 0 {
                    dispatch(NativeEvent::UnsupportedKey);
                }
            }
        }
        WM_DISPLAYCHANGE | WM_POWERBROADCAST => {
            dispatch(NativeEvent::TopologyChanged);
        }
        _ => {}
    }
    DefWindowProcW(hwnd, msg, w, l)
}
/// Explicitly started only after the user enables sharing. All callbacks execute on this thread.
pub fn capture(handler: Handler, stop: Arc<AtomicBool>) -> Result<()> {
    unsafe {
        super::dpi_awareness()?;
        if !super::input_desktop_available() {
            bail!("当前不是可操作的普通桌面：请解锁、返回当前登录会话后手动恢复共享");
        }
        DISPATCHER.with(|d| d.install(handler));
        let name = super::wide("CrossDeskInputWindow");
        let instance = GetModuleHandleW(null());
        let class = WNDCLASSW {
            lpfnWndProc: Some(window),
            hInstance: instance,
            lpszClassName: name.as_ptr(),
            ..zeroed()
        };
        RegisterClassW(&class);
        // Hidden top-level window receives display/power broadcasts, unlike HWND_MESSAGE.
        let hwnd = CreateWindowExW(
            0,
            name.as_ptr(),
            name.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            null_mut(),
            null_mut(),
            instance,
            null(),
        );
        if hwnd.is_null() {
            DISPATCHER.with(|d| d.clear());
            bail!("input message window failed");
        }
        let raw = RAWINPUTDEVICE {
            usUsagePage: 1,
            usUsage: 2,
            dwFlags: RIDEV_INPUTSINK | RIDEV_DEVNOTIFY,
            hwndTarget: hwnd,
        };
        if RegisterRawInputDevices(&raw, 1, size_of::<RAWINPUTDEVICE>() as u32) == 0 {
            DestroyWindow(hwnd);
            DISPATCHER.with(|d| d.clear());
            bail!("Raw Input registration failed");
        }
        let kh = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard), instance, 0);
        let mh = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse), instance, 0);
        if kh.is_null() || mh.is_null() {
            if !kh.is_null() {
                UnhookWindowsHookEx(kh);
            }
            if !mh.is_null() {
                UnhookWindowsHookEx(mh);
            }
            DestroyWindow(hwnd);
            DISPATCHER.with(|d| d.clear());
            bail!("input hooks could not be installed");
        }
        dispatch(NativeEvent::Started);
        let mut msg: MSG = zeroed();
        let mut ticks = TickSchedule::new(Instant::now());
        let mut desktop_check = Instant::now();
        let mut pump_error = None;
        while !stop.load(Ordering::Acquire) {
            if DISPATCHER.with(|d| d.fault().is_some()) {
                break;
            }
            if desktop_check.elapsed() >= Duration::from_millis(50) {
                desktop_check = Instant::now();
                if !super::input_desktop_available() {
                    pump_error = Some("桌面已锁定、切换或会话已断开；输入捕获已停止");
                    break;
                }
            }
            if ticks.due(Instant::now()) {
                dispatch(NativeEvent::Tick);
            }
            // One message per iteration, so continuous mouse traffic cannot
            // starve control acknowledgements, cleanup, or stop checks.
            if PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) != 0 {
                if msg.message == WM_QUIT {
                    break;
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            } else if MsgWaitForMultipleObjectsEx(0, null(), 2, QS_ALLINPUT, MWMO_INPUTAVAILABLE)
                == WAIT_FAILED
            {
                pump_error = Some("input message wait failed");
                break;
            }
        }
        UnhookWindowsHookEx(kh);
        UnhookWindowsHookEx(mh);
        dispatch(NativeEvent::Shutdown);
        DestroyWindow(hwnd);
        let fault = DISPATCHER.with(|d| {
            let fault = d.fault();
            d.clear();
            fault
        });
        if let Some(error) = fault.or(pump_error) {
            bail!("{error}");
        }
    }
    Ok(())
}
