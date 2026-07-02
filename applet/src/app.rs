use cosmic::{
    app::{Application, Core, Task},
    iced::{
        Alignment, Background, Border, Color, Length,
        window::Id as WindowId,
        widget::container::Style as ContainerStyle,
    },
    widget::{column, container, mouse_area, row, text},
    Element,
};
use cosmic::iced::platform_specific::shell::commands::popup::{destroy_popup, get_popup};
use cosmic::cosmic_config::{ConfigGet, ConfigSet};
use switcher_config::{Theme, APP_ID, CONFIG_VERSION};

const APPLET_ID: &str = "io.github.cosmic-ext-applet-app-switcher";

pub struct AppletApp {
    core:                Core,
    popup:               Option<WindowId>,
    current_theme:       Theme,
    config_handler:      Option<cosmic::cosmic_config::Config>,
    shortcut_configured: bool,
    shortcut_error:      Option<String>,
}

#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    PopupClosed(WindowId),
    SetTheme(Theme),
    ToggleShortcut(bool),
}

// ---------------------------------------------------------------------------
// Shortcut helpers
// ---------------------------------------------------------------------------

fn shortcuts_config_path() -> Option<std::path::PathBuf> {
    let config_home = std::env::var("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config")
        });

    let shortcuts_dir = config_home
        .join("cosmic")
        .join("com.system76.CosmicSettings.Shortcuts");

    if !shortcuts_dir.exists() {
        return None;
    }

    // Mirror find-config.sh: find existing system_actions in highest version dir.
    if let Ok(entries) = std::fs::read_dir(&shortcuts_dir) {
        let mut version_dirs: Vec<_> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir() && e.file_name().to_string_lossy().starts_with('v'))
            .map(|e| e.path())
            .collect();
        version_dirs.sort();

        for dir in version_dirs.iter().rev() {
            let p = dir.join("system_actions");
            if p.exists() {
                return Some(p);
            }
        }

        if let Some(latest) = version_dirs.last() {
            return Some(latest.join("system_actions"));
        }
    }

    Some(shortcuts_dir.join("v1").join("system_actions"))
}

fn shortcut_is_configured() -> bool {
    shortcuts_config_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|s| s.contains("cosmic-ext-app-switcher") || s.contains("cosmic-app-switcher"))
        .unwrap_or(false)
}

fn switcher_exec() -> String {
    if std::env::var("FLATPAK_ID").is_ok() {
        format!("flatpak run --command=cosmic-ext-app-switcher {APPLET_ID}")
    } else {
        let home = std::env::var("HOME").unwrap_or_default();
        format!("{home}/.local/bin/cosmic-ext-app-switcher")
    }
}

fn do_unregister_shortcut() -> Result<(), String> {
    let path = match shortcuts_config_path() {
        Some(p) => p,
        None => return Ok(()),
    };
    if !path.exists() {
        return Ok(());
    }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let filtered: String = content
        .lines()
        .filter(|l| {
            let t = l.trim();
            !t.starts_with("WindowSwitcher:") && !t.starts_with("WindowSwitcherPrevious:")
        })
        .flat_map(|l| [l, "\n"])
        .collect();
    std::fs::write(&path, filtered).map_err(|e| e.to_string())
}

