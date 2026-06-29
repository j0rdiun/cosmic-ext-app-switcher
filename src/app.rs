use anyhow::Result;
use std::sync::mpsc;
use cosmic::{
    app::{Application, Core, Settings, Task},
    iced::{
        self,
        event,
        keyboard::{self, key::Named},
        window::Id as WindowId,
        Event, Subscription,
    },
    iced::platform_specific::shell::wayland::commands::layer_surface::{
        Anchor, KeyboardInteractivity, Layer, get_layer_surface, destroy_layer_surface,
    },
    iced::platform_specific::runtime::wayland::layer_surface::SctkLayerSurfaceSettings,
    Element,
};
use crate::wayland::{ActivateCommand, ToplevelEntry};

pub struct AppSwitcher {
    core:           Core,
    pub toplevels:  Vec<ToplevelEntry>,
    pub selected:   usize,
    cmd_tx:         mpsc::SyncSender<ActivateCommand>,
    super_held:     bool,
    alt_held:       bool,
    surface_id:     WindowId,
}

#[derive(Debug, Clone)]
pub enum Message {
    SelectNext,
    SelectPrev,
    SelectIndex(usize),
    Activate,
    Cancel,
    KeyEvent(keyboard::Event),
}

impl Application for AppSwitcher {
    type Executor = cosmic::executor::Default;
    type Flags    = (Vec<ToplevelEntry>, bool, mpsc::SyncSender<ActivateCommand>);
    type Message  = Message;

    const APP_ID: &'static str = "io.github.cosmic-ext-app-switcher";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, flags: Self::Flags) -> (Self, Task<Message>) {
        let (toplevels, reverse, cmd_tx) = flags;
        let n = toplevels.len();
        let selected = if reverse {
            n.saturating_sub(1)
        } else {
            1.min(n.saturating_sub(1))
        };

        // Each cell: icon(60) + padding(20) = 80px wide, 4px gap, strip padding(36) + margin(40)
        let n = toplevels.len() as u32;
        let surface_w = n * 80 + (n.saturating_sub(1)) * 4 + 36 + 40;
        let surface_h = 160u32;

        let surface_id = WindowId::unique();
        let layer_task = get_layer_surface::<cosmic::Action<Message>>(SctkLayerSurfaceSettings {
            id: surface_id,
            layer: Layer::Overlay,
            keyboard_interactivity: KeyboardInteractivity::Exclusive,
            anchor: Anchor::empty(),
            size: Some((Some(surface_w), Some(surface_h))),
            ..Default::default()
        });

        (
            AppSwitcher { core, toplevels, selected, cmd_tx, super_held: false, alt_held: false, surface_id },
            layer_task,
        )
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        let n = self.toplevels.len();
        match msg {
            Message::SelectNext => {
                self.selected = (self.selected + 1) % n;
            }
            Message::SelectPrev => {
                self.selected = (self.selected + n - 1) % n;
            }
            Message::SelectIndex(i) => {
                self.selected = i;
            }
            Message::Activate => {
                self.cmd_tx
                    .send(ActivateCommand::Activate(self.toplevels[self.selected].handle_key))
                    .ok();
                return Task::batch([
                    destroy_layer_surface(self.surface_id),
                    iced::exit(),
                ]);
            }
            Message::Cancel => {
                self.cmd_tx.send(ActivateCommand::Cancel).ok();
                return Task::batch([
                    destroy_layer_surface(self.surface_id),
                    iced::exit(),
                ]);
            }
            Message::KeyEvent(ke) => match ke {
                keyboard::Event::KeyPressed { key, modifiers, .. } => match key {
                    iced::keyboard::Key::Named(Named::Tab) => {
                        return self.update(if modifiers.shift() {
                            Message::SelectPrev
                        } else {
                            Message::SelectNext
                        });
                    }
                    iced::keyboard::Key::Named(Named::Escape) => {
                        return self.update(Message::Cancel);
                    }
                    iced::keyboard::Key::Named(Named::Enter) => {
                        return self.update(Message::Activate);
                    }
                    _ => {}
                },
                keyboard::Event::ModifiersChanged(mods) => {
                    let activate = (self.super_held && !mods.logo())
                        || (self.alt_held && !mods.alt());
                    self.super_held = mods.logo();
                    self.alt_held = mods.alt();
                    if activate {
                        return self.update(Message::Activate);
                    }
                }
                _ => {}
            },
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        let key_sub = event::listen_with(|event, _status, _window| {
            match event {
                Event::Keyboard(ke) => Some(Message::KeyEvent(ke)),
                Event::Window(iced::window::Event::Unfocused) => Some(Message::Cancel),
                _ => None,
            }
        });

        // Listen on the Unix socket for cycle commands from subsequent binary invocations.
        let socket_sub = {
            use std::any::TypeId;
            use cosmic::iced::futures::SinkExt;
            struct SocketSub;
            Subscription::run_with(TypeId::of::<SocketSub>(), |_| {
                cosmic::iced::stream::channel(16, |mut tx: cosmic::iced::futures::channel::mpsc::Sender<Message>| async move {
                    use tokio::io::AsyncReadExt;
                    let listener = match tokio::net::UnixListener::bind(crate::SOCKET) {
                        Ok(l) => l,
                        Err(_) => { std::future::pending::<()>().await; unreachable!() }
                    };
                    loop {
                        if let Ok((mut stream, _)) = listener.accept().await {
                            let mut buf = Vec::new();
                            let _ = stream.read_to_end(&mut buf).await;
                            let msg = match buf.as_slice() {
                                b"next" => Message::SelectNext,
                                b"prev" => Message::SelectPrev,
                                _ => continue,
                            };
                            let _ = tx.send(msg).await;
                        }
                    }
                })
            })
        };

        Subscription::batch([key_sub, socket_sub])
    }

    // Called for the main window (unused with no_main_window, but required by trait)
    fn view(&self) -> Element<'_, Message> {
        crate::ui::view(self)
    }

    // Called for our layer-shell surface
    fn view_window(&self, _id: WindowId) -> Element<'_, Message> {
        crate::ui::view(self)
    }
}

pub fn run(
    toplevels: Vec<ToplevelEntry>,
    reverse: bool,
    cmd_tx: mpsc::SyncSender<ActivateCommand>,
) -> Result<()> {
    let settings = Settings::default().no_main_window(true);

    cosmic::app::run::<AppSwitcher>(settings, (toplevels, reverse, cmd_tx))
        .map_err(|e| anyhow::anyhow!("{e:?}"))
}
