//! Tracing subscriber setup.

use std::path::Path;
use std::sync::Mutex;

use thiserror::Error;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;
use tracing_subscriber::{EnvFilter, Layer as _, fmt};

/// Filter used when `RUST_LOG` is not set.
pub const DEFAULT_FILTER: &str = "massived=info";

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Format {
    Text,
    Json,
}

#[derive(Debug, Error)]
pub enum LoggingError {
    #[error("invalid log filter {filter:?}: {source}")]
    Filter {
        filter: String,
        source: tracing_subscriber::filter::ParseError,
    },
    #[error("cannot open log file {path}: {source}")]
    File {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    #[error("a global subscriber is already installed")]
    AlreadySet,
}

/// Installs a stderr layer in `format` and, when `file` is given, a JSON
/// layer appending to it. The filter comes from `filter`, else `RUST_LOG`,
/// else [`DEFAULT_FILTER`].
pub fn init(format: Format, filter: Option<&str>, file: Option<&Path>) -> Result<(), LoggingError> {
    let directive = filter
        .map(ToOwned::to_owned)
        .or_else(|| std::env::var("RUST_LOG").ok())
        .unwrap_or_else(|| DEFAULT_FILTER.to_owned());
    let make_filter = || {
        EnvFilter::try_new(&directive).map_err(|source| LoggingError::Filter {
            filter: directive.clone(),
            source,
        })
    };
    let stderr = match format {
        Format::Text => fmt::layer().with_writer(std::io::stderr).boxed(),
        Format::Json => fmt::layer().json().with_writer(std::io::stderr).boxed(),
    }
    .with_filter(make_filter()?);
    let file_layer = file
        .map(|path| {
            let file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .map_err(|source| LoggingError::File {
                    path: path.to_owned(),
                    source,
                })?;
            Ok::<_, LoggingError>(
                fmt::layer()
                    .json()
                    .with_ansi(false)
                    .with_writer(Mutex::new(file))
                    .with_filter(make_filter()?),
            )
        })
        .transpose()?;
    tracing_subscriber::registry()
        .with(stderr)
        .with(file_layer)
        .try_init()
        .map_err(|_| LoggingError::AlreadySet)
}
