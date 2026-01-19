mod app;
mod config;
mod state;
mod ui;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "LDK Server GUI",
        options,
        Box::new(|cc| Ok(Box::new(app::LdkServerApp::new(cc)))),
    )
}
