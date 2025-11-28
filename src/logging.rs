//! Logging utilities wrapping `tracing` initialisation

use crate::config::{LogRotation, LoggingOptions};
use crate::error::{Error, Result};
use std::fs::OpenOptions;
use std::io;
use std::path::Path;
use std::sync::OnceLock;
use tracing::Subscriber;
use tracing_appender::non_blocking::{self, WorkerGuard};
use tracing_appender::rolling;
use tracing_subscriber::fmt::time::UtcTime;
use tracing_subscriber::layer::{Layered, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer, Registry, fmt};

static FILE_GUARD: OnceLock<WorkerGuard> = OnceLock::new();

/// Initialise the global tracing subscriber according to the provided logging options.
///
/// Subsequent calls are ignored to avoid reinitialisation panics.
pub fn init(options: &LoggingOptions) -> Result<()> {
    if tracing::dispatcher::has_been_set() {
        // Already configured by tests or caller; nothing to do.
        return Ok(());
    }

    let level = std::env::var("QLINK_LOG_LEVEL").unwrap_or_else(|_| options.level.clone());
    let env_filter = EnvFilter::try_new(level.as_str())
        .map_err(|e| Error::Config(format!("Invalid log level '{level}': {e}")))?;

    let env_filter_for_file = env_filter.clone();

    if let Some(file_layer) = file_layer::<LayeredEnvFilter>(options)? {
        Registry::default()
            .with(env_filter_for_file)
            .with(file_layer)
            .with(stdout_layer::<_>(options.color))
            .try_init()
            .map_err(|e| Error::Config(format!("Failed to install tracing subscriber: {e}")))
    } else {
        Registry::default()
            .with(env_filter)
            .with(stdout_layer::<LayeredEnvFilter>(options.color))
            .try_init()
            .map_err(|e| Error::Config(format!("Failed to install tracing subscriber: {e}")))
    }
}

type LayeredEnvFilter = Layered<EnvFilter, Registry>;
type BoxedLayer<S> = Box<dyn Layer<S> + Send + Sync + 'static>;

fn file_layer<S>(options: &LoggingOptions) -> Result<Option<BoxedLayer<S>>>
where
    S: Subscriber + for<'span> LookupSpan<'span> + Send + Sync + 'static,
{
    let path = match options.file.as_ref() {
        Some(path) => path,
        None => return Ok(None),
    };

    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir).map_err(|e| {
        Error::Config(format!(
            "Failed to create log directory {}: {e}",
            dir.display()
        ))
    })?;

    let (non_blocking, guard) = match options.rotation {
        Some(rotation) => {
            let file_name = path.file_name().ok_or_else(|| {
                Error::Config(format!(
                    "Log file path '{}' must include a filename when rotation is enabled",
                    path.display()
                ))
            })?;

            let dir = path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            std::fs::create_dir_all(dir).map_err(|e| {
                Error::Config(format!(
                    "Failed to create log directory {}: {e}",
                    dir.display()
                ))
            })?;

            let appender = match rotation {
                LogRotation::Hourly => rolling::hourly(dir, file_name),
                LogRotation::Daily => rolling::daily(dir, file_name),
            };

            non_blocking::NonBlockingBuilder::default()
                .lossy(false)
                .finish(appender)
        }
        None => {
            let file = OpenOptions::new()
                .append(true)
                .create(true)
                .open(path)
                .map_err(|e| {
                    Error::Config(format!("Failed to open log file {}: {e}", path.display()))
                })?;

            non_blocking::NonBlockingBuilder::default()
                .lossy(false)
                .finish(file)
        }
    };
    // Store guard to keep background thread alive.
    let _ = FILE_GUARD.set(guard);

    Ok(Some(
        fmt::layer()
            .with_timer(UtcTime::rfc_3339())
            .with_ansi(false)
            .with_writer(non_blocking)
            .with_target(true)
            .with_level(true)
            .boxed(),
    ))
}

fn stdout_layer<S>(color: bool) -> BoxedLayer<S>
where
    S: Subscriber + for<'span> LookupSpan<'span> + Send + Sync + 'static,
{
    fmt::layer()
        .with_timer(UtcTime::rfc_3339())
        .with_writer(|| io::stdout())
        .with_ansi(color)
        .with_target(true)
        .with_level(true)
        .boxed()
}
