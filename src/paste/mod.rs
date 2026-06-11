mod clipboard;
mod keystroke;

#[cfg(target_os = "linux")]
pub mod linux_check;

use std::time::Duration;

use tokio::sync::mpsc;
use tracing::{error, info};

/// Paste worker: clipboard write → sleep → simulate paste keystroke.
pub fn run(mut rx: mpsc::UnboundedReceiver<String>, delay: Duration) {
    let mut clipboard_child: Option<std::process::Child> = None;

    while let Some(text) = rx.blocking_recv() {
        if let Err(e) = clipboard::set(&text, &mut clipboard_child) {
            error!("Clipboard write failed: {e}");
            continue;
        }

        std::thread::sleep(delay);

        if let Err(e) = keystroke::paste() {
            error!("Paste keystroke failed: {e}");
            continue;
        }

        info!("Pasted successfully");
    }

    // Clean up the last wl-copy on shutdown.
    if let Some(mut child) = clipboard_child {
        let _ = child.kill();
        let _ = child.wait();
    }
}
