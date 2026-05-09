//! Search-based code exploration tools.
//!
//! Uses `ignore` crate for .gitignore-aware file walking and `regex` for pattern matching.
//! These tools are used by the fix agent to explore the codebase before generating patches.

use std::path::PathBuf;

use ignore::WalkBuilder;
use regex::Regex;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{ToolError, truncate_str};

// Output size limits
const MAX_MATCHES: usize = 50;
const MAX_FILES: usize = 200;
const MAX_CONTEXT_LINES: usize = 5;
const MAX_OUTPUT_CHARS: usize = 20_000;

/// Shared context for search tools.
#[derive(Debug, Clone)]
pub struct SearchContext {
    pub workspace_root: PathBuf,
}

impl SearchContext {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }

    /// Create a .gitignore-aware file walker for the workspace.
    pub fn walker(&self) -> ignore::Walk {
        WalkBuilder::new(&self.workspace_root)
            .hidden(true)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .build()
    }
}

// ============================================================================
// Unified Search Tool
// ============================================================================

#[derive(Debug, Deserialize, Serialize)]
pub struct SearchArgs {
    /// Regex pattern to search for (optional, if not provided only file_glob is used)
    pub pattern: Option<String>,
    /// Glob filter for file paths (optional)
    pub file_glob: Option<String>,
    /// Context lines before/after match (default 2, max 5, only for content search)
    pub context_lines: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct SearchTool {
    ctx: SearchContext,
}

impl SearchTool {
    pub fn new(ctx: SearchContext) -> Self {
        Self { ctx }
    }
}

impl Tool for SearchTool {
    const NAME: &'static str = "search";
    type Error = ToolError;
    type Args = SearchArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Search codebase - regex search file contents (like grep), or find files by glob (like find), or both combined. \
                Automatically skips .gitignore files and build artifacts.\n\
                Usage:\n\
                - pattern only -> content search (find function definitions, types, references)\n\
                - file_glob only -> list matching files\n\
                - both -> search content within matching files\n\
                **Tip:** Use file_glob to narrow scope and avoid full-repo searches."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Regex pattern (e.g. 'fn process_payment', 'impl.*Handler', 'struct Config'). Leave empty to only filter by glob."
                    },
                    "file_glob": {
                        "type": "string",
                        "description": "Glob filter (e.g. '**/*.rs', 'src/**/*.go', '**/*billing*'). Leave empty to search all text files."
                    },
                    "context_lines": {
                        "type": "integer",
                        "description": "Context lines before/after each match (default 2, max 5, only for content search)"
                    }
                }
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let args_str = serde_json::to_string(&args).unwrap_or_default();
        tracing::info!("[{}] input: {}", Self::NAME, args_str);

        let ctx = self.ctx.clone();
        let result =
            tokio::task::spawn_blocking(move || SearchTool::call_inner_static(&ctx, &args))
                .await
                .map_err(|e| ToolError::Internal(format!("Task join error: {}", e)))?;

        let output = match &result {
            Ok(s) => s.clone(),
            Err(e) => e.to_string(),
        };

        tracing::info!("[{}] output: {} chars", Self::NAME, output.len());
        result
    }
}

impl SearchTool {
    fn call_inner_static(
        ctx: &SearchContext,
        args: &SearchArgs,
    ) -> Result<String, ToolError> {
        match (&args.pattern, &args.file_glob) {
            (Some(pattern), _) => {
                Self::search_content(ctx, pattern, args.file_glob.as_deref(), args.context_lines)
            }
            (None, Some(glob)) => {
                Self::search_files(ctx, glob)
            }
            (None, None) => Err(ToolError::InvalidArgs(
                "Must provide at least pattern (regex search) or file_glob (file listing)".to_string(),
            )),
        }
    }

