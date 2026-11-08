use std::sync::mpsc;

use anyhow::{Context, Result, anyhow};
use cosmic_protocols::{
    toplevel_info::v1::client::{
        zcosmic_toplevel_handle_v1::{self, ZcosmicToplevelHandleV1},
        zcosmic_toplevel_info_v1::{self, ZcosmicToplevelInfoV1},
    },
    toplevel_management::v1::client::zcosmic_toplevel_manager_v1::{
        self, ZcosmicToplevelManagerV1,
    },
    workspace::v1::client::{
        zcosmic_workspace_group_handle_v1::{self, ZcosmicWorkspaceGroupHandleV1},
        zcosmic_workspace_handle_v1::{self, ZcosmicWorkspaceHandleV1},
        zcosmic_workspace_manager_v1::{self, ZcosmicWorkspaceManagerV1},
    },
};
use switcher_config::WorkspaceScope;
use wayland_client::{
    Connection, Dispatch, QueueHandle, event_created_child,
    globals::{GlobalListContents, registry_queue_init},
    protocol::{wl_output, wl_registry, wl_seat},
};

#[derive(Debug, Clone)]
pub struct ToplevelEntry {
    pub app_id: String,
    pub title: String,
    pub handle_key: usize,
}

pub enum ActivateCommand {
    Activate(usize),
    Snapshot(WorkspaceScope, mpsc::SyncSender<Vec<ToplevelEntry>>),
}

// Versions we bind at. The compositor has to keep speaking the version a client bound, so
// newer versions it adds later don't affect us. zcosmic_toplevel_info_v1 must stay at 1:
// v2+ never emits Toplevel events.
const TOPLEVEL_INFO_VERSION: u32 = 1;
const TOPLEVEL_MANAGER_VERSION: u32 = 1;
const SEAT_VERSION: u32 = 7;

/// Globals the switcher can't work without, and the lowest version of each it accepts.
/// zwlr_layer_shell_v1 is bound by libcosmic on its own connection (for the overlay), not
/// by us, but cosmic-comp gates it the same way as the toplevel protocols, so it's checked
/// here too. libcosmic's toolkit accepts any version from 1.
const REQUIRED_GLOBALS: &[(&str, u32)] = &[
    ("zcosmic_toplevel_info_v1", TOPLEVEL_INFO_VERSION),
    ("zcosmic_toplevel_manager_v1", TOPLEVEL_MANAGER_VERSION),
    ("zwlr_layer_shell_v1", 1),
    ("wl_seat", SEAT_VERSION),
];

pub struct GlobalStatus {
    interface: &'static str,
    needed: u32,
    advertised: Option<u32>, // highest version offered, None if not offered at all
}

impl GlobalStatus {
    pub fn ok(&self) -> bool {
        self.advertised.is_some_and(|v| v >= self.needed)
    }
}

impl std::fmt::Display for GlobalStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (interface, needed) = (self.interface, self.needed);
        match self.advertised {
            Some(v) if v >= needed => write!(f, "{interface}: ok (v{v}, needs v{needed})"),
            Some(v) => write!(f, "{interface}: TOO OLD (v{v}, needs v{needed})"),
            None => write!(f, "{interface}: NOT offered (needs v{needed})"),
        }
    }
}

fn required_globals_status(advertised: &[(String, u32)]) -> Vec<GlobalStatus> {
    REQUIRED_GLOBALS
        .iter()
        .map(|&(interface, needed)| GlobalStatus {
            interface,
            needed,
            advertised: advertised
                .iter()
                .filter(|(i, _)| i == interface)
                .map(|&(_, v)| v)
                .max(),
        })
        .collect()
}

/// Connects to the compositor and reports on every required global, for `--check-compat`.
pub fn probe_required_globals() -> Result<Vec<GlobalStatus>> {
    let conn = Connection::connect_to_env()
        .context("could not connect to the Wayland compositor (is COSMIC running?)")?;
    let (globals, _queue) = registry_queue_init::<RegistryProbe>(&conn)?;
    let advertised: Vec<(String, u32)> = globals
        .contents()
        .clone_list()
        .into_iter()
        .map(|g| (g.interface, g.version))
        .collect();
    Ok(required_globals_status(&advertised))
}

