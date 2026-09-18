use crate::wayland::{ActivateCommand, ToplevelEntry};
use anyhow::Result;
use cosmic::{
    Element,
    app::{Application, Core, Settings, Task},
    iced::platform_specific::runtime::wayland::layer_surface::SctkLayerSurfaceSettings,
    iced::platform_specific::shell::wayland::commands::layer_surface::{
        Anchor, KeyboardInteractivity, Layer, destroy_layer_surface, get_layer_surface,
    },
    iced::{
        self, Event, Subscription, event,
        keyboard::{self, key::Named},
        window::Id as WindowId,
    },
};
use std::sync::mpsc;
use switcher_config::{Theme, ThemeValues};

pub struct AppSwitcher {
    core: Core,
    pub toplevels: Vec<ToplevelEntry>,
    pub selected: usize,
    cmd_tx: mpsc::SyncSender<ActivateCommand>,
    super_held: bool,
    alt_held: bool,
    surface_id: Option<WindowId>,
    pub theme: ThemeValues,
}

#[derive(Debug, Clone)]
pub enum Message {
    SelectNext,
    SelectPrev,
    SelectIndex(usize),
    Activate,
    Cancel,
    Invoke(bool),
    KeyEvent(keyboard::Event),
}

impl Application for AppSwitcher {
    type Executor = cosmic::executor::Default;
    type Flags = (
        Vec<ToplevelEntry>,
        bool,
        mpsc::SyncSender<ActivateCommand>,
        Theme,
    );
    type Message = Message;

    const APP_ID: &'static str = "io.github.cosmic-ext-app-switcher";

    fn core(&self) -> &Core {
        &self.core
    }
    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, flags: Self::Flags) -> (Self, Task<Message>) {
        let (toplevels, reverse, cmd_tx, theme_preset) = flags;
        let n = toplevels.len();
        let selected = if reverse {
            n.saturating_sub(1)
        } else {
            1.min(n.saturating_sub(1))
        };

        let theme = theme_preset.values();
        let (surface_id, layer_task) = if toplevels.is_empty() {
            (None, Task::none())
        } else {
            let (surface_w, surface_h) = crate::ui::surface_size(toplevels.len(), &theme);
            let surface_id = WindowId::unique();
            let task = get_layer_surface::<cosmic::Action<Message>>(SctkLayerSurfaceSettings {
                id: surface_id,
                layer: Layer::Overlay,
                keyboard_interactivity: KeyboardInteractivity::Exclusive,
                anchor: Anchor::empty(),
                size: Some((Some(surface_w), Some(surface_h))),
                ..Default::default()
            });
            (Some(surface_id), task)
        };

        (
            AppSwitcher {
                core,
                toplevels,
                selected,
                cmd_tx,
                super_held: false,
                alt_held: false,
                surface_id,
                theme,
            },
            layer_task,
        )
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        let n = self.toplevels.len();
        match msg {
            Message::SelectNext => {
                if n > 0 {
                    self.selected = (self.selected + 1) % n;
                }
            }
            Message::SelectPrev => {
                if n > 0 {
                    self.selected = (self.selected + n - 1) % n;
                }
            }
            Message::SelectIndex(i) => {
                self.selected = i;
            }
            Message::Activate => {
                let Some(surface_id) = self.surface_id.take() else {
                    return Task::none();
                };
                let Some(selected) = self.toplevels.get(self.selected) else {
                    return destroy_layer_surface(surface_id);
                };
                self.cmd_tx
                    .send(ActivateCommand::Activate(selected.handle_key))
                    .ok();
                return destroy_layer_surface(surface_id);
            }
            Message::Cancel => {
                if let Some(surface_id) = self.surface_id.take() {
                    return destroy_layer_surface(surface_id);
                }
            }
            Message::Invoke(reverse) => {
                if self.surface_id.is_some() {
                    return self.update(if reverse {
                        Message::SelectPrev
                    } else {
                        Message::SelectNext
                    });
                }

                let (reply_tx, reply_rx) = mpsc::sync_channel(1);
                if self
                    .cmd_tx
                    .send(ActivateCommand::Snapshot(crate::load_scope(), reply_tx))
                    .is_err()
                {
                    return Task::none();
                }
                let Ok(toplevels) = reply_rx.recv() else {
                    return Task::none();
                };
                if toplevels.is_empty() {
                    return Task::none();
                }

                self.toplevels = toplevels;
                self.theme = crate::load_theme().values();
                self.selected = if reverse {
                    self.toplevels.len().saturating_sub(1)
                } else {
                    1.min(self.toplevels.len().saturating_sub(1))
                };
                self.super_held = false;
                self.alt_held = false;

                let surface_id = WindowId::unique();
                let (surface_w, surface_h) =
                    crate::ui::surface_size(self.toplevels.len(), &self.theme);
                self.surface_id = Some(surface_id);
                return get_layer_surface::<cosmic::Action<Message>>(SctkLayerSurfaceSettings {
                    id: surface_id,
                    layer: Layer::Overlay,
                    keyboard_interactivity: KeyboardInteractivity::Exclusive,
                    anchor: Anchor::empty(),
                    size: Some((Some(surface_w), Some(surface_h))),
                    ..Default::default()
                });
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
                    let activate =
                        (self.super_held && !mods.logo()) || (self.alt_held && !mods.alt());
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
        let key_sub = event::listen_with(|event, _status, _window| match event {
            Event::Keyboard(ke) => Some(Message::KeyEvent(ke)),
            Event::Window(iced::window::Event::Unfocused) => Some(Message::Cancel),
            _ => None,
        });

        // Listen on the Unix socket for cycle commands from subsequent binary invocations.
        let socket_sub = {
            use cosmic::iced::futures::SinkExt;
            use std::any::TypeId;
            struct SocketSub;
            Subscription::run_with(TypeId::of::<SocketSub>(), |_| {
                cosmic::iced::stream::channel(
                    16,
                    |mut tx: cosmic::iced::futures::channel::mpsc::Sender<Message>| async move {
                        use tokio::io::AsyncReadExt;
                        let listener = match tokio::net::UnixListener::bind(crate::socket_path()) {
                            Ok(l) => l,
                            Err(_) => {
                                std::future::pending::<()>().await;
                                unreachable!()
                            }
                        };
                        loop {
                            if let Ok((mut stream, _)) = listener.accept().await {
                                let mut buf = Vec::new();
                                let _ = stream.read_to_end(&mut buf).await;
                                let msg = match buf.as_slice() {
                                    b"next" => Message::Invoke(false),
                                    b"prev" => Message::Invoke(true),
                                    _ => continue,
                                };
                                let _ = tx.send(msg).await;
                            }
                        }
                    },
                )
            })
        };

        Subscription::batch([key_sub, socket_sub])
    }

    fn view(&self) -> Element<'_, Message> {
        crate::ui::view(self)
    }

    fn view_window(&self, _id: WindowId) -> Element<'_, Message> {
        crate::ui::view(self)
    }
}

pub fn run(
    toplevels: Vec<ToplevelEntry>,
    reverse: bool,
    cmd_tx: mpsc::SyncSender<ActivateCommand>,
    theme: Theme,
) -> Result<()> {
    let settings = Settings::default().no_main_window(true);
    cosmic::app::run::<AppSwitcher>(settings, (toplevels, reverse, cmd_tx, theme))
        .map_err(|e| anyhow::anyhow!("{e:?}"))
}
