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
pub fn type_text(text: &str) -> Result<(), String> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, SendInput,
        VK_BACK, VK_RETURN, VK_TAB,
    };

    fn key_event(vk: u16, scan: u16, flags: u32) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: scan,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    fn tap(vk: u16) -> [INPUT; 2] {
        [key_event(vk, 0, 0), key_event(vk, 0, KEYEVENTF_KEYUP)]
    }

    let mut inputs: Vec<INPUT> = Vec::with_capacity(text.len() * 2 + 16);
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            // \r\n counts as one Enter; standalone \r or \n also one Enter.
            // Every line break must map to exactly one Enter keypress so
            // blank lines / multiple newlines are preserved.
            '\r' => {
                inputs.extend(tap(VK_RETURN));
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
            }
            '\n' => inputs.extend(tap(VK_RETURN)),
            '\t' => inputs.extend(tap(VK_TAB)),
            '\u{8}' => inputs.extend(tap(VK_BACK)),
            // Anything else goes in as a raw UTF-16 character, so Chinese,
            // emoji (surrogate pairs), etc. need no IME or keyboard layout.
            ch => {
                let mut buf = [0u16; 2];
                for unit in ch.encode_utf16(&mut buf) {
                    inputs.extend([
                        key_event(0, *unit, KEYEVENTF_UNICODE),
                        key_event(0, *unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP),
                    ]);
                }
            }
        }
    }

    // Send in batches so very long payloads don't exceed a single call.
    for batch in inputs.chunks(256) {
        let sent = unsafe {
            SendInput(
                batch.len() as u32,
                batch.as_ptr(),
                std::mem::size_of::<INPUT>() as i32,
            )
        };
        if sent != batch.len() as u32 {
            return Err(format!("SendInput failed: {}", std::io::Error::last_os_error()));
        }
    }

    Ok(())
}
