use clap::Parser;

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
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }
}