pub fn spawn_wayland_thread(
    scope: WorkspaceScope,
) -> Result<(Vec<ToplevelEntry>, mpsc::SyncSender<ActivateCommand>)> {
    let (list_tx, list_rx) = mpsc::sync_channel(1);
    let (cmd_tx, cmd_rx) = mpsc::sync_channel(1);

    std::thread::spawn(move || {
        if let Err(e) = wayland_thread_main(scope, &list_tx, cmd_rx) {
            // Before the list is sent, main is still waiting in recv() and reports the
            // error itself. After that the receiver is gone, so all we can do is log.
            if let Err(mpsc::SendError(Err(e))) = list_tx.send(Err(e)) {
                log::error!("wayland thread: {e}");
            }
        }
    });

    let toplevels = list_rx.recv()??;
    Ok((toplevels, cmd_tx))
}

struct AppData {
    toplevels: Vec<Toplevel>,
    /// Stable toplevel keys, newest activation first. Handles absent from this list have
    /// never been observed active and retain compositor creation order in snapshots.
    mru: Vec<usize>,
    next_key: usize,
    _info: Option<ZcosmicToplevelInfoV1>, // must stay alive to receive events
    manager: Option<ZcosmicToplevelManagerV1>,
    seat: Option<wl_seat::WlSeat>,
    _workspace_manager: Option<ZcosmicWorkspaceManagerV1>,
    outputs: Vec<wl_output::WlOutput>,
    // cosmic-comp does a one-time sync of each toplevel's output membership at handle
    // creation time, using whatever wl_output binds the client already has — so we must
    // not bind zcosmic_toplevel_info_v1 (which creates toplevel handles) until our own
    // wl_output binds have been sent first. Deferred here and bound explicitly after the
    // registry listing is fully drained, rather than reactively as its Global arrives.
    toplevel_info_name: Option<u32>,
    globals: Vec<(String, u32)>, // every (interface, version) advertised
}

struct Toplevel {
    key: usize,
    handle: ZcosmicToplevelHandleV1,
    app_id: String,
    title: String,
    is_active: bool,
    outputs: Vec<wl_output::WlOutput>,
    workspaces: Vec<ZcosmicWorkspaceHandleV1>,
}

