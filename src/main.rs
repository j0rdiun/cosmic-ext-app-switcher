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

    /// Which windows to list, overriding the configured scope for this invocation
    #[arg(long, value_enum)]
    pub scope: Option<ScopeArg>,

    /// Report whether the compositor offers the Wayland interfaces the switcher needs, then exit
    #[arg(long, default_value_t = false)]
    pub check_compat: bool,
}

/// The scopes worth offering on the command line. `CurrentWorkspace` is deliberately absent
/// for the same reason the applet's picker hides it: it's a no-op until the
/// ext_foreign_toplevel_list_v1 migration lands.
#[derive(Copy, Clone, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum ScopeArg {
    /// Every window, on every workspace and monitor
    All,
    /// Only windows on the monitor holding the window you're switching away from
    CurrentMonitor,
}

impl From<ScopeArg> for WorkspaceScope {
    fn from(arg: ScopeArg) -> Self {
        match arg {
            ScopeArg::All            => WorkspaceScope::AllWorkspaces,
            ScopeArg::CurrentMonitor => WorkspaceScope::CurrentOutput,
        }
    }
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
    // A --scope on the command line beats the applet's setting, so one binding can list
    // every window while another lists only this monitor's (#7).
    let scope = args.scope.map_or_else(load_scope, WorkspaceScope::from);
    let (toplevels, cmd_tx) = wayland::spawn_wayland_thread(scope)?;

    app::run(toplevels, args.reverse, cmd_tx, theme)
}

#[cfg(test)]
mod tests {
    use super::{Args, ScopeArg};
    use clap::Parser;
    use switcher_config::WorkspaceScope;

    #[test]
    fn scope_is_optional_so_the_applet_setting_still_applies() {
        assert_eq!(Args::parse_from(["switcher"]).scope, None);
    }

    #[test]
    fn scope_values_map_to_the_config_scopes() {
        let parsed = Args::parse_from(["switcher", "--scope", "current-monitor"]);
        assert_eq!(parsed.scope, Some(ScopeArg::CurrentMonitor));
        assert_eq!(WorkspaceScope::from(parsed.scope.unwrap()), WorkspaceScope::CurrentOutput);

        let parsed = Args::parse_from(["switcher", "--scope", "all", "--reverse"]);
        assert_eq!(WorkspaceScope::from(parsed.scope.unwrap()), WorkspaceScope::AllWorkspaces);
        assert!(parsed.reverse);
    }

    #[test]
    fn unknown_scopes_are_rejected() {
        // Including the one the picker hides, so it can't look like a working option.
        assert!(Args::try_parse_from(["switcher", "--scope", "current-workspace"]).is_err());
        assert!(Args::try_parse_from(["switcher", "--scope", "nonsense"]).is_err());
    }
}
