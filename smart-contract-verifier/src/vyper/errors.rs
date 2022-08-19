use std::{io, path::PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum VyperError {
    /// Internal vyper error
    #[error("Vyper Error: {0}")]
    VyperError(String),
    #[error("{0}")]
    Message(String),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    #[error(transparent)]
    SemverError(#[from] semver::Error),
    #[error(transparent)]
    Io(#[from] VyperIoError),
}

impl VyperError {
    pub(crate) fn io(err: io::Error, path: impl Into<PathBuf>) -> Self {
        VyperIoError::new(err, path).into()
    }
    pub(crate) fn vyper(msg: impl Into<String>) -> Self {
        VyperError::VyperError(msg.into())
    }
    pub fn msg(msg: impl Into<String>) -> Self {
        VyperError::Message(msg.into())
    }
}

#[derive(Debug, Error)]
#[error("\"{}\": {io}", self.path.display())]
pub struct VyperIoError {
    io: io::Error,
    path: PathBuf,
}

impl VyperIoError {
    pub fn new(io: io::Error, path: impl Into<PathBuf>) -> Self {
        Self { io, path: path.into() }
    }
}

impl From<VyperIoError> for io::Error {
    fn from(err: VyperIoError) -> Self {
        err.io
    }
}