fn wayland_thread_main(
    scope: WorkspaceScope,
    list_tx: &mpsc::SyncSender<Result<Vec<ToplevelEntry>>>,
    cmd_rx: mpsc::Receiver<ActivateCommand>,
) -> Result<()> {
    let conn = Connection::connect_to_env()?;
    let display = conn.display();
    let mut queue = conn.new_event_queue();
    let qh = queue.handle();

    let registry = display.get_registry(&qh, ());

    let mut data = AppData {
        toplevels: vec![],
        mru: vec![],
        next_key: 0,
        _info: None,
        manager: None,
        seat: None,
        _workspace_manager: None,
        outputs: vec![],
        toplevel_info_name: None,
        globals: vec![],
    };

    // First roundtrip: drain the full registry listing. wl_output/wl_seat/workspace-manager
    // are bound immediately as their Globals arrive; zcosmic_toplevel_info_v1's bind is
    // deferred (see AppData::toplevel_info_name) regardless of where in the listing it
    // appears.
    queue.roundtrip(&mut data)?;

    // Without this, a missing global means an empty window list and a silent exit (or an
    // overlay whose selection can't be activated), which looks like a bug in the switcher.
    let missing: Vec<String> = required_globals_status(&data.globals)
        .iter()
        .filter(|s| !s.ok())
        .map(ToString::to_string)
        .collect();
    if !missing.is_empty() {
        return Err(anyhow!(
            "cosmic-ext-app-switcher can't run: the compositor doesn't offer the Wayland \
             interfaces it needs ({}). A COSMIC update may have removed or changed them, or \
             the switcher is running inside a sandbox such as Flatpak. Run \
             `cosmic-ext-app-switcher --check-compat` for details.",
            missing.join("; "),
        ));
    }

    // Second roundtrip: force the server to fully process our wl_output bind requests
    // before we bind toplevel_info below — otherwise toplevel handles it creates may miss
    // their initial output_enter sync (see comment on toplevel_info_name).
    queue.roundtrip(&mut data)?;

    if let Some(name) = data.toplevel_info_name.take() {
        data._info = Some(registry.bind::<ZcosmicToplevelInfoV1, _, _>(
            name,
            TOPLEVEL_INFO_VERSION,
            &qh,
            (),
        ));
    }

    // Extra roundtrips over the original three: workspace-group/workspace handles and
    // toplevel handles both need time to fully cascade before we read state back out.
    for _ in 0..5 {
        queue.roundtrip(&mut data)?;
    }

    let entries = data.snapshot(scope);

    list_tx.send(Ok(entries)).ok();

    // Remain connected after the overlay closes. State events received here are what
    // make focus changes from every source (not only this switcher) update the MRU.
    loop {
        queue.dispatch_pending(&mut data)?;

        while let Ok(command) = cmd_rx.try_recv() {
            match command {
                ActivateCommand::Activate(key) => {
                    if let (Some(toplevel), Some(mgr), Some(seat)) = (
                        data.toplevels.iter().find(|t| t.key == key),
                        &data.manager,
                        &data.seat,
                    ) {
                        mgr.activate(&toplevel.handle, seat);
                    }
                }
                ActivateCommand::Snapshot(scope, reply) => {
                    let _ = reply.send(data.snapshot(scope));
                }
            }
        }

        conn.flush()?;
        // A short blocking dispatch keeps the protocol responsive while bounding command
        // latency from shortcut invocations. Wayland has no cross-thread wake primitive.
        if let Some(guard) = queue.prepare_read() {
            use std::os::fd::AsRawFd;
            let mut fd = libc::pollfd {
                fd: conn.backend().poll_fd().as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            // SAFETY: `fd` points to one valid pollfd for the duration of this call.
            let ready = unsafe { libc::poll(&mut fd, 1, 20) };
            if ready > 0 && fd.revents & libc::POLLIN != 0 {
                guard.read()?;
            }
        }
    }
}

impl AppData {
    fn snapshot(&self, scope: WorkspaceScope) -> Vec<ToplevelEntry> {
        let active = self.toplevels.iter().find(|t| t.is_active);
        let active_outputs = active.map(|t| t.outputs.as_slice()).unwrap_or_default();

        let in_scope = |t: &Toplevel| match scope {
            WorkspaceScope::AllWorkspaces | WorkspaceScope::CurrentWorkspace => true,
            WorkspaceScope::CurrentOutput => {
                active_outputs.is_empty() || t.outputs.iter().any(|o| active_outputs.contains(&o))
            }
        };

        let protocol_order: Vec<_> = self.toplevels.iter().map(|t| t.key).collect();
        let ordered: Vec<&Toplevel> = ordered_keys(&self.mru, &protocol_order)
            .into_iter()
            .filter_map(|key| self.toplevels.iter().find(|t| t.key == key))
            .collect();

        let entries: Vec<_> = ordered
            .into_iter()
            .filter(|t| {
                (!t.app_id.is_empty() || !t.title.is_empty()) && (t.is_active || in_scope(t))
            })
            .map(|t| ToplevelEntry {
                app_id: t.app_id.clone(),
                title: t.title.clone(),
                handle_key: t.key,
            })
            .collect();
        log::debug!(
            "scope={scope:?}: {}/{} toplevels in scope",
            entries.len(),
            self.toplevels.len()
        );
        entries
    }
}

fn ordered_keys(mru: &[usize], protocol_order: &[usize]) -> Vec<usize> {
    let mut ordered: Vec<_> = mru
        .iter()
        .copied()
        .filter(|key| protocol_order.contains(key))
        .collect();
    ordered.extend(
        protocol_order
            .iter()
            .copied()
            .filter(|key| !mru.contains(key)),
    );
    ordered
}

// --- Registry: bind our three globals ---

impl Dispatch<wl_registry::WlRegistry, ()> for AppData {
    fn event(
        data: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            data.globals.push((interface.clone(), version));
            // A global offered below the version we bind is left unbound: binding it would
            // be a protocol error that kills the connection before the required-globals
            // check in wayland_thread_main can report it.
            match interface.as_str() {
                "zcosmic_toplevel_info_v1" if version >= TOPLEVEL_INFO_VERSION => {
                    // Binding deferred until after wl_output is bound — see
                    // AppData::toplevel_info_name and wayland_thread_main.
                    data.toplevel_info_name = Some(name);
                }
                "zcosmic_toplevel_manager_v1" if version >= TOPLEVEL_MANAGER_VERSION => {
                    data.manager = Some(registry.bind::<ZcosmicToplevelManagerV1, _, _>(
                        name,
                        TOPLEVEL_MANAGER_VERSION,
                        qh,
                        (),
                    ));
                }
                "wl_seat" if version >= SEAT_VERSION => {
                    if data.seat.is_none() {
                        data.seat = Some(registry.bind::<wl_seat::WlSeat, _, _>(
                            name,
                            SEAT_VERSION,
                            qh,
                            (),
                        ));
                    }
                }
                "wl_output" => {
                    data.outputs
                        .push(registry.bind::<wl_output::WlOutput, _, _>(name, 1, qh, ()));
                }
                "zcosmic_workspace_manager_v1" => {
                    data._workspace_manager =
                        Some(registry.bind::<ZcosmicWorkspaceManagerV1, _, _>(name, 1, qh, ()));
                }
                _ => {}
            }
        }
    }
}

