use clap::{Parser, ValueEnum};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum LinuxBackend {
    Auto,
    X11,
    Wayland,
}

#[derive(Debug, Parser)]
#[command(
    name = "rustywolf",
    about = "Privacy-focused WebKit browser",
    long_about = "RustyWolf launches an embedded WebKit browser window.",
    version
)]
pub struct Cli {
    #[arg(long, help = "Optional startup URL (defaults to https://duckduckgo.com)")]
    pub url: Option<String>,

    #[arg(long, help = "Set a custom window title")]
    pub title: Option<String>,

    #[arg(long, help = "Try private/incognito mode when backend supports it")]
    pub private: bool,

    #[arg(long, help = "Print launch settings without opening a window")]
    pub dry_run: bool,

    #[arg(
        long,
        value_enum,
        default_value_t = LinuxBackend::Auto,
        help = "Linux only: prefer a windowing backend (auto, x11, wayland)"
    )]
    pub linux_backend: LinuxBackend,

    #[arg(
        long,
        help = "Linux only: disable WebKit dmabuf renderer (helps some NVIDIA/Wayland setups)"
    )]
    pub linux_disable_dmabuf: bool,
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }
}
