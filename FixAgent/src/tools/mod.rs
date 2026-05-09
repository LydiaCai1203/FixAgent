//! Tool definitions for LLM tool calling.
//!
//! This module provides tools for the fix agent loop:
//! - `read_file`: Read file contents
//! - `search`: Search codebase for patterns

pub mod read_file;
pub mod search;

pub use read_file::{ReadFileArgs, ReadFileTool};
pub use search::{SearchArgs, SearchContext, SearchTool};

use thiserror::Error;

/// Error type for tools.
#[derive(Debug, Error)]
pub enum ToolError {
    #[error("Internal error: {0}")]
    Internal(String),
    #[error("File not found: {0}")]
    FileNotFound(String),
    #[error("Invalid arguments: {0}")]
    InvalidArgs(String),
}

impl From<String> for ToolError {
    fn from(s: String) -> Self {
        ToolError::InvalidArgs(s)
    }
}

/// Truncate a string to `max_bytes` at a valid UTF-8 boundary, appending "..." if truncated.
pub fn truncate_str(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        s.to_string()
    } else {
        let mut end = max_bytes;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &s[..end])
    }
}
