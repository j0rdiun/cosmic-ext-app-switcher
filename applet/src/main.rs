mod app;

fn main() -> cosmic::iced::Result {
    cosmic::applet::run::<app::AppletApp>(())
}
