//! Fix orchestrator: orchestrates the complete fix workflow.
//!
//! `FixOrchestrator` coordinates the entire fix process:
//! 1. Prepare fix task (load issue, resolve file)
//! 2. Run FixAgent to generate patch (with or without tools)
//! 3. Validate patch via refine layer
//! 4. Apply or reject patch
//! 5. Return structured fix result

use std::path::{Path, PathBuf};

use crate::agent::{FixAgent, FIX_SYSTEM_PROMPT, build_fix_prompt};
use crate::config::FixAgentConfig;
use crate::error::{FixError, Result};
use crate::models::{
    FixExecutionStatus, FixPatchOutcome, FixResult, FixTask, LineRange,
};
use crate::refine::PatchValidator;
use reviewagent::llm::{Issue, LlmClient, ReviewResponse};

/// Orchestrates the complete fix workflow.
pub struct FixOrchestrator {
    repo_dir: PathBuf,
    config: FixAgentConfig,
    llm: LlmClient,
}

impl FixOrchestrator {
    pub async fn new(repo_dir: PathBuf) -> Result<Self> {
        let repo_dir = std::fs::canonicalize(&repo_dir).map_err(FixError::Io)?;
        let config = FixAgentConfig::load_or_default(&repo_dir)
            .map_err(|e| FixError::Config(e.to_string()))?;
        let review_config =
            crate::config::load_reviewagent_config(&repo_dir).map_err(|e| FixError::Config(e.to_string()))?;
        let llm = LlmClient::from_config(&review_config)
            .map_err(|e| FixError::Config(format!("Failed to initialize LLM: {}", e)))?;

        Ok(Self {
            repo_dir,
            config,
            llm,
        })
    }

    /// Run a fix from a ReviewAgent JSON report file.
    pub async fn run_from_file(
        &self,
        review_file: PathBuf,
        issue_index: usize,
        dry_run: bool,
    ) -> Result<FixResult> {
        let review = self.load_review(&review_file).await?;
        let issue = self.select_issue(&review, issue_index)?;
        let task = FixTask {
            issue_id: None,
            issue_index,
            issue: issue.clone(),
        };
        self.run_task(task, dry_run).await
    }

    /// Run a fix task directly.
    pub async fn run_task(&self,
        task: FixTask,
        dry_run: bool,
    ) -> Result<FixResult> {
        let target_file = self.resolve_target_file(&task.issue.file)?;
        let original_content = tokio::fs::read_to_string(&target_file).await?;

        // Build prompt
        let prompt = build_fix_prompt(
            &task.issue,
            &original_content,
            self.config.fix.context_lines,
        );

        // Run FixAgent
        let fix_agent = FixAgent::new(
            self.config.clone(),
            self.llm.clone(),
            self.repo_dir.clone(),
        )?;

        let patch = if self.config.agent.enabled {
            fix_agent.run(FIX_SYSTEM_PROMPT, &prompt).await?
        } else {
            fix_agent.run_simple(FIX_SYSTEM_PROMPT, &prompt).await?
        };

        // Validate patch
        let validator = PatchValidator::new(self.config.fix.max_replacement_lines);
        let corrections = validator.validate_strict(&task.issue,
            &patch,
            &original_content,
        )?;

        if !corrections.is_empty() {
            for correction in &corrections {
                tracing::info!("Patch correction: {}", correction);
            }
        }

        // Apply or reject patch
        let replacement_preview = patch.replacement.clone();
        let status = match patch.outcome {
            FixPatchOutcome::Apply => {
                if !dry_run {
                    let updated = apply_replacement(
                        &original_content,
                        patch.start_line,
                        patch.end_line,
                        &patch.replacement,
                    )?;
                    tokio::fs::write(&target_file, updated).await?;
                    FixExecutionStatus::Applied
                } else {
                    FixExecutionStatus::Suggested
                }
            }
            FixPatchOutcome::NeedsHuman => FixExecutionStatus::NeedsHuman,
            FixPatchOutcome::InvalidCandidate => FixExecutionStatus::InvalidCandidate,
        };

        Ok(FixResult {
            issue_id: task.issue_id,
            issue_index: task.issue_index,
            issue_title: task.issue.title.clone(),
            status,
            dry_run,
            file: patch.file,
            applied_range: LineRange {
                start: patch.start_line,
                end: patch.end_line,
            },
            summary: patch.summary,
            rationale: patch.rationale,
            verification_steps: patch.verification_steps,
            replacement_preview,
        })
    }

    async fn load_review(
        &self,
        review_file: &Path,
    ) -> Result<ReviewResponse> {
        let content = tokio::fs::read_to_string(review_file).await?;
        Ok(serde_json::from_str(&content)?)
    }

    fn select_issue<'a>(
        &self,
        review: &'a ReviewResponse,
        issue_index: usize,
    ) -> Result<&'a Issue> {
        if issue_index == 0 {
            return Err(FixError::IssueSelection(
                "issue_index must be a 1-based positive integer".to_string(),
            ));
        }

        review.issues.get(issue_index - 1).ok_or_else(|| {
            FixError::IssueSelection(format!(
                "Issue index {} out of range, total issues={}.",
                issue_index,
                review.issues.len()
            ))
        })
    }

    fn resolve_target_file(&self,
        relative_path: &str,
    ) -> Result<PathBuf> {
        let joined = self.repo_dir.join(relative_path);
        let canonical = std::fs::canonicalize(&joined).map_err(FixError::Io)?;

        if !canonical.starts_with(&self.repo_dir) {
            return Err(FixError::PatchValidation(format!(
                "Resolved file escapes repository root: {}",
                relative_path
            )));
        }

        Ok(canonical)
    }
}

/// Apply a replacement to file content.
fn apply_replacement(
    content: &str,
    start_line: usize,
    end_line: usize,
    replacement: &str,
) -> Result<String> {
    let mut lines: Vec<String> = content.lines().map(ToString::to_string).collect();

    if start_line == 0 || end_line == 0 || start_line > end_line || end_line > lines.len() {
        return Err(FixError::PatchValidation(format!(
            "Replacement range {}-{} is out of bounds for file with {} lines.",
            start_line,
            end_line,
            lines.len()
        )));
    }

    let replacement_lines: Vec<String> = if replacement.is_empty() {
        vec![String::new()]
    } else {
        replacement.lines().map(ToString::to_string).collect()
    };

    lines.splice((start_line - 1)..end_line, replacement_lines);

    let mut updated = lines.join("\n");
    if content.ends_with('\n') {
        updated.push('\n');
    }
    Ok(updated)
}

#[cfg(test)]
mod tests {
    use super::apply_replacement;

    #[test]
    fn replaces_requested_range() {
        let input = "a\nb\nc\nd\n";
        let updated = apply_replacement(input, 2, 3, "x\ny").unwrap();
        assert_eq!(updated, "a\nx\ny\nd\n");
    }

    #[test]
    fn preserves_trailing_newline() {
        let input = "a\nb\nc";
        let updated = apply_replacement(input, 2, 2, "x").unwrap();
        assert_eq!(updated, "a\nx\nc");
    }

    #[test]
    fn adds_trailing_newline_when_original_has_it() {
        let input = "a\nb\nc\n";
        let updated = apply_replacement(input, 2, 2, "x").unwrap();
        assert_eq!(updated, "a\nx\nc\n");
    }
}
