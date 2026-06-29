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

const CELL_PAD: u16 = 10;

pub fn view(state: &AppSwitcher) -> Element<Message> {
    let tv = &state.theme;
    let bg       = Color::from_rgba(tv.bg[0],          tv.bg[1],          tv.bg[2],          tv.bg[3]);
    let sel_bg   = Color::from_rgba(tv.selected_bg[0], tv.selected_bg[1], tv.selected_bg[2], tv.selected_bg[3]);
    let corner   = tv.corner_radius;
    let icon_sz  = tv.icon_size;

    let cells: Vec<Element<Message>> = state.toplevels
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let icon_name = icon_name_for(&entry.app_id);
            let selected  = i == state.selected;

            let app_icon: Element<Message> = icon::from_name(icon_name.as_str())
                .size(icon_sz)
                .icon()
                .size(icon_sz)
                .into();

            let cell = container(app_icon)
                .padding(CELL_PAD)
                .style(move |_: &cosmic::Theme| {
                    if selected {
                        ContainerStyle {
                            background: Some(Background::Color(sel_bg)),
                            border: Border {
                                radius: (corner - 2.0).into(),
                                width: 0.0,
                                color: Color::TRANSPARENT,
                            },
                            ..Default::default()
                        }
                    } else {
                        ContainerStyle::default()
                    }
                });

            mouse_area(cell)
                .on_press(Message::SelectIndex(i))
                .on_release(Message::Activate)
                .into()
        })
        .collect();

    let strip = container(
        row(cells).spacing(4).align_y(Alignment::Center)
    )
    .style(move |_: &cosmic::Theme| ContainerStyle {
        background: Some(Background::Color(bg)),
        border: Border {
            radius: corner.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        shadow: Shadow {
            color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.6 },
            offset: Vector::new(0.0, 8.0),
            blur_radius: 32.0,
        },
        ..Default::default()
    })
    .padding([14, 18]);

    container(strip)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}
