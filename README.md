# About RustyWolf

RustyWolf is a minimal, privacy-focused browser shell built with Rust + WebKit.
- macOS: native `WKWebView`
- Linux: `WebKitGTK`

<img width="2912" height="1900" alt="image" src="https://github.com/user-attachments/assets/90d3880e-bb3e-44f5-8307-bc96011e24c8" />


# What It Does

- Really, I mean really lightweight! the reason I even made this is because I need a lightweight browser to run on my old macbook unibody 2010 4gb ddr3
- Privacy-first defaults: DuckDuckGo home + search fallback from the URL bar
- Multi-tab browsing with back, forward, reload, and keyboard shortcuts (`Ctrl/Cmd+T`, `W`, `L`)
- Built-in custom adblock with top-bar toggle (`ADS`)
- Popup permission gate: asks before a site opens a new tab
- Download support with live progress panel (`D`) and session download history
- Properties page with session browsing history + download history
- Default window size `1000x600` (still fully resizable)
- Optional `--private` mode when backend support is available
- Automatically clears all cookies and search history on exit

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
doas emerge --ask net-libs/webkit-gtk x11-libs/gtk+ virtual/pkgconfig dev-vcs/git
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

# MacOS

## installing Homebrew (skip if already installed)

```bash
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
```

## Setup (prerequisites)

```bash
brew install webkitgtk gtk+3 pkgconf git
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

# clone the code onto your device

```bash
git clone https://github.com/dvorakmortonxd/RustyWolf.git
```

# Dev Quick Checks

```bash
cargo fmt
cargo check
cargo test
```

# Run

## simple launch

```bash
cargo run --
```

## advanced options

```bash
cargo run -- --url https://example.com
cargo run -- --url example.com
cargo run -- --title "RustyWolf"
cargo run -- --private
cargo run -- --dry-run
```

# Privacy Notes

RustyWolf applies baseline hardening in page context:
- `navigator.doNotTrack = "1"`
- disables legacy `openDatabase`
- disables `RTCPeerConnection`

## btw
Adblocking is built-in and lightweight. It is not full uBlock Origin parity. But it even blocks YouTube videos, like, they literally vanish when you toggle it on while watching lol, the browser will even ask you when the website wants to redirect you somewhere so you dont get spammed by multiple tabs. 

# Enjoy!
