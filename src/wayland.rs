use std::sync::mpsc;

use anyhow::Result;
use wayland_client::{
    Connection, Dispatch, QueueHandle, event_created_child,
    protocol::{wl_output, wl_registry, wl_seat},
};
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

#[derive(Debug, Clone)]
pub struct ToplevelEntry {
    pub app_id:     String,
    pub title:      String,
    pub is_active:  bool,
    pub handle_key: usize,
}

pub enum ActivateCommand {
    Activate(usize),
    Cancel,
}

pub fn spawn_wayland_thread(
    scope: WorkspaceScope,
) -> Result<(Vec<ToplevelEntry>, mpsc::SyncSender<ActivateCommand>)> {
    let (list_tx, list_rx) = mpsc::sync_channel(1);
    let (cmd_tx, cmd_rx)   = mpsc::sync_channel(1);

    std::thread::spawn(move || {
        if let Err(e) = wayland_thread_main(scope, list_tx, cmd_rx) {
            log::error!("wayland thread: {e}");
        }
    });

    let toplevels = list_rx.recv()?;
    Ok((toplevels, cmd_tx))
}

struct AppData {
    toplevels:          Vec<Toplevel>,
    _info:              Option<ZcosmicToplevelInfoV1>,  // must stay alive to receive events
    manager:            Option<ZcosmicToplevelManagerV1>,
    seat:               Option<wl_seat::WlSeat>,
    _workspace_manager: Option<ZcosmicWorkspaceManagerV1>,
    outputs:            Vec<wl_output::WlOutput>,
    // cosmic-comp does a one-time sync of each toplevel's output membership at handle
    // creation time, using whatever wl_output binds the client already has — so we must
    // not bind zcosmic_toplevel_info_v1 (which creates toplevel handles) until our own
    // wl_output binds have been sent first. Deferred here and bound explicitly after the
    // registry listing is fully drained, rather than reactively as its Global arrives.
    toplevel_info_name: Option<u32>,
}

struct Toplevel {
    handle:     ZcosmicToplevelHandleV1,
    app_id:     String,
    title:      String,
    is_active:  bool,
    outputs:    Vec<wl_output::WlOutput>,
    workspaces: Vec<ZcosmicWorkspaceHandleV1>,
}

