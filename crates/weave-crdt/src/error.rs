use thiserror::Error;

#[derive(Error, Debug)]
pub enum WeaveError {
    #[error("automerge error: {0}")]
    Automerge(#[from] automerge::AutomergeError),

    #[error("entity not found: {0}")]
    EntityNotFound(String),

    #[error("agent not found: {0}")]
    AgentNotFound(String),

    #[error("entity has no content: {0}")]
    NoContent(String),

    #[error("entity not in conflict: {0}")]
    NotInConflict(String),

    #[error("merge error: {0}")]
    MergeError(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, WeaveError>;
