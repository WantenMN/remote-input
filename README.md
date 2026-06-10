# Remote Input

Type on your phone, paste into your Linux desktop. Enter text in the browser, and it gets automatically clipboard-pasted at your cursor position.

## Dependencies

```bash
# Arch
sudo pacman -S wl-clipboard xdotool

# Ubuntu / Debian
sudo apt install wl-clipboard xdotool
```

## Build & Run

```bash
cargo build --release
./target/release/remote-input
```

The terminal will print the URL for your phone to open.

## Options

```
  -p, --port <PORT>              Listening port [default: 48732]
  -D, --paste-delay <MS>         Delay between clipboard write and paste (ms) [default: 20]
```

## Features

- Voice/keyboard input, auto-paste to desktop on send
- History: pin, delete, expand long text, clear all
- Recording toggle to pause/resume history

## Network

Phone and desktop must be on the same LAN. Open the port in your firewall:

```bash
sudo ufw allow 48732/tcp
```