fn wayland_thread_main(
    scope:   WorkspaceScope,
    list_tx: mpsc::SyncSender<Vec<ToplevelEntry>>,
    cmd_rx:  mpsc::Receiver<ActivateCommand>,
) -> Result<()> {
    let conn    = Connection::connect_to_env()?;
    let display = conn.display();
    let mut queue = conn.new_event_queue();
    let qh = queue.handle();

    let registry = display.get_registry(&qh, ());

    let mut data = AppData {
        toplevels: vec![],
        _info: None,
        manager: None,
        seat: None,
        _workspace_manager: None,
        outputs: vec![],
        toplevel_info_name: None,
    };

    // First roundtrip: drain the full registry listing. wl_output/wl_seat/workspace-manager
    // are bound immediately as their Globals arrive; zcosmic_toplevel_info_v1's bind is
    // deferred (see AppData::toplevel_info_name) regardless of where in the listing it
    // appears.
    queue.roundtrip(&mut data)?;

    // Second roundtrip: force the server to fully process our wl_output bind requests
    // before we bind toplevel_info below — otherwise toplevel handles it creates may miss
    // their initial output_enter sync (see comment on toplevel_info_name).
    queue.roundtrip(&mut data)?;

    if let Some(name) = data.toplevel_info_name.take() {
        data._info = Some(registry.bind::<ZcosmicToplevelInfoV1, _, _>(name, 1, &qh, ()));
    }

    // Extra roundtrips over the original three: workspace-group/workspace handles and
    // toplevel handles both need time to fully cascade before we read state back out.
    for _ in 0..5 {
        queue.roundtrip(&mut data)?;
    }

    // Sort: active window first (index 0 = current), rest in protocol order
    data.toplevels.sort_by_key(|t| if t.is_active { 0usize } else { 1 });

    // Scope filtering uses the previously-focused (active) window as the reference
    // point for "current workspace" / "current monitor" — there's no direct way to
    // query pointer/focus location via these protocols, but the window being
    // switched away from is a reliable proxy for both.
    let active_outputs: Vec<wl_output::WlOutput> = data.toplevels.iter()
        .find(|t| t.is_active)
        .map(|t| t.outputs.clone())
        .unwrap_or_default();
    // Kept for the future migration this unblocks (see CurrentWorkspace arm below),
    // but currently unread: legacy workspace_enter/workspace_leave never fire.
    let _active_workspaces: Vec<ZcosmicWorkspaceHandleV1> = data.toplevels.iter()
        .find(|t| t.is_active)
        .map(|t| t.workspaces.clone())
        .unwrap_or_default();

    let in_scope = |t: &Toplevel| -> bool {
        match scope {
            WorkspaceScope::AllWorkspaces => true,
            WorkspaceScope::CurrentWorkspace => {
                // Legacy workspace_enter/workspace_leave are never sent by cosmic-comp
                // (confirmed by reading its server source) — no membership data is
                // available via this protocol path. No-op until a migration to
                // ext_foreign_toplevel_list_v1 + ext_workspace_enter/leave lands.
                true
            }
            WorkspaceScope::CurrentOutput => {
                active_outputs.is_empty()
                    || t.outputs.iter().any(|o| active_outputs.contains(o))
            }
        }
    };

    let entries: Vec<ToplevelEntry> = data.toplevels.iter().enumerate()
        .filter(|(_, t)| (!t.app_id.is_empty() || !t.title.is_empty()) && (t.is_active || in_scope(t)))
        .map(|(i, t)| ToplevelEntry {
            app_id:     t.app_id.clone(),
            title:      t.title.clone(),
            is_active:  t.is_active,
            handle_key: i,
        })
        .collect();

    log::debug!(
        "scope={scope:?}: {}/{} toplevels in scope (active outputs={})",
        entries.len(), data.toplevels.len(), active_outputs.len(),
    );

    list_tx.send(entries).ok();

    match cmd_rx.recv() {
        Ok(ActivateCommand::Activate(key)) => {
            if let (Some(toplevel), Some(mgr), Some(seat)) = (
                data.toplevels.get(key),
                &data.manager,
                &data.seat,
            ) {
                mgr.activate(&toplevel.handle, seat);
                conn.flush()?;
            }
        }
        _ => {}
    }

    Ok(())
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
        if let wl_registry::Event::Global { name, interface, .. } = event {
            match interface.as_str() {
                "zcosmic_toplevel_info_v1" => {
                    // Binding deferred until after wl_output is bound — see
                    // AppData::toplevel_info_name and wayland_thread_main.
                    data.toplevel_info_name = Some(name);
                }
                "zcosmic_toplevel_manager_v1" => {
                    data.manager = Some(
                        registry.bind::<ZcosmicToplevelManagerV1, _, _>(name, 1, qh, ())
                    );
                }
                "wl_seat" => {
                    if data.seat.is_none() {
                        data.seat = Some(
                            registry.bind::<wl_seat::WlSeat, _, _>(name, 7, qh, ())
                        );
                    }
                }
                "wl_output" => {
                    data.outputs.push(
                        registry.bind::<wl_output::WlOutput, _, _>(name, 1, qh, ())
                    );
                }
                "zcosmic_workspace_manager_v1" => {
                    data._workspace_manager = Some(
                        registry.bind::<ZcosmicWorkspaceManagerV1, _, _>(name, 1, qh, ())
                    );
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
            data.toplevels.push(Toplevel {
                handle:     toplevel,
                app_id:     String::new(),
                title:      String::new(),
                is_active:  false,
                outputs:    vec![],
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
                t.is_active = state
                    .chunks_exact(4)
                    .any(|b| u32::from_ne_bytes(b.try_into().unwrap()) == activated);
            }
            zcosmic_toplevel_handle_v1::Event::Closed => {
                data.toplevels.retain(|t| &t.handle != handle);
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

// --- No-op dispatches for manager and seat ---

impl Dispatch<ZcosmicToplevelManagerV1, ()> for AppData {
    fn event(_: &mut Self, _: &ZcosmicToplevelManagerV1,
             _: zcosmic_toplevel_manager_v1::Event, _: &(),
             _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<wl_seat::WlSeat, ()> for AppData {
    fn event(_: &mut Self, _: &wl_seat::WlSeat, _: wl_seat::Event,
             _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<wl_output::WlOutput, ()> for AppData {
    fn event(_: &mut Self, _: &wl_output::WlOutput, _: wl_output::Event,
             _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

// --- Workspace hierarchy: bound only so zcosmic_toplevel_handle_v1's workspace_enter/
// leave events have valid zcosmic_workspace_handle_v1 objects to reference. We don't
// need names/coordinates/active-state off these — membership identity is enough to
// compare against the currently-active toplevel's own workspace list.

impl Dispatch<ZcosmicWorkspaceManagerV1, ()> for AppData {
    fn event(_: &mut Self, _: &ZcosmicWorkspaceManagerV1,
             _: zcosmic_workspace_manager_v1::Event, _: &(),
             _: &Connection, _: &QueueHandle<Self>) {}

    event_created_child!(AppData, ZcosmicWorkspaceManagerV1, [
        zcosmic_workspace_manager_v1::EVT_WORKSPACE_GROUP_OPCODE =>
            (ZcosmicWorkspaceGroupHandleV1, ()),
    ]);
}

impl Dispatch<ZcosmicWorkspaceGroupHandleV1, ()> for AppData {
    fn event(_: &mut Self, _: &ZcosmicWorkspaceGroupHandleV1,
             _: zcosmic_workspace_group_handle_v1::Event, _: &(),
             _: &Connection, _: &QueueHandle<Self>) {}

    event_created_child!(AppData, ZcosmicWorkspaceGroupHandleV1, [
        zcosmic_workspace_group_handle_v1::EVT_WORKSPACE_OPCODE =>
            (ZcosmicWorkspaceHandleV1, ()),
    ]);
}

impl Dispatch<ZcosmicWorkspaceHandleV1, ()> for AppData {
    fn event(_: &mut Self, _: &ZcosmicWorkspaceHandleV1,
             _: zcosmic_workspace_handle_v1::Event, _: &(),
             _: &Connection, _: &QueueHandle<Self>) {}
}
