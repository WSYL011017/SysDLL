//! Application-level error envelope surfaced across the Tauri/Rust boundary.
//!
//! Each Tauri command currently returns `Result<T, String>` which discards
//! structure on the way back. This type lets the GUI distinguish a real
//! I/O failure from a "no backups yet" status without resorting to
//! string-matching, and gives the Rust layer a single place to evolve the
//! taxonomy.

use serde::Serialize;
use sysdll_core::error::Error as CoreError;

#[derive(Debug, Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum AppError {
    /// Filesystem / network / OS rejection. Contains the underlying message.
    Io(String),
    /// A scan produced no `targets` because the user-supplied path failed
    /// sanitisation (UNC, .., non-existent, etc.).
    InvalidPath(String),
    /// A required predecessor step has not been completed yet.
    /// e.g. `restore_backup` before any `backup`.
    PreconditionFailed(String),
    /// Raised when the elevated CLI child can be found / spawned but
    /// has not advertised the expected manifest.
    ElevationMissing(String),
    /// Everything else, including unexpected `anyhow::Error`s.
    Other(String),
}

impl AppError {
    pub fn other(msg: impl Into<String>) -> Self {
        AppError::Other(msg.into())
    }

    pub fn invalid_path(msg: impl Into<String>) -> Self {
        AppError::InvalidPath(msg.into())
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Io(m) => write!(f, "io: {m}"),
            AppError::InvalidPath(m) => write!(f, "invalid path: {m}"),
            AppError::PreconditionFailed(m) => write!(f, "precondition: {m}"),
            AppError::ElevationMissing(m) => write!(f, "elevation missing: {m}"),
            AppError::Other(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for AppError {}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Io(err.to_string())
    }
}

impl From<CoreError> for AppError {
    fn from(err: CoreError) -> Self {
        AppError::Other(err.to_string())
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::Other(format!("{err:#}"))
    }
}

pub type AppResult<T> = std::result::Result<T, AppError>;
