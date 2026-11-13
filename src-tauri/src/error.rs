use serde::{Serialize, Serializer};

/// Serialized as a plain message so the frontend can show it as is.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("terminal session {0} does not exist")]
    SessionNotFound(u32),
    #[error("pseudo-terminal error: {0}")]
    Pty(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Message(String),
}

impl Error {
    pub fn pty(err: impl std::fmt::Display) -> Self {
        Self::Pty(err.to_string())
    }
}

impl Serialize for Error {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
