//! Error types for the core engine.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid PE file: {0}")]
    Pe(String),

    #[error("walk error: {0}")]
    Walk(String),
}

pub type Result<T> = std::result::Result<T, Error>;
