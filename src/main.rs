#![cfg_attr(feature = "slint-shell", allow(dead_code))]

mod app;
mod fonts;
mod macos;
mod renderer;
mod scanner;
mod strings;
mod thumbnail;
mod ui;
mod utils;
mod view_model;
mod worker;

#[cfg(feature = "slint-shell")]
mod slint_shell;

#[cfg(feature = "slint-shell")]
fn main() -> Result<(), slint::PlatformError> {
    slint_shell::run()
}

#[cfg(not(feature = "slint-shell"))]
fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Glow,
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1024.0, 720.0])
            .with_min_inner_size([900.0, 620.0])
            .with_decorations(false)
            .with_title(strings::APP_TITLE),
        ..Default::default()
    };

    eframe::run_native(
        strings::APP_TITLE,
        options,
        Box::new(|cc| Ok(Box::new(app::AppState::new(cc)))),
    )
}
