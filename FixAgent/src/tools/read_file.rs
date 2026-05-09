//! Read file tool for LLM tool calling.
//!
//! `ReadFileTool` allows the fix agent to read file contents from the workspace.

use std::path::PathBuf;

use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{ToolError, truncate_str};

/// Max total output size for batch reads (avoid blowing up context).
const MAX_READ_OUTPUT_CHARS: usize = 100_000;

#[derive(Debug, Deserialize, Serialize)]
pub struct ReadFileArgs {
    /// Single file path (use this or file_paths)
    #[serde(default)]
    pub file_path: Option<String>,
    /// Multiple file paths to read in one call
    #[serde(default)]
    pub file_paths: Option<Vec<String>>,
}

#[derive(Clone)]
pub struct ReadFileTool {
    workspace_root: PathBuf,
}

impl std::fmt::Debug for ReadFileTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReadFileTool")
            .field("workspace_root", &self.workspace_root)
            .finish()
    }
}

impl ReadFileTool {
    /// Create a new ReadFileTool with a workspace root.
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }

    fn resolve_path(&self, path: &str) -> Result<PathBuf, ToolError> {
        let resolved = if PathBuf::from(path).is_absolute() {
            PathBuf::from(path)
        } else {
            self.workspace_root.join(path)
        };
        let canonical = resolved.canonicalize().map_err(|e| {
            ToolError::FileNotFound(format!("Cannot resolve path '{}': {}", path, e))
        })?;
        let workspace_canonical = self
            .workspace_root
            .canonicalize()
            .unwrap_or(self.workspace_root.clone());
        if !canonical.starts_with(&workspace_canonical) {
            return Err(ToolError::InvalidArgs(format!(
                "Path '{}' is outside the workspace",
                path
            )));
        }
        Ok(canonical)
    }

    /// Resolve the list of file paths from args, normalizing single/multi input.
    fn resolve_file_list(args: &ReadFileArgs) -> Result<Vec<String>, ToolError> {
        match (&args.file_path, &args.file_paths) {
            (Some(p), _) if !p.is_empty() => {
                let mut files = vec![p.clone()];
                if let Some(paths) = &args.file_paths {
                    files.extend(paths.iter().cloned());
                }
                Ok(files)
            }
            (_, Some(paths)) if !paths.is_empty() => Ok(paths.clone()),
            _ => Err(ToolError::InvalidArgs(
                "Must provide file_path or file_paths".to_string(),
            )),
        }
    }

    /// Read a single file and return its content.
    async fn read_one(&self, path: &str) -> Result<String, ToolError> {
        let file_path = self.resolve_path(path)?;
        match tokio::fs::read_to_string(&file_path).await {
            Ok(content) => Ok(content),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(ToolError::FileNotFound(
                format!("File not found: {}", file_path.display()),
            )),
            Err(e) => Err(ToolError::Internal(format!("Failed to read file: {}", e))),
        }
    }
}

impl Tool for ReadFileTool {
    const NAME: &'static str = "read_file";
    type Error = ToolError;
    type Args = ReadFileArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Read file contents. Supports single file or batch reading multiple files. \
                Batch reads reduce tool call overhead."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Single file path (relative to workspace root)"
                    },
                    "file_paths": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Multiple file paths for batch reading (e.g. ['src/main.rs', 'src/lib.rs'])"
                    }
                }
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let args_str = serde_json::to_string(&args).unwrap_or_default();
        tracing::info!("[{}] input: {}", Self::NAME, args_str);

        let files = Self::resolve_file_list(&args)?;
        tracing::info!("[{}] reading {} file(s)", Self::NAME, files.len());

        let result = if files.len() == 1 {
            self.read_one(&files[0]).await
        } else {
            let mut output = String::new();
            let mut success_count = 0usize;
            for path in &files {
                output.push_str(&format!("=== {} ===\n", path));
                match self.read_one(path).await {
                    Ok(content) => {
                        output.push_str(&content);
                        if !content.ends_with('\n') {
                            output.push('\n');
                        }
                        success_count += 1;
                    }
                    Err(e) => {
                        output.push_str(&format!("[Error: {}]\n", e));
                    }
                }
                output.push('\n');
                if output.len() > MAX_READ_OUTPUT_CHARS {
                    output.push_str(&format!(
                        "[Output truncated, read {}/{} files]\n",
                        success_count,
                        files.len()
                    ));
                    break;
                }
            }
            Ok(output)
        };

        let output = match &result {
            Ok(s) => s.clone(),
            Err(e) => e.to_string(),
        };
        tracing::info!("[{}] output:\n{}", Self::NAME, truncate_str(&output, 5000));

        result
    }
}
