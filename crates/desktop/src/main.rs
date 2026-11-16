mod config_file;
mod theme;
mod ui;

fn main() -> iced::Result {
    #[cfg(target_os = "macos")]
    slopogram_macos::install();
    iced::application(ui::App::default, ui::App::update, ui::App::view)
        .title("Slopogram")
        .theme(|_app: &ui::App| theme::system_theme())
        .style(|_app: &ui::App, theme: &iced::Theme| iced::theme::Style {
            background_color: theme.palette().background.scale_alpha(0.80),
            text_color: theme.palette().text,
        })
        .subscription(ui::App::subscription)
        .window_size((1080.0, 760.0))
        .window(iced::window::Settings {
            min_size: Some(iced::Size::new(760.0, 540.0)),
            transparent: cfg!(target_os = "macos"),
            exit_on_close_request: false,
            ..Default::default()
        })
        .run()
}
