mod app;
mod scanner;
mod strings;
mod thumbnail;
mod ui;
mod utils;
mod worker;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1024.0, 720.0]),
        ..Default::default()
    };

    eframe::run_native(
        strings::APP_TITLE,
        options,
        Box::new(|_cc| Ok(Box::new(app::AppState::default()))),
    )
}
