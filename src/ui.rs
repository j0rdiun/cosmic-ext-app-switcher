use cosmic::{
    iced::{
        Alignment, Border, Color, Length, Shadow, Vector,
        Background,
        widget::container::Style as ContainerStyle,
    },
    widget::{container, row, mouse_area},
    widget::icon,
    Element,
};
use crate::app::{AppSwitcher, Message};
use crate::icons::icon_name_for;

// macOS CMD+Tab palette — always dark regardless of system theme
const BG:          Color = Color { r: 0.13, g: 0.13, b: 0.13, a: 0.92 };
const SELECTED_BG: Color = Color { r: 1.0,  g: 1.0,  b: 1.0,  a: 0.25 };

const ICON_SIZE: u16 = 60;
const CELL_PAD:  u16 = 10;

pub fn view(state: &AppSwitcher) -> Element<Message> {
    let cells: Vec<Element<Message>> = state.toplevels
        .iter()
        .enumerate()
        .map(|(i, entry)| build_cell(i, &entry.app_id, i == state.selected))
        .collect();

    let strip = container(
        row(cells).spacing(4).align_y(Alignment::Center)
    )
    .style(strip_style)
    .padding([14, 18]);

    container(strip)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}

fn build_cell<'a>(index: usize, app_id: &str, selected: bool) -> Element<'a, Message> {
    let icon_name = icon_name_for(app_id);

    let app_icon: Element<Message> = icon::from_name(icon_name.as_str())
        .size(ICON_SIZE)
        .icon()
        .size(ICON_SIZE)
        .into();

    let cell = container(app_icon)
        .padding(CELL_PAD)
        .style(if selected { selected_style } else { transparent_style });

    mouse_area(cell)
        .on_press(Message::SelectIndex(index))
        .on_release(Message::Activate)
        .into()
}

fn strip_style(_theme: &cosmic::Theme) -> ContainerStyle {
    ContainerStyle {
        background: Some(Background::Color(BG)),
        border: Border {
            radius: 14.0.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        shadow: Shadow {
            color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.6 },
            offset: Vector::new(0.0, 8.0),
            blur_radius: 32.0,
        },
        ..Default::default()
    }
}

fn selected_style(_theme: &cosmic::Theme) -> ContainerStyle {
    ContainerStyle {
        background: Some(Background::Color(SELECTED_BG)),
        border: Border {
            radius: 12.0.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        ..Default::default()
    }
}

fn transparent_style(_theme: &cosmic::Theme) -> ContainerStyle {
    ContainerStyle::default()
}

