use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum AichanError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("JSON error at {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("invalid identity file: {0}")]
    InvalidIdentity(String),

    #[error("invalid device file: {0}")]
    InvalidDevice(String),

    #[error("invalid memory file: {0}")]
    InvalidMemory(String),

    #[error("invalid protocol object: {0}")]
    InvalidProtocol(String),
}

pub type Result<T> = std::result::Result<T, AichanError>;

pub(crate) fn io_error(path: impl Into<PathBuf>, source: std::io::Error) -> AichanError {
    AichanError::Io {
        path: path.into(),
        source,
    }
}

pub(crate) fn json_error(path: impl Into<PathBuf>, source: serde_json::Error) -> AichanError {
    AichanError::Json {
        path: path.into(),
        source,
    }
}
