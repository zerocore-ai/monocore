use thiserror::Error;

#[derive(Error, Debug)]
pub enum RaftError {
    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Transport error: {0}")]
    Transport(String),

    #[error("Invalid state: {0}")]
    InvalidState(String),

    #[error("Invalid term: {0}")]
    InvalidTerm(String),
}

pub type Result<T> = std::result::Result<T, RaftError>;