    fn search_content(
        ctx: &SearchContext,
        pattern: &str,
        file_glob: Option<&str>,
        context_lines: Option<usize>,
    ) -> Result<String, ToolError> {
        let re = Regex::new(pattern)
            .map_err(|e| ToolError::InvalidArgs(format!("Invalid regex '{}': {}", pattern, e)))?;
        let ctx_lines = context_lines.unwrap_or(2).min(MAX_CONTEXT_LINES);

        let glob_pattern = file_glob
            .map(|g| {
                glob::Pattern::new(g)
                    .map_err(|e| ToolError::InvalidArgs(format!("Invalid glob '{}': {}", g, e)))
            })
            .transpose()?;

        let mut output = String::new();
        let mut match_count = 0usize;
        let mut file_count = 0usize;

        'outer: for entry in ctx.walker().flatten() {
            if entry.file_type().map(|t| !t.is_file()).unwrap_or(true) {
                continue;
            }
            let path = entry.path();

            if let Some(ref pat) = glob_pattern {
                let rel = path.strip_prefix(&ctx.workspace_root).unwrap_or(path);
                if !pat.matches_path(rel) {
                    continue;
                }
            }

            let bytes = match std::fs::read(path) {
                Ok(b) => b,
                Err(_) => continue,
            };
            if is_likely_binary(&bytes) {
                continue;
            }
            let text = match std::str::from_utf8(&bytes) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let lines: Vec<&str> = text.lines().collect();
            let rel_path = path.strip_prefix(&ctx.workspace_root).unwrap_or(path);
            let mut file_has_match = false;
            let mut last_emitted: usize = 0;

            for (idx, line) in lines.iter().enumerate() {
                if !re.is_match(line) {
                    continue;
                }

                if !file_has_match {
                    file_has_match = true;
                    file_count += 1;
                    output.push_str(&format!("=== {} ===\n", rel_path.display()));
                }

                let start = idx.saturating_sub(ctx_lines);
                let end = (idx + ctx_lines + 1).min(lines.len());
                let effective_start = start.max(last_emitted);
                if last_emitted > 0 && start > last_emitted {
                    output.push_str("  ...\n");
                }
                for (i, line) in lines[effective_start..end].iter().enumerate() {
                    let line_idx = effective_start + i;
                    if line_idx == idx {
                        output.push_str(&format!("{:5} > {}\n", line_idx + 1, line));
                    } else {
                        output.push_str(&format!("{:5} | {}\n", line_idx + 1, line));
                    }
                }
                last_emitted = end;
                output.push('\n');

                match_count += 1;
                if match_count >= MAX_MATCHES || output.len() > MAX_OUTPUT_CHARS {
                    output.push_str("[Results truncated, please narrow your search]\n");
                    break 'outer;
                }
            }
        }

        if match_count == 0 {
            return Ok(format!("No matches found for pattern: `{}`", pattern));
        }

        Ok(format!(
            "Found {} match(es) in {} file(s):\n\n{}",
            match_count, file_count, output
        ))
    }

    fn search_files(ctx: &SearchContext, glob_str: &str) -> Result<String, ToolError> {
        let pattern = glob::Pattern::new(glob_str)
            .map_err(|e| ToolError::InvalidArgs(format!("Invalid glob '{}': {}", glob_str, e)))?;

        let mut paths: Vec<String> = Vec::new();

        for entry in ctx.walker().flatten() {
            if entry.file_type().map(|t| !t.is_file()).unwrap_or(true) {
                continue;
            }
            let rel = entry
                .path()
                .strip_prefix(&ctx.workspace_root)
                .unwrap_or(entry.path());
            if pattern.matches_path(rel) {
                paths.push(rel.to_string_lossy().to_string());
                if paths.len() > MAX_FILES {
                    break;
                }
            }
        }

        if paths.is_empty() {
            return Ok(format!("No files found matching: `{}`", glob_str));
        }

        let truncated = paths.len() > MAX_FILES;
        if truncated {
            paths.truncate(MAX_FILES);
        }
        paths.sort();
        let mut output = format!("Found {} file(s):\n", paths.len());
        for p in &paths {
            output.push_str(&format!("  {}\n", p));
        }
        if truncated {
            output.push_str("[Results truncated, please narrow your search]\n");
        }

        Ok(output)
    }
}

/// Heuristic binary detection: check first 8KB for null bytes.
fn is_likely_binary(bytes: &[u8]) -> bool {
    let check_len = bytes.len().min(8192);
    bytes[..check_len].contains(&0u8)
}
