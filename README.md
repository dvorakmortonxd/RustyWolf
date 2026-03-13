# RustyWolf

RustyWolf is a minimal, privacy-focused browser shell built with Rust + WebKit.

- macOS: native `WKWebView`
- Linux: `WebKitGTK`

No Firefox install. No extension system. Fast startup, clean UI.

## What It Does

- Privacy-first defaults: DuckDuckGo home + search fallback from the URL bar
- Multi-tab browsing with back, forward, reload, and keyboard shortcuts (`Ctrl/Cmd+T`, `W`, `L`)
- Built-in custom adblock with top-bar toggle (`ADS`)
- Popup permission gate: asks before a site opens a new tab
- Download support with live progress panel (`D`) and session download history
- Properties page with session browsing history + download history
- Default window size `1000x600` (still fully resizable)
- Optional `--private` mode when backend support is available

## Run

```bash
cargo run --
```

```bash
cargo run -- --url https://example.com
cargo run -- --url example.com
cargo run -- --title "RustyWolf"
cargo run -- --private
cargo run -- --dry-run
```

## Privacy Notes

RustyWolf applies baseline hardening in page context:

- `navigator.doNotTrack = "1"`
- disables legacy `openDatabase`
- disables `RTCPeerConnection`

Adblocking is built-in and lightweight. It is not full uBlock Origin parity.

## Linux Setup

Install WebKitGTK development/runtime packages before building.

```bash
sudo apt update
sudo apt install -y libwebkit2gtk-4.1-dev libgtk-3-dev build-essential pkg-config
```

## Dev Quick Checks

```bash
cargo fmt
cargo check
cargo test
```