// --- ZcosmicToplevelInfoV1: new toplevel events ---

impl Dispatch<ZcosmicToplevelInfoV1, ()> for AppData {
    fn event(
        data: &mut Self,
        _: &ZcosmicToplevelInfoV1,
        event: zcosmic_toplevel_info_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zcosmic_toplevel_info_v1::Event::Toplevel { toplevel } = event {
            let key = data.next_key;
            data.next_key += 1;
            data.toplevels.push(Toplevel {
                key,
                handle: toplevel,
                app_id: String::new(),
                title: String::new(),
                is_active: false,
                outputs: vec![],
                workspaces: vec![],
            });
        }
    }

    event_created_child!(AppData, ZcosmicToplevelInfoV1, [
        zcosmic_toplevel_info_v1::EVT_TOPLEVEL_OPCODE =>
            (ZcosmicToplevelHandleV1, ()),
    ]);
}

// --- ZcosmicToplevelHandleV1: per-window metadata ---

impl Dispatch<ZcosmicToplevelHandleV1, ()> for AppData {
    fn event(
        data: &mut Self,
        handle: &ZcosmicToplevelHandleV1,
        event: zcosmic_toplevel_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(t) = data.toplevels.iter_mut().find(|t| &t.handle == handle) else {
            return;
        };
        match event {
            zcosmic_toplevel_handle_v1::Event::AppId { app_id } => {
                t.app_id = app_id;
            }
            zcosmic_toplevel_handle_v1::Event::Title { title } => {
                t.title = title;
            }
            zcosmic_toplevel_handle_v1::Event::State { state } => {
                let activated = zcosmic_toplevel_handle_v1::State::Activated as u32;
                let is_active = state
                    .chunks_exact(4)
                    .any(|b| u32::from_ne_bytes(b.try_into().unwrap()) == activated);
                t.is_active = is_active;
                let key = t.key;
                if is_active {
                    data.mru.retain(|candidate| *candidate != key);
                    data.mru.insert(0, key);
                }
            }
            zcosmic_toplevel_handle_v1::Event::Closed => {
                let key = t.key;
                data.toplevels.retain(|t| &t.handle != handle);
                data.mru.retain(|candidate| *candidate != key);
            }
            zcosmic_toplevel_handle_v1::Event::OutputEnter { output } => {
                if !t.outputs.contains(&output) {
                    t.outputs.push(output);
                }
            }
            zcosmic_toplevel_handle_v1::Event::OutputLeave { output } => {
                t.outputs.retain(|o| o != &output);
            }
            zcosmic_toplevel_handle_v1::Event::WorkspaceEnter { workspace } => {
                if !t.workspaces.contains(&workspace) {
                    t.workspaces.push(workspace);
                }
            }
            zcosmic_toplevel_handle_v1::Event::WorkspaceLeave { workspace } => {
                t.workspaces.retain(|w| w != &workspace);
            }
            _ => {}
        }
    }
}

// --- Registry for probe_required_globals: registry_queue_init collects the global list
// itself, so there's nothing to handle here ---

