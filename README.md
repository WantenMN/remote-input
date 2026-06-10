# Remote Input

Type on your phone, paste into your desktop. Enter text in the browser, and it gets automatically clipboard-pasted at your cursor position.

## Dependencies

### Linux

```bash
# Arch
sudo pacman -S wl-clipboard xdotool

# Ubuntu / Debian
sudo apt install wl-clipboard xdotool
```

### Windows

No extra dependencies needed. Uses PowerShell's `Set-Clipboard` and `SendKeys`.

## Build & Run

```bash
cargo build --release
./target/release/remote-input
```

The terminal will print the URL and a QR code for your phone to scan. HTTPS is enabled by default with a self-signed certificate.

## Options

```
  -p, --port <PORT>              Listening port [default: 48732]
  -D, --paste-delay <MS>         Delay between clipboard write and paste (ms) [default: 20]
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
