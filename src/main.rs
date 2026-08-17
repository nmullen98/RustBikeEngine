mod app;

use app::EngineApp;
use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Motorbike Engine Lab")
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([860.0, 620.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Motorbike Engine Lab",
        options,
        Box::new(|context| Ok(Box::new(EngineApp::new(context)))),
    )
}
