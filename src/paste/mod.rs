#[cfg(not(windows))]
mod clipboard;
mod keystroke;

#[cfg(target_os = "linux")]
pub mod linux_check;

use std::time::Duration;

use tokio::sync::mpsc;
use tracing::{error, info};

/// Paste worker. On Windows the text is typed directly via SendInput
/// (works in Emacs and other apps that ignore clipboard paste);
/// on Unix it writes the clipboard then simulates a paste keystroke.
pub fn run(mut rx: mpsc::UnboundedReceiver<String>, delay: Duration) {
    let mut injector = Injector::new(delay);

    while let Some(text) = rx.blocking_recv() {
        match injector.inject(&text) {
            Ok(()) => info!("Text delivered"),
            Err(e) => error!("Injection failed: {e}"),
        }
    }

    injector.shutdown();
}

#[cfg(windows)]
struct Injector;

#[cfg(windows)]
impl Injector {
    fn new(_delay: Duration) -> Self {
        Self
    }

    fn inject(&mut self, text: &str) -> Result<(), String> {
        keystroke::type_text(text)
    }

    fn shutdown(&mut self) {}
}

#[cfg(not(windows))]
struct Injector {
    clipboard_child: Option<std::process::Child>,
    delay: Duration,
}

#[cfg(not(windows))]
impl Injector {
    fn new(delay: Duration) -> Self {
        Self {
            clipboard_child: None,
            delay,
        }
    }

    fn inject(&mut self, text: &str) -> Result<(), String> {
        clipboard::set(text, &mut self.clipboard_child)?;
        std::thread::sleep(self.delay);
        keystroke::paste()
    }

    fn shutdown(&mut self) {
        // Clean up the last wl-copy on shutdown.
        if let Some(mut child) = self.clipboard_child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
