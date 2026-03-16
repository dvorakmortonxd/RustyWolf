# RustyWolf

RustyWolf is a minimal, privacy-focused browser shell built with Rust + system webviews.

- macOS: native `WKWebView`
- Linux: `WebKitGTK`
- Windows: `WebView2`

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

# Linux backend controls
cargo run -- --linux-backend x11
cargo run -- --linux-backend wayland
cargo run -- --linux-disable-dmabuf
cargo run -- --linux-backend x11 --linux-disable-dmabuf
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

### Linux Troubleshooting (Wayland/X11)

RustyWolf supports both X11 and Wayland, but some NVIDIA + WebKitGTK stacks can fail with dmabuf/gbm allocation errors.

Try these launch modes:

```bash
# safest fallback on many NVIDIA setups
cargo run -- --linux-backend x11 --linux-disable-dmabuf

# native wayland with dmabuf disabled
cargo run -- --linux-backend wayland --linux-disable-dmabuf

# auto backend, dmabuf disabled only
cargo run -- --linux-disable-dmabuf
```

## Windows Setup

Install the Microsoft Edge WebView2 Runtime (Evergreen), then build normally with Rust MSVC toolchain.

- WebView2 Runtime: https://developer.microsoft.com/microsoft-edge/webview2/
- Rust toolchain target: `x86_64-pc-windows-msvc`

## Dev Quick Checks

```bash
cargo fmt
cargo check
cargo test
```