fn do_register_shortcut() -> Result<(), String> {
    let path = shortcuts_config_path()
        .ok_or_else(|| "COSMIC shortcuts directory not found — is COSMIC installed?".to_string())?;

    let cmd = switcher_exec();

    std::fs::create_dir_all(path.parent().unwrap())
        .map_err(|e| e.to_string())?;

    let content = if let Ok(existing) = std::fs::read_to_string(&path) {
        if existing.contains("cosmic-ext-app-switcher") {
            return Ok(());
        }
        let last_brace = existing.rfind('}').unwrap_or(existing.len());
        let before = existing[..last_brace].trim_end();
        format!(
            "{before}\n    WindowSwitcher: \"{cmd}\",\n    WindowSwitcherPrevious: \"{cmd} --reverse\",\n}}\n"
        )
    } else {
        format!(
            "{{\n    WindowSwitcher: \"{cmd}\",\n    WindowSwitcherPrevious: \"{cmd} --reverse\",\n}}\n"
        )
    };

    std::fs::write(&path, content).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Application impl
// ---------------------------------------------------------------------------

impl Application for AppletApp {
    type Executor = cosmic::executor::Default;
    type Flags    = ();
    type Message  = Message;

    const APP_ID: &'static str = APPLET_ID;

    fn core(&self) -> &Core { &self.core }
    fn core_mut(&mut self) -> &mut Core { &mut self.core }

    fn init(core: Core, _flags: ()) -> (Self, Task<Message>) {
        let config_handler = cosmic::cosmic_config::Config::new(APP_ID, CONFIG_VERSION).ok();
        let current_theme  = config_handler
            .as_ref()
            .and_then(|c| c.get::<Theme>("theme").ok())
            .unwrap_or_default();

        (
            Self {
                core,
                popup: None,
                current_theme,
                config_handler,
                shortcut_configured: shortcut_is_configured(),
                shortcut_error: None,
            },
            Task::none(),
        )
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }

    fn on_close_requested(&self, id: WindowId) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::TogglePopup => {
                if let Some(popup_id) = self.popup.take() {
                    return destroy_popup(popup_id);
                }
                let new_id = WindowId::unique();
                self.popup = Some(new_id);
                let popup_settings = self.core.applet.get_popup_settings(
                    self.core.main_window_id().unwrap(),
                    new_id,
                    None,
                    None,
                    None,
                );
                return get_popup(popup_settings);
            }
            Message::PopupClosed(id) => {
                if self.popup == Some(id) {
                    self.popup = None;
                }
            }
            Message::SetTheme(theme) => {
                if let Some(handler) = &self.config_handler {
                    let _ = handler.set("theme", &theme);
                }
                self.current_theme = theme;
            }
            Message::ToggleShortcut(enable) => {
                let result = if enable {
                    do_register_shortcut()
                } else {
                    do_unregister_shortcut()
                };
                match result {
                    Ok(()) => {
                        self.shortcut_configured = enable;
                        self.shortcut_error = None;
                    }
                    Err(e) => self.shortcut_error = Some(e),
                }
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<Message> {
        let mut handle = cosmic::widget::icon::from_svg_bytes(
            include_bytes!("../data/io.github.cosmic-ext-applet-app-switcher-symbolic.svg")
                as &'static [u8],
        );
        handle.symbolic = true;
        self.core
            .applet
            .icon_button_from_handle(handle)
            .on_press(Message::TogglePopup)
            .into()
    }

    fn view_window(&self, _id: WindowId) -> Element<Message> {
        let swatches: Vec<Element<Message>> = Theme::all()
            .into_iter()
            .map(|t| {
                let preview_bg = {
                    let p = t.preview_bg();
                    Color::from_rgb(p[0], p[1], p[2])
                };
                let selected = t == self.current_theme;
                let label = t.label().to_string();
                let t_clone = t.clone();

                let swatch = container(
                    cosmic::widget::Space::new()
                )
                .width(Length::Fixed(68.0))
                .height(Length::Fixed(40.0))
                .style(move |_: &cosmic::Theme| ContainerStyle {
                    background: Some(Background::Color(preview_bg)),
                    border: Border {
                        radius: 6.0.into(),
                        width: if selected { 2.5 } else { 0.0 },
                        color: Color::from_rgb(0.38, 0.58, 1.0),
                    },
                    ..Default::default()
                });

                let card = column![
                    swatch,
                    text(label).size(11),
                ]
                .spacing(4)
                .align_x(Alignment::Center);

                mouse_area(card)
                    .on_press(Message::SetTheme(t_clone))
                    .into()
            })
            .collect();

        let shortcut_row = row![
            text("Super+Tab shortcut").size(14),
            cosmic::widget::Space::new().width(Length::Fill),
            cosmic::widget::toggler(self.shortcut_configured)
                .on_toggle(Message::ToggleShortcut),
        ]
        .align_y(Alignment::Center);

        let mut content = column![
            shortcut_row,
            cosmic::widget::divider::horizontal::default(),
            text("Theme").size(14),
            row(swatches).spacing(12),
        ]
        .spacing(12)
        .padding(16)
        .align_x(Alignment::Center);

        if let Some(err) = &self.shortcut_error {
            content = column![content, text(format!("Error: {err}")).size(10)]
                .spacing(4)
                .align_x(Alignment::Center);
        }

        self.core.applet.popup_container(content).into()
    }
}
