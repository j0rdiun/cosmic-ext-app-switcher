mod app;
mod icons;
mod ui;
mod wayland;

use anyhow::Result;
use clap::Parser;
use std::io::Write;
use std::os::unix::net::UnixStream;

const SOCKET: &str = "/tmp/cosmic-app-switcher.sock";

#[derive(Parser, Debug)]
#[command(name = "cosmic-app-switcher")]
pub struct Args {
    #[arg(long, default_value_t = false)]
    pub reverse: bool,
}

fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();

    // If a switcher is already running, signal it to cycle and exit.
    let cmd = if args.reverse { b"prev" as &[u8] } else { b"next" as &[u8] };
    if let Ok(mut s) = UnixStream::connect(SOCKET) {
        let _ = s.write_all(cmd);
        return Ok(());
    }

    // We are the first instance. Clean up any stale socket from a crash.
    let _ = std::fs::remove_file(SOCKET);

    let (toplevels, cmd_tx) = wayland::spawn_wayland_thread()?;
    if toplevels.is_empty() {
        return Ok(());
    }

    app::run(toplevels, args.reverse, cmd_tx)
}
