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
    core:           Core,
    popup:          Option<WindowId>,
    current_theme:  Theme,
    config_handler: Option<cosmic::cosmic_config::Config>,
}

#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    PopupClosed(WindowId),
    SetTheme(Theme),
}

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

        (Self { core, popup: None, current_theme, config_handler }, Task::none())
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
        }
        Task::none()
    }

    fn view(&self) -> Element<Message> {
        self.core
            .applet
            .icon_button("preferences-desktop-theme-symbolic")
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

        let content = column![
            text("App Switcher Theme").size(14),
            row(swatches).spacing(12),
        ]
        .spacing(12)
        .padding(16)
        .align_x(Alignment::Center);

        self.core.applet.popup_container(content).into()
    }
}
