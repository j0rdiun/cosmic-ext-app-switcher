mod app;
mod icons;
mod ui;
mod wayland;

use anyhow::Result;
use clap::Parser;
use cosmic::cosmic_config::ConfigGet;
use std::io::Write;
use std::os::unix::net::UnixStream;
use switcher_config::{APP_ID, CONFIG_VERSION, Theme, WorkspaceScope};

pub fn socket_path() -> std::path::PathBuf {
    let dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
    std::path::PathBuf::from(dir).join("cosmic-ext-app-switcher.sock")
}

#[derive(Parser, Debug)]
#[command(name = "cosmic-ext-app-switcher", version)]
pub struct Args {
    #[arg(long, default_value_t = false)]
    pub reverse: bool,

    /// Report whether the compositor offers the Wayland interfaces the switcher needs, then exit
    #[arg(long, default_value_t = false)]
    pub check_compat: bool,
}

pub(crate) fn load_theme() -> Theme {
    use cosmic::cosmic_config::Config;
    Config::new(APP_ID, CONFIG_VERSION)
        .ok()
        .and_then(|c| c.get::<Theme>("theme").ok())
        .unwrap_or_default()
}

pub(crate) fn load_scope() -> WorkspaceScope {
    use cosmic::cosmic_config::Config;
    Config::new(APP_ID, CONFIG_VERSION)
        .ok()
        .and_then(|c| c.get::<WorkspaceScope>("workspace_scope").ok())
        .unwrap_or_default()
}

/// Prints one line per required Wayland global. Exits with status 1 if any is unusable.
fn check_compat() -> Result<()> {
    let statuses = wayland::probe_required_globals()?;
    for status in &statuses {
        println!("{status}");
    }
    if !statuses.iter().all(wayland::GlobalStatus::ok) {
        std::process::exit(1);
    }
    Ok(())
}

fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();

    if args.check_compat {
        return check_compat();
    }

    // If a switcher is already running, signal it to cycle and exit.
    let cmd = if args.reverse {
        b"prev" as &[u8]
    } else {
        b"next" as &[u8]
    };
    if let Ok(mut s) = UnixStream::connect(socket_path()) {
        let _ = s.write_all(cmd);
        return Ok(());
    }

    // We are the first instance. Clean up any stale socket from a crash.
    let _ = std::fs::remove_file(socket_path());

    let theme = load_theme();
    let scope = load_scope();
    let (toplevels, cmd_tx) = wayland::spawn_wayland_thread(scope)?;

    app::run(toplevels, args.reverse, cmd_tx, theme)
}
