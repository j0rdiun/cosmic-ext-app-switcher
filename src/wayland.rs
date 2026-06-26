use std::sync::mpsc;

use anyhow::Result;
use wayland_client::{
    Connection, Dispatch, QueueHandle, event_created_child,
    protocol::{wl_registry, wl_seat},
};
use cosmic_protocols::{
    toplevel_info::v1::client::{
        zcosmic_toplevel_handle_v1::{self, ZcosmicToplevelHandleV1},
        zcosmic_toplevel_info_v1::{self, ZcosmicToplevelInfoV1},
    },
    toplevel_management::v1::client::zcosmic_toplevel_manager_v1::{
        self, ZcosmicToplevelManagerV1,
    },
};

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

pub fn spawn_wayland_thread() -> Result<(Vec<ToplevelEntry>, mpsc::SyncSender<ActivateCommand>)> {
    let (list_tx, list_rx) = mpsc::sync_channel(1);
    let (cmd_tx, cmd_rx)   = mpsc::sync_channel(1);

    std::thread::spawn(move || {
        if let Err(e) = wayland_thread_main(list_tx, cmd_rx) {
            log::error!("wayland thread: {e}");
        }
    });

    let toplevels = list_rx.recv()?;
    Ok((toplevels, cmd_tx))
}

struct AppData {
    toplevels: Vec<Toplevel>,
    _info:     Option<ZcosmicToplevelInfoV1>,  // must stay alive to receive events
    manager:   Option<ZcosmicToplevelManagerV1>,
    seat:      Option<wl_seat::WlSeat>,
}

struct Toplevel {
    handle:    ZcosmicToplevelHandleV1,
    app_id:    String,
    title:     String,
    is_active: bool,
}

fn wayland_thread_main(
    list_tx: mpsc::SyncSender<Vec<ToplevelEntry>>,
    cmd_rx:  mpsc::Receiver<ActivateCommand>,
) -> Result<()> {
    let conn    = Connection::connect_to_env()?;
    let display = conn.display();
    let mut queue = conn.new_event_queue();
    let qh = queue.handle();

    let _registry = display.get_registry(&qh, ());

    let mut data = AppData { toplevels: vec![], _info: None, manager: None, seat: None };

    // Three roundtrips: registry → bind globals → receive all toplevel events
    queue.roundtrip(&mut data)?;
    queue.roundtrip(&mut data)?;
    queue.roundtrip(&mut data)?;

    // Sort: active window first (index 0 = current), rest in protocol order
    data.toplevels.sort_by_key(|t| if t.is_active { 0usize } else { 1 });

    let entries: Vec<ToplevelEntry> = data.toplevels.iter().enumerate()
        .filter(|(_, t)| !t.app_id.is_empty())
        .map(|(i, t)| ToplevelEntry {
            app_id:     t.app_id.clone(),
            title:      t.title.clone(),
            is_active:  t.is_active,
            handle_key: i,
        })
        .collect();

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
                    data._info = Some(
                        registry.bind::<ZcosmicToplevelInfoV1, _, _>(name, 1, qh, ())
                    );
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
                handle:    toplevel,
                app_id:    String::new(),
                title:     String::new(),
                is_active: false,
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
