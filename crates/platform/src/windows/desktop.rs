//! Read-only desktop/session checks. Never switches desktops or elevates access.
use std::{mem::size_of, ptr::null_mut};
use windows_sys::Win32::{
    Foundation::HANDLE,
    System::{RemoteDesktop::*, StationsAndDesktops::*, Threading::*},
};

unsafe fn name(handle: HANDLE) -> Option<String> {
    let mut buffer = [0u16; 128];
    let mut needed = 0;
    if handle.is_null()
        || GetUserObjectInformationW(
            handle,
            UOI_NAME,
            buffer.as_mut_ptr().cast(),
            size_of::<[u16; 128]>() as u32,
            &mut needed,
        ) == 0
    {
        return None;
    }
    let end = buffer.iter().position(|c| *c == 0)?;
    String::from_utf16(&buffer[..end]).ok()
}

fn allowed(
    active_session: bool,
    thread: Option<&str>,
    input: Option<&str>,
    receives_input: bool,
) -> bool {
    active_session
        && receives_input
        && thread.is_some_and(|n| n.eq_ignore_ascii_case("default"))
        && input.is_some_and(|n| n.eq_ignore_ascii_case("default"))
}

/// Also rejects disconnected RDP sessions: OpenInputDesktop alone can return
/// the desktop that will become active on reconnection, even while disconnected.
pub fn input_desktop_available() -> bool {
    unsafe {
        let mut data = null_mut();
        let mut bytes = 0;
        let queried = WTSQuerySessionInformationW(
            WTS_CURRENT_SERVER_HANDLE,
            WTS_CURRENT_SESSION,
            WTSConnectState,
            &mut data,
            &mut bytes,
        ) != 0;
        let active = if queried && !data.is_null() && bytes as usize >= size_of::<i32>() {
            std::ptr::read_unaligned(data.cast::<i32>()) == WTSActive
        } else if !queried {
            // Remote Desktop Services may be disabled. Only the actual console
            // session can use this fallback; the desktop must still be active.
            let mut session = u32::MAX;
            ProcessIdToSessionId(GetCurrentProcessId(), &mut session) != 0
                && session != 0
                && session != u32::MAX
                && session == WTSGetActiveConsoleSessionId()
        } else {
            false
        };
        if !data.is_null() {
            WTSFreeMemory(data.cast());
        }
        if !active {
            return false;
        }
        let thread = GetThreadDesktop(GetCurrentThreadId());
        let input = OpenInputDesktop(0, 0, DESKTOP_READOBJECTS);
        if input.is_null() {
            return false;
        }
        let mut receives_input: i32 = 0;
        let mut needed = 0;
        let io = GetUserObjectInformationW(
            thread,
            UOI_IO,
            (&mut receives_input as *mut i32).cast(),
            size_of::<i32>() as u32,
            &mut needed,
        ) != 0
            && receives_input != 0;
        let result = allowed(active, name(thread).as_deref(), name(input).as_deref(), io);
        CloseDesktop(input); // GetThreadDesktop returns a borrowed handle.
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lock_disconnect_unknown_and_alternate_desktops_are_denied() {
        assert!(allowed(true, Some("Default"), Some("default"), true));
        assert!(!allowed(false, Some("Default"), Some("Default"), true));
        assert!(!allowed(true, Some("Default"), Some("Winlogon"), true));
        assert!(!allowed(true, Some("Default"), None, true));
        assert!(!allowed(true, None, Some("Default"), true));
        assert!(!allowed(true, Some("Default"), Some("Default"), false));
        assert!(!allowed(
            true,
            Some("Screen-saver"),
            Some("Screen-saver"),
            true
        ));
    }
}
