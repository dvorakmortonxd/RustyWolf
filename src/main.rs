//main.rs
mod cli;
mod webview;

use anyhow::Result;

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = cli::Cli::parse_args();
    webview::launch_webkit(&cli)
}
