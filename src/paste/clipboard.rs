use std::time::Duration;

#[cfg(unix)]
pub fn set(text: &str, prev: &mut Option<std::process::Child>) -> Result<(), String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

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
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut child = Command::new("powershell")
        .args(["-NoProfile", "-Command", "$input | Set-Clipboard"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to spawn powershell: {e}"))?;

    child
        .stdin
        .take()
        .unwrap()
        .write_all(text.as_bytes())
        .map_err(|e| format!("Failed to write to powershell stdin: {e}"))?;

    let _ = child.wait();
    Ok(())
}
