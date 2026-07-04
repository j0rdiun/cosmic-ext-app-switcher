mod app;
mod icons;
mod ui;
mod wayland;

use anyhow::Result;
use clap::Parser;
use std::io::Write;
use std::os::unix::net::UnixStream;
use cosmic::cosmic_config::ConfigGet;
use switcher_config::{Theme, APP_ID, CONFIG_VERSION};

pub fn socket_path() -> std::path::PathBuf {
    let dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
    std::path::PathBuf::from(dir).join("cosmic-ext-app-switcher.sock")
}

#[derive(Parser, Debug)]
#[command(name = "cosmic-ext-app-switcher")]
pub struct Args {
    #[arg(long, default_value_t = false)]
    pub reverse: bool,
}

fn load_theme() -> Theme {
    use cosmic::cosmic_config::Config;
    Config::new(APP_ID, CONFIG_VERSION)
        .ok()
        .and_then(|c| c.get::<Theme>("theme").ok())
        .unwrap_or_default()
}

fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();

    // If a switcher is already running, signal it to cycle and exit.
    let cmd = if args.reverse { b"prev" as &[u8] } else { b"next" as &[u8] };
    if let Ok(mut s) = UnixStream::connect(socket_path()) {
        let _ = s.write_all(cmd);
        return Ok(());
    }

    // We are the first instance. Clean up any stale socket from a crash.
    let _ = std::fs::remove_file(socket_path());

    let theme = load_theme();
    let (toplevels, cmd_tx) = wayland::spawn_wayland_thread()?;
    if toplevels.is_empty() {
        return Ok(());
    }

    app::run(toplevels, args.reverse, cmd_tx, theme)
}
