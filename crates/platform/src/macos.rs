//! macOS CI builds pass on arm64 and x86_64; physical native input acceptance is pending.
//! Quartz coordinates are global desktop points; backing pixels remain a separate field.
use crate::NativeEvent;
use anyhow::{bail, Context, Result};
use crossdesk_core::*;
use std::{
    ffi::c_void,
    path::Path,
    ptr::{null, null_mut},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
type CFRef = *const c_void;
type Event = *mut c_void;
#[repr(C)]
#[derive(Clone, Copy)]
struct CGPoint {
    x: f64,
    y: f64,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct CGSize {
    width: f64,
    height: f64,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct CGRect {
    origin: CGPoint,
    size: CGSize,
}
type TapCallback = unsafe extern "C" fn(*mut c_void, u32, Event, *mut c_void) -> Event;
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn CGPreflightListenEventAccess() -> bool;
    fn CGPreflightPostEventAccess() -> bool;
    fn CGGetActiveDisplayList(max: u32, displays: *mut u32, count: *mut u32) -> i32;
    fn CGDisplayBounds(id: u32) -> CGRect;
    fn CGDisplayPixelsWide(id: u32) -> usize;
    fn CGDisplayPixelsHigh(id: u32) -> usize;
    fn CGDisplayRotation(id: u32) -> f64;
    fn CGDisplayIsMain(id: u32) -> u32;
    fn CGDisplayMirrorsDisplay(id: u32) -> u32;
    fn CGDisplayCreateUUIDFromDisplayID(id: u32) -> CFRef;
    fn CGEventCreate(source: CFRef) -> Event;
    fn CGEventGetLocation(event: Event) -> CGPoint;
    fn CGEventCreateKeyboardEvent(source: CFRef, key: u16, down: bool) -> Event;
    fn CGEventCreateMouseEvent(source: CFRef, kind: u32, point: CGPoint, button: u32) -> Event;
    fn CGEventCreateScrollWheelEvent(source: CFRef, units: u32, count: u32, ...) -> Event;
    fn CGEventSetIntegerValueField(event: Event, field: u32, value: i64);
    fn CGEventGetIntegerValueField(event: Event, field: u32) -> i64;
    fn CGEventGetFlags(event: Event) -> u64;
    fn CGEventSourceKeyState(state: u32, key: u16) -> bool;
    fn CGEventSourceButtonState(state: u32, button: u32) -> bool;
    fn CGEventPost(tap: u32, event: Event);
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        mask: u64,
        callback: TapCallback,
        user: *mut c_void,
    ) -> CFRef;
    fn CGEventTapEnable(tap: CFRef, enable: bool);
}
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(value: CFRef);
    fn CFUUIDCreateString(allocator: CFRef, uuid: CFRef) -> CFRef;
    fn CFStringGetCString(value: CFRef, buffer: *mut i8, size: isize, encoding: u32) -> bool;
    fn CFMachPortCreateRunLoopSource(allocator: CFRef, port: CFRef, order: isize) -> CFRef;
    fn CFRunLoopGetCurrent() -> CFRef;
    fn CFRunLoopAddSource(loop_: CFRef, source: CFRef, mode: CFRef);
    fn CFRunLoopRemoveSource(loop_: CFRef, source: CFRef, mode: CFRef);
    fn CFRunLoopRunInMode(mode: CFRef, seconds: f64, return_after_source: bool) -> i32;
    static kCFRunLoopDefaultMode: CFRef;
}
pub fn dpi_awareness() -> Result<()> {
    Ok(())
}
fn permitted() -> bool {
    unsafe {
        AXIsProcessTrusted() && CGPreflightListenEventAccess() && CGPreflightPostEventAccess()
    }
}
unsafe fn display_uuid(id: u32) -> Result<String> {
    let uuid = CGDisplayCreateUUIDFromDisplayID(id);
    if uuid.is_null() {
        bail!("display UUID unavailable");
    }
    let text = CFUUIDCreateString(null(), uuid);
    CFRelease(uuid);
    if text.is_null() {
        bail!("display UUID conversion failed");
    }
    let mut bytes = [0i8; 128];
    let ok = CFStringGetCString(text, bytes.as_mut_ptr(), 128, 0x08000100);
    CFRelease(text);
    if !ok {
        bail!("invalid display UUID");
    }
    Ok(std::ffi::CStr::from_ptr(bytes.as_ptr())
        .to_string_lossy()
        .into_owned())
}
pub fn topology(id: &str, version: u64) -> Result<DeviceTopology> {
    unsafe {
        let mut displays = [0u32; 32];
        let mut count = 0;
        if CGGetActiveDisplayList(32, displays.as_mut_ptr(), &mut count) != 0 {
            bail!("CGGetActiveDisplayList failed");
        }
        let mut monitors = vec![];
        for display in &displays[..count as usize] {
            let r = CGDisplayBounds(*display);
            let w = CGDisplayPixelsWide(*display);
            let h = CGDisplayPixelsHigh(*display);
            let mirrored = CGDisplayMirrorsDisplay(*display);
            monitors.push(Monitor {
                id: display_uuid(*display)?,
                device: id.into(),
                name: format!("Display {}", display),
                primary: CGDisplayIsMain(*display) != 0,
                rect: Rect {
                    x: r.origin.x,
                    y: r.origin.y,
                    width: r.size.width,
                    height: r.size.height,
                },
                resolution: [w as u32, h as u32],
                scale: w as f64 / r.size.width,
                rotation: CGDisplayRotation(*display) as u32,
                enabled: true,
                mirror_of: if mirrored == 0 {
                    None
                } else {
                    Some(display_uuid(mirrored)?)
                },
                unit: CoordinateUnit::DesktopPoints,
            });
        }
        Ok(DeviceTopology {
            device: Device {
                id: id.into(),
                name: std::env::var("HOSTNAME").unwrap_or_else(|_| "Mac".into()),
                os: OperatingSystem::MacOS,
                online: true,
                authorized: true,
                permission: if permitted() {
                    PermissionState::Granted
                } else {
                    PermissionState::Denied
                },
            },
            version,
            monitors,
        })
    }
}
pub fn cursor() -> Result<Point> {
    unsafe {
        let e = CGEventCreate(null());
        if e.is_null() {
            bail!("cannot query Quartz cursor");
        }
        let p = CGEventGetLocation(e);
        CFRelease(e);
        Ok(Point { x: p.x, y: p.y })
    }
}
pub fn physical_idle() -> bool {
    unsafe {
        KEYS.iter().all(|(key, _)| !CGEventSourceKeyState(1, *key))
            && (0..5).all(|b| !CGEventSourceButtonState(1, b))
    }
}
// Apple virtual key code -> USB HID keyboard usage. Unmapped Fn/media/JIS keys fail closed.
const KEYS: &[(u16, u16)] = &[
    (0, 4),
    (1, 22),
    (2, 7),
    (3, 9),
    (4, 11),
    (5, 10),
    (6, 29),
    (7, 27),
    (8, 6),
    (9, 25),
    (11, 5),
    (12, 20),
    (13, 26),
    (14, 8),
    (15, 21),
    (16, 28),
    (17, 23),
    (18, 30),
    (19, 31),
    (20, 32),
    (21, 33),
    (22, 35),
    (23, 34),
    (24, 46),
    (25, 38),
    (26, 36),
    (27, 45),
    (28, 37),
    (29, 39),
    (30, 48),
    (31, 18),
    (32, 24),
    (33, 47),
    (34, 12),
    (35, 19),
    (36, 40),
    (37, 15),
    (38, 13),
    (39, 52),
    (40, 14),
    (41, 51),
    (42, 49),
    (43, 54),
    (44, 56),
    (45, 17),
    (46, 16),
    (47, 55),
    (48, 43),
    (49, 44),
    (50, 53),
    (51, 42),
    (53, 41),
    (54, 231),
    (55, 227),
    (56, 225),
    (57, 57),
    (58, 226),
    (59, 224),
    (60, 229),
    (61, 230),
    (62, 228),
    (65, 99),
    (67, 85),
    (69, 87),
    (71, 83),
    (75, 84),
    (76, 88),
    (78, 86),
    (82, 98),
    (83, 89),
    (84, 90),
    (85, 91),
    (86, 92),
    (87, 93),
    (88, 94),
    (89, 95),
    (91, 96),
    (92, 97),
    (96, 62),
    (97, 63),
    (98, 64),
    (99, 60),
    (100, 65),
    (101, 66),
    (103, 68),
    (109, 67),
    (111, 69),
    (115, 74),
    (116, 75),
    (117, 76),
    (118, 61),
    (119, 77),
    (120, 59),
    (121, 78),
    (122, 58),
    (123, 80),
    (124, 79),
    (125, 81),
    (126, 82),
];
pub fn inject(event: &InputEvent) -> Result<()> {
    if !unsafe { CGPreflightPostEventAccess() } {
        bail!("macOS Accessibility permission missing for crossdesk-daemon");
    }
    unsafe {
        let native = match event {
            InputEvent::Key { key, down, .. } => {
                let code = KEYS
                    .iter()
                    .find(|(_, h)| *h == key.0)
                    .context("unmapped Mac physical key")?
                    .0;
                CGEventCreateKeyboardEvent(null(), code, *down)
            }
            InputEvent::Move { position, .. } => CGEventCreateMouseEvent(
                null(),
                5,
                CGPoint {
                    x: position.x,
                    y: position.y,
                },
                0,
            ),
            InputEvent::Button { button, down } => {
                let p = cursor()?;
                let (kind, button) = match (*button, *down) {
                    (1, true) => (1, 0),
                    (1, false) => (2, 0),
                    (2, true) => (3, 1),
                    (2, false) => (4, 1),
                    (3..=5, true) => (25, (*button - 1) as u32),
                    (3..=5, false) => (26, (*button - 1) as u32),
                    _ => bail!("unknown mouse button"),
                };
                CGEventCreateMouseEvent(null(), kind, CGPoint { x: p.x, y: p.y }, button)
            }
            InputEvent::Wheel { horizontal, delta } => {
                if *horizontal {
                    CGEventCreateScrollWheelEvent(null(), 1, 2, 0i32, *delta / 120)
                } else {
                    CGEventCreateScrollWheelEvent(null(), 1, 1, *delta / 120)
                }
            }
        };
        if native.is_null() {
            bail!("Quartz refused event creation");
        }
        CGEventSetIntegerValueField(native, 42, crate::INJECTION_TAG as i64);
        CGEventPost(0, native);
        CFRelease(native);
    }
    // Quartz posting has no success return; real delivery must be verified on the Mac.
    Ok(())
}
type Handler = Box<dyn FnMut(NativeEvent) -> bool + Send>;
struct TapContext {
    handler: Handler,
    disabled: bool,
}
unsafe extern "C" fn event_callback(
    _: *mut c_void,
    kind: u32,
    event: Event,
    user: *mut c_void,
) -> Event {
    let context = &mut *(user as *mut TapContext);
    if kind == 0xfffffffe || kind == 0xffffffff {
        context.disabled = true;
        (context.handler)(NativeEvent::TopologyChanged);
        return event;
    }
    if CGEventGetIntegerValueField(event, 42) == crate::INJECTION_TAG as i64 {
        return event;
    }
    let input = match kind {
        10 | 11 | 12 => {
            let code = CGEventGetIntegerValueField(event, 9) as u16;
            if let Some((_, hid)) = KEYS.iter().find(|(k, _)| *k == code) {
                let down = if kind == 12 {
                    let flags = CGEventGetFlags(event);
                    let mask = match code {
                        59 => 1,
                        62 => 0x2000,
                        56 => 2,
                        60 => 4,
                        58 => 0x20,
                        61 => 0x40,
                        55 => 8,
                        54 => 0x10,
                        57 => 1 << 16,
                        _ => 0,
                    };
                    flags & mask != 0
                } else {
                    kind == 10
                };
                NativeEvent::Input(InputEvent::Key {
                    key: PhysicalKey(*hid),
                    down,
                    repeat: CGEventGetIntegerValueField(event, 8) != 0,
                })
            } else {
                NativeEvent::UnsupportedKey
            }
        }
        5 | 6 | 7 | 27 => {
            let p = CGEventGetLocation(event);
            NativeEvent::Motion {
                delta: Point {
                    x: CGEventGetIntegerValueField(event, 4) as f64,
                    y: CGEventGetIntegerValueField(event, 5) as f64,
                },
                local: Point { x: p.x, y: p.y },
            }
        }
        1 | 2 | 3 | 4 | 25 | 26 => {
            let button = match kind {
                1 | 2 => 1,
                3 | 4 => 2,
                _ => CGEventGetIntegerValueField(event, 3) as u8 + 1,
            };
            NativeEvent::Input(InputEvent::Button {
                button,
                down: matches!(kind, 1 | 3 | 25),
            })
        }
        22 => {
            let horizontal = CGEventGetIntegerValueField(event, 11) != 0;
            NativeEvent::Input(InputEvent::Wheel {
                horizontal,
                delta: CGEventGetIntegerValueField(event, if horizontal { 11 } else { 10 }) as i32
                    * 120,
            })
        }
        _ => NativeEvent::LocalActivity,
    };
    if (context.handler)(input) {
        null_mut()
    } else {
        event
    }
}
pub fn capture(handler: Handler, stop: Arc<AtomicBool>) -> Result<()> {
    if !permitted() {
        bail!("grant Input Monitoring and Accessibility to the actual crossdesk-daemon binary, then restart");
    }
    unsafe {
        let mut context = Box::new(TapContext {
            handler,
            disabled: false,
        });
        let mask = [1, 2, 3, 4, 5, 6, 7, 10, 11, 12, 22, 25, 26, 27]
            .iter()
            .fold(0u64, |m, i| m | (1 << i));
        // options=0 is an active, suppressible tap, not listen-only.
        let tap = CGEventTapCreate(0, 0, 0, mask, event_callback, &mut *context as *mut _ as _);
        if tap.is_null() {
            bail!("active CGEventTap creation denied");
        }
        let source = CFMachPortCreateRunLoopSource(null(), tap, 0);
        if source.is_null() {
            CFRelease(tap);
            bail!("event tap run-loop source failed");
        }
        CFRunLoopAddSource(CFRunLoopGetCurrent(), source, kCFRunLoopDefaultMode);
        CGEventTapEnable(tap, true);
        (context.handler)(NativeEvent::Started);
        while !stop.load(Ordering::Acquire) && !context.disabled {
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.008, true);
            (context.handler)(NativeEvent::Tick);
        }
        CGEventTapEnable(tap, false);
        (context.handler)(NativeEvent::Shutdown);
        CFRunLoopRemoveSource(CFRunLoopGetCurrent(), source, kCFRunLoopDefaultMode);
        CFRelease(source);
        CFRelease(tap);
        if context.disabled {
            bail!("event tap disabled by macOS; sharing stopped");
        }
    }
    Ok(())
}
pub fn protect(bytes: &[u8]) -> Result<Vec<u8>> {
    // The file contains only a random Keychain account reference, never the private key.
    let id = format!(
        "identity-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    );
    security_framework::passwords::set_generic_password(
        crossdesk_core::profile::KEYCHAIN_SERVICE,
        &id,
        bytes,
    )?;
    Ok(id.into_bytes())
}
pub fn unprotect(reference: &[u8]) -> Result<Vec<u8>> {
    Ok(security_framework::passwords::generic_password(
        security_framework::passwords::PasswordOptions::new_generic_password(
            crossdesk_core::profile::KEYCHAIN_SERVICE,
            std::str::from_utf8(reference)?,
        ),
    )?)
}
pub fn atomic_replace(from: &Path, to: &Path) -> Result<()> {
    std::fs::rename(from, to)?;
    Ok(())
}
