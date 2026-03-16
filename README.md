# RustyWolf

RustyWolf is a minimal, privacy-focused browser shell built with Rust + system webviews.

- macOS: native `WKWebView`
- Linux: `WebKitGTK`
- Windows: `WebView2`

No Firefox install. No extension system. Fast startup, clean UI.

<img width="2912" height="1900" alt="image" src="https://github.com/user-attachments/assets/90d3880e-bb3e-44f5-8307-bc96011e24c8" />

## What It Does

- Really, I mean really lightweight! the reason I even made this is because I need a lightweight browser to run on my old macbook unibody 2010 4gb ddr3
    - So light in fact in my gentoo(openrc)+i3wm setup it took only 285mb ram with one tab open (dvorakmortonxd.dev) compared to firefox which took 866mb ram
    - THATS 3.04 TIMES AS MUCH RAM, so yeah it is light
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

# Linux Setup (prerequisites)

Install WebKitGTK development/runtime packages before building.

## Debian

```bash
sudo apt update
sudo apt install -y libwebkit2gtk-4.1-dev libgtk-3-dev build-essential pkg-config git
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

## Arch

```bash
sudo pacman -Sy
sudo pacman -S --needed webkit2gtk-4.1 gtk3 base-devel pkgconf git
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

## Gentoo

(u can replace doas with sudo btw)
```bash
doas emerge --sync
doas emerge --ask net-libs/webkit-gtk:4.1 x11-libs/gtk+ virtual/pkgconfig dev-vcs/git
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

# Linux Troubleshooting (Wayland/X11)

RustyWolf supports both X11 and native Wayland on Linux.

Some NVIDIA + WebKitGTK stacks can fail with dmabuf/gbm allocation errors, especially on Wayland.

Try these launch modes:

```bash
# safest fallback on many NVIDIA setups
cargo run -- --linux-backend x11 --linux-disable-dmabuf

# native wayland
cargo run -- --linux-backend wayland


# auto backend, dmabuf disabled only
cargo run -- --linux-disable-dmabuf
```

# MacOS

## installing Homebrew (skip if already installed)

```bash
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
```

# Windows Setup

Install the Microsoft Edge WebView2 Runtime (Evergreen), then build normally with Rust MSVC toolchain.

- WebView2 Runtime: https://developer.microsoft.com/microsoft-edge/webview2/
- Rust toolchain target: `x86_64-pc-windows-msvc`

## Dev Quick Checks

```bash
cargo fmt
cargo check
cargo test
```
