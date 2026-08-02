# Remote Input

Type on your phone, paste into your desktop. Enter text in the browser, and it gets automatically clipboard-pasted at your cursor position.

## Video Demo

https://github.com/user-attachments/assets/267f14ea-7e5d-47e6-b0d3-1d0224213477

## Dependencies

### Linux

Clipboard utilities (paste simulation uses `/dev/uinput` directly, no extra packages needed):

```bash
# Arch
sudo pacman -S wl-clipboard

# Ubuntu / Debian
sudo apt install wl-clipboard
```

Your user must be in the `input` group to access `/dev/uinput`:

```bash
sudo usermod -aG input $USER
# Log out and back in for the change to take effect
```

### Windows

No extra dependencies needed. Text is typed directly with the native `SendInput` API in Unicode mode — each UTF-16 character is injected as raw input, so it handles Chinese/emoji without an IME.

## Build & Run

Prerequisites: [Rust](https://rustup.rs/) toolchain and [pnpm](https://pnpm.io/) (for the web frontend build).

```bash
cargo build --release
./target/release/remote-input
```

On Windows, run:

```powershell
.\target\release\remote-input.exe
```

The terminal will print the URL and a QR code for your phone to scan. HTTPS is enabled by default with a self-signed certificate.

## Options

```
  -p, --port <PORT>              Listening port [default: 48732]
  -D, --paste-delay <MS>         Delay between clipboard write and paste (ms, Linux only) [default: 20]
      --http                     Use HTTP instead of HTTPS (insecure, not recommended)
  -m, --max-connections <N>      Maximum number of distinct client IPs allowed at once [default: 1]
  -a, --allow <IP>               Only allow connections from these IPs (comma-separated)
```

## Features

- Voice/keyboard input, auto-paste to desktop on send
- History: pin, delete, expand long text, clear all
- Recording toggle to pause/resume history
- HTTPS by default with auto-generated self-signed certificate
- QR code in terminal for quick phone access
- Connection limit (default: 1 IP at a time)
- IP whitelist mode (`-a`)

## How Text Insertion Works

When you hit "Send", the text is inserted at the focused cursor position:

- **Linux**: Writes the text to the system clipboard (`wl-copy`/`xclip`), then simulates **Ctrl+Shift+V** via `/dev/uinput` injection. Works in terminals and most GUI applications. Requires the user to be in the `input` group.
- **Windows**: Types the text directly with `SendInput` + `KEYEVENTF_UNICODE` — each UTF-16 character is injected as raw input, so it needs no clipboard and no IME. (Terminal/console windows don't receive injected input; use the clipboard instead there.)

On Linux, the `--paste-delay` flag (default 20ms) controls the delay between the clipboard write and the keystroke, giving the system time to register the new clipboard contents. Increase it if text arrives empty or garbled. It is ignored on Windows.

## Network

Phone and desktop must be on the same LAN. Open the port in your firewall:

```bash
sudo ufw allow 48732/tcp
```

## Security

By default, only one client IP is allowed at a time. The first connection is accepted, others are rejected until it disconnects. Adjust with `-m`:

```bash
remote-input -m 3   # allow up to 3 IPs
```

To restrict access to specific IPs only, use `-a`:

```bash
remote-input -a 192.168.1.5,192.168.1.10
```

HTTPS is the default mode. Use `--http` only on trusted networks.

## Links

- [Linux.do](https://linux.do)