struct RegistryProbe;

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for RegistryProbe {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

// --- No-op dispatches for manager and seat ---

impl Dispatch<ZcosmicToplevelManagerV1, ()> for AppData {
    fn event(
        _: &mut Self,
        _: &ZcosmicToplevelManagerV1,
        _: zcosmic_toplevel_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for AppData {
    fn event(
        _: &mut Self,
        _: &wl_seat::WlSeat,
        _: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_output::WlOutput, ()> for AppData {
    fn event(
        _: &mut Self,
        _: &wl_output::WlOutput,
        _: wl_output::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

// --- Workspace hierarchy: bound only so zcosmic_toplevel_handle_v1's workspace_enter/
// leave events have valid zcosmic_workspace_handle_v1 objects to reference. We don't
// need names/coordinates/active-state off these — membership identity is enough to
// compare against the currently-active toplevel's own workspace list.

impl Dispatch<ZcosmicWorkspaceManagerV1, ()> for AppData {
    fn event(
        _: &mut Self,
        _: &ZcosmicWorkspaceManagerV1,
        _: zcosmic_workspace_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }

    event_created_child!(AppData, ZcosmicWorkspaceManagerV1, [
        zcosmic_workspace_manager_v1::EVT_WORKSPACE_GROUP_OPCODE =>
            (ZcosmicWorkspaceGroupHandleV1, ()),
    ]);
}

impl Dispatch<ZcosmicWorkspaceGroupHandleV1, ()> for AppData {
    fn event(
        _: &mut Self,
        _: &ZcosmicWorkspaceGroupHandleV1,
        _: zcosmic_workspace_group_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }

    event_created_child!(AppData, ZcosmicWorkspaceGroupHandleV1, [
        zcosmic_workspace_group_handle_v1::EVT_WORKSPACE_OPCODE =>
            (ZcosmicWorkspaceHandleV1, ()),
    ]);
}

impl Dispatch<ZcosmicWorkspaceHandleV1, ()> for AppData {
    fn event(
        _: &mut Self,
        _: &ZcosmicWorkspaceHandleV1,
        _: zcosmic_workspace_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

#[cfg(test)]
mod tests {
    use super::{ordered_keys, required_globals_status};

    #[test]
    fn mru_windows_precede_unseen_windows_in_protocol_order() {
        assert_eq!(ordered_keys(&[30, 10], &[10, 20, 30, 40]), [30, 10, 20, 40]);
    }

    #[test]
    fn closed_mru_windows_are_ignored() {
        assert_eq!(ordered_keys(&[99, 20], &[10, 20, 30]), [20, 10, 30]);
    }

    fn advertised(globals: &[(&str, u32)]) -> Vec<(String, u32)> {
        globals.iter().map(|&(i, v)| (i.to_string(), v)).collect()
    }

    fn line_for(globals: &[(&str, u32)], interface: &str) -> String {
        required_globals_status(&advertised(globals))
            .iter()
            .map(ToString::to_string)
            .find(|l| l.starts_with(&format!("{interface}:")))
            .unwrap()
    }

    #[test]
    fn everything_offered_is_ok() {
        let globals = [
            ("zcosmic_toplevel_info_v1", 3),
            ("zcosmic_toplevel_manager_v1", 4),
            ("zwlr_layer_shell_v1", 5),
            ("wl_seat", 9),
        ];
        assert!(
            required_globals_status(&advertised(&globals))
                .iter()
                .all(|s| s.ok())
        );
    }

    #[test]
    fn missing_global_is_reported() {
        let globals = [
            ("zcosmic_toplevel_info_v1", 3),
            ("zwlr_layer_shell_v1", 5),
            ("wl_seat", 9),
        ];
        let statuses = required_globals_status(&advertised(&globals));
        assert_eq!(statuses.iter().filter(|s| !s.ok()).count(), 1);
        assert_eq!(
            line_for(&globals, "zcosmic_toplevel_manager_v1"),
            "zcosmic_toplevel_manager_v1: NOT offered (needs v1)",
        );
    }

    #[test]
    fn version_below_the_one_we_bind_is_too_old() {
        let globals = [("wl_seat", 5)];
        assert_eq!(
            line_for(&globals, "wl_seat"),
            "wl_seat: TOO OLD (v5, needs v7)"
        );
    }

    /// A seat advertised twice counts as its highest version.
    #[test]
    fn repeated_global_uses_highest_version() {
        let globals = [("wl_seat", 5), ("wl_seat", 8)];
        assert_eq!(line_for(&globals, "wl_seat"), "wl_seat: ok (v8, needs v7)");
    }
}
