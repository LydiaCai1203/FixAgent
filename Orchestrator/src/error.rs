use thiserror::Error;

#[derive(Debug, Error)]
pub enum OrchestratorError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Configuration error: {0}")]
    Config(String),
}

pub type Result<T> = std::result::Result<T, OrchestratorError>;
