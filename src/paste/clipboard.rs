#[cfg(unix)]
pub fn set(text: &str, prev: &mut Option<std::process::Child>) -> Result<(), String> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    use std::time::Duration;

    let is_wayland = std::env::var("WAYLAND_DISPLAY").is_ok();

    if is_wayland {
        // Kill the previous wl-copy so we can own the selection cleanly.
        if let Some(mut child) = prev.take() {
            let _ = child.kill();
            let _ = child.wait();
            std::thread::sleep(Duration::from_millis(10));
        }

        let mut child = Command::new("wl-copy")
            .args(["--foreground"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to spawn wl-copy: {e}"))?;

        child
            .stdin
            .take()
            .unwrap()
            .write_all(text.as_bytes())
            .map_err(|e| format!("Failed to write to wl-copy stdin: {e}"))?;

        std::thread::sleep(Duration::from_millis(20));
        *prev = Some(child);
    } else {
        let mut child = Command::new("xclip")
            .args(["-selection", "clipboard"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to spawn xclip: {e}"))?;

        child
            .stdin
            .take()
            .unwrap()
            .write_all(text.as_bytes())
            .map_err(|e| format!("Failed to write to xclip stdin: {e}"))?;

        let _ = child.wait();
    }

    Ok(())
}

#[cfg(windows)]
pub fn set(text: &str, _prev: &mut Option<std::process::Child>) -> Result<(), String> {
    use std::mem::size_of;
    use windows_sys::Win32::Foundation::GlobalFree;
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows_sys::Win32::System::Memory::{
        GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock,
    };
    use windows_sys::Win32::System::Ole::CF_UNICODETEXT;

    struct ClipboardGuard;

    impl Drop for ClipboardGuard {
        fn drop(&mut self) {
            unsafe {
                CloseClipboard();
            }
        }
    }

    fn last_error(action: &str) -> String {
        format!("{action} failed: {}", std::io::Error::last_os_error())
    }

    fn open_clipboard() -> Result<ClipboardGuard, String> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            unsafe {
                if OpenClipboard(std::ptr::null_mut()) != 0 {
                    return Ok(ClipboardGuard);
                }
            }

            if std::time::Instant::now() >= deadline {
                return Err(last_error("OpenClipboard"));
            }

            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes = wide
        .len()
        .checked_mul(size_of::<u16>())
        .ok_or_else(|| "Clipboard text is too large".to_string())?;

    unsafe {
        let mem = GlobalAlloc(GMEM_MOVEABLE, bytes);
        if mem.is_null() {
            return Err(last_error("GlobalAlloc"));
        }

        let ptr = GlobalLock(mem).cast::<u16>();
        if ptr.is_null() {
            GlobalFree(mem);
            return Err(last_error("GlobalLock"));
        }

        std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
        GlobalUnlock(mem);

        let _clipboard = match open_clipboard() {
            Ok(guard) => guard,
            Err(e) => {
                GlobalFree(mem);
                return Err(e);
            }
        };

        if EmptyClipboard() == 0 {
            GlobalFree(mem);
            return Err(last_error("EmptyClipboard"));
        }

        if SetClipboardData(CF_UNICODETEXT as u32, mem).is_null() {
            GlobalFree(mem);
            return Err(last_error("SetClipboardData"));
        }
    }

    Ok(())
}

#[cfg(all(test, windows))]
mod tests {
    use super::set;
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, OpenClipboard,
    };
    use windows_sys::Win32::System::Memory::{GlobalLock, GlobalUnlock};
    use windows_sys::Win32::System::Ole::CF_UNICODETEXT;

    struct ClipboardGuard;

    impl Drop for ClipboardGuard {
        fn drop(&mut self) {
            unsafe {
                CloseClipboard();
            }
        }
    }

    fn open_clipboard() -> Result<ClipboardGuard, String> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            unsafe {
                if OpenClipboard(std::ptr::null_mut()) != 0 {
                    return Ok(ClipboardGuard);
                }
            }

            if std::time::Instant::now() >= deadline {
                return Err(format!(
                    "OpenClipboard failed: {}",
                    std::io::Error::last_os_error()
                ));
            }

            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    fn read_unicode_clipboard() -> Result<String, String> {
        unsafe {
            let _guard = open_clipboard()?;

            let handle = GetClipboardData(CF_UNICODETEXT as u32);
            if handle.is_null() {
                return Err(format!(
                    "GetClipboardData(CF_UNICODETEXT) failed: {}",
                    std::io::Error::last_os_error()
                ));
            }

            let ptr = GlobalLock(handle).cast::<u16>();
            if ptr.is_null() {
                return Err(format!(
                    "GlobalLock failed: {}",
                    std::io::Error::last_os_error()
                ));
            }

            let mut len = 0;
            while *ptr.add(len) != 0 {
                len += 1;
            }
            let slice = std::slice::from_raw_parts(ptr, len);
            let text = String::from_utf16(slice).map_err(|e| format!("invalid UTF-16: {e}"));
            GlobalUnlock(handle);
            text
        }
    }

    #[test]
    fn windows_clipboard_preserves_unicode_text() {
        let text = "\u{4e2d}\u{6587}\u{8f93}\u{5165}\u{1f680}";
        let mut prev = None;

        set(text, &mut prev).unwrap();

        assert_eq!(read_unicode_clipboard().unwrap(), text);
    }
}
