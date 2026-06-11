#[cfg(target_os = "linux")]
pub fn paste() -> Result<(), String> {
    use evdev::{AttributeSet, InputEvent, KeyCode};
    use std::time::Duration;

    let mut keys = AttributeSet::<KeyCode>::new();
    keys.insert(KeyCode::KEY_LEFTCTRL);
    keys.insert(KeyCode::KEY_LEFTSHIFT);
    keys.insert(KeyCode::KEY_V);

    let mut dev = evdev::uinput::VirtualDevice::builder()
        .map_err(|e| format!("Cannot open /dev/uinput: {e}. Are you in the 'input' group?"))?
        .name("remote-input virtual keyboard")
        .with_keys(&keys)
        .map_err(|e| format!("Failed to set key capabilities: {e}"))?
        .build()
        .map_err(|e| format!("Cannot create uinput device: {e}. Are you in the 'input' group?"))?;

    std::thread::sleep(Duration::from_millis(100));

    let events: Vec<InputEvent> = [
        (KeyCode::KEY_LEFTCTRL, 1),
        (KeyCode::KEY_LEFTSHIFT, 1),
        (KeyCode::KEY_V, 1),
        (KeyCode::KEY_V, 0),
        (KeyCode::KEY_LEFTSHIFT, 0),
        (KeyCode::KEY_LEFTCTRL, 0),
    ]
    .map(|(code, value)| InputEvent::new(evdev::EventType::KEY.0, code.0, value))
    .to_vec();

    dev.emit(&events)
        .map_err(|e| format!("Failed to emit key events: {e}"))?;

    std::thread::sleep(Duration::from_millis(10));
    Ok(())
}

#[cfg(all(unix, not(target_os = "linux")))]
pub fn paste() -> Result<(), String> {
    use std::process::Command;
    let status = Command::new("xdotool")
        .args(["key", "ctrl+shift+v"])
        .status()
        .map_err(|e| format!("xdotool failed: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("xdotool exited with {status}"))
    }
}

#[cfg(windows)]
pub fn paste() -> Result<(), String> {
    use std::process::Command;
    let status = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Add-Type -AssemblyName System.Windows.Forms; \
             [System.Windows.Forms.SendKeys]::SendWait('^v')",
        ])
        .status()
        .map_err(|e| format!("Failed to run powershell: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("powershell exited with {status}"))
    }
}
