mod app;
mod logging;

use app::EngineApp;
use eframe::egui;
use logging::Logger;

fn main() -> eframe::Result {
    let logger = Logger::init().unwrap_or_else(|error| {
        eprintln!("failed to initialize application logging: {error}");
        std::process::exit(2);
    });
    logger.install_panic_hook();
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        os = std::env::consts::OS,
        architecture = std::env::consts::ARCH,
        log_directory = %logger.log_directory().display(),
        "application starting"
    );

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Motorbike Engine Lab")
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([860.0, 620.0]),
        ..Default::default()
    };
    let log_path = logger.log_directory().to_path_buf();
    let crash_path = logger.crash_log_path().to_path_buf();
    let result = eframe::run_native(
        "Motorbike Engine Lab",
        options,
        Box::new(move |context| Ok(Box::new(EngineApp::new(context, log_path, crash_path)))),
    );
    match &result {
        Ok(()) => tracing::info!("application closed normally"),
        Err(error) => tracing::error!(%error, "application terminated with an error"),
    }
    result
}
