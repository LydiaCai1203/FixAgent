use thiserror::Error;

#[derive(Debug, Error)]
pub enum FixError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Issue selection error: {0}")]
    IssueSelection(String),

    #[error("Patch validation error: {0}")]
    PatchValidation(String),

    #[error("LLM error: {0}")]
    Llm(String),
}

pub type Result<T> = std::result::Result<T, FixError>;
