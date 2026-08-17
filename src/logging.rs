use directories::ProjectDirs;
use std::{
    backtrace::Backtrace,
    error::Error,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
use tracing_appender::non_blocking::WorkerGuard;

/// Keeps the asynchronous writer alive and exposes diagnostic file locations.
pub struct Logger {
    log_directory: PathBuf,
    crash_log_path: PathBuf,
    _guard: WorkerGuard,
}

impl Logger {
    /// Starts daily rotating logs and installs the global tracing subscriber.
    ///
    /// # Errors
    ///
    /// Returns an error if neither the application-data directory nor the system temporary
    /// directory can be created, or if another global tracing subscriber is already installed.
    pub fn init() -> Result<Self, Box<dyn Error + Send + Sync>> {
        let preferred = ProjectDirs::from("uk", "BikeEngineLab", "MotorbikeEngineSimulator")
            .map(|directories| directories.data_local_dir().join("logs"));
        let log_directory = writable_log_directory(preferred)?;
        let crash_log_path = log_directory.join("crash.log");

        let appender = tracing_appender::rolling::daily(&log_directory, "simulator.log");
        let (writer, guard) = tracing_appender::non_blocking(appender);
        tracing_subscriber::fmt()
            .with_ansi(false)
            .with_target(true)
            .with_thread_ids(true)
            .with_writer(writer)
            .try_init()?;

        Ok(Self {
            log_directory,
            crash_log_path,
            _guard: guard,
        })
    }

    #[must_use]
    pub fn log_directory(&self) -> &Path {
        &self.log_directory
    }

    #[must_use]
    pub fn crash_log_path(&self) -> &Path {
        &self.crash_log_path
    }

    /// Installs a hook that writes crash details synchronously before the normal panic output.
    pub fn install_panic_hook(&self) {
        let crash_log_path = self.crash_log_path.clone();
        let previous_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic_info| {
            let report = format!(
                "\n=== Motorbike Engine Lab crash ===\nSystem time: {:?}\n{panic_info}\nBacktrace:\n{}\n",
                std::time::SystemTime::now(),
                Backtrace::force_capture()
            );
            if let Ok(mut file) = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&crash_log_path)
            {
                let _ = file.write_all(report.as_bytes());
                let _ = file.flush();
            }
            tracing::error!(crash_log = %crash_log_path.display(), "application panic: {panic_info}");
            previous_hook(panic_info);
        }));
    }
}

fn writable_log_directory(preferred: Option<PathBuf>) -> Result<PathBuf, std::io::Error> {
    if let Some(path) = preferred
        && fs::create_dir_all(&path).is_ok()
    {
        return Ok(path);
    }
    let fallback = std::env::temp_dir().join("motorbike-engine-simulator-logs");
    fs::create_dir_all(&fallback)?;
    Ok(fallback)
}
