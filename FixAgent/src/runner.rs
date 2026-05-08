use crate::config::{FixAgentConfig, load_reviewagent_config};
use crate::error::{FixError, Result};
use crate::models::{
    FixExecutionStatus, FixPatch, FixPatchOutcome, FixResult, FixTask, LineRange,
};
use reviewagent::llm::{Issue, LlmClient, ReviewResponse};
use std::path::{Path, PathBuf};

const FIX_SYSTEM_PROMPT: &str = r#"
You are a minimal code fix agent.

Your task is to fix exactly one reported issue with the smallest safe code change.

Rules:
- Only modify the file identified by the issue.
- Only modify the code range necessary to resolve the issue.
- Do not refactor unrelated code.
- Preserve formatting style already present in the file.
- If the issue is not safe to fix automatically, return outcome=needs_human.
- If the issue appears invalid, return outcome=invalid_candidate.
- Return only structured data.
"#;

pub struct FixRunner {
    repo_dir: PathBuf,
    config: FixAgentConfig,
    llm: LlmClient,
}

impl FixRunner {
    pub async fn new(repo_dir: PathBuf) -> Result<Self> {
        let repo_dir = std::fs::canonicalize(&repo_dir).map_err(FixError::Io)?;
        let config = FixAgentConfig::load_or_default(&repo_dir)
            .map_err(|e| FixError::Config(e.to_string()))?;
        let review_config =
            load_reviewagent_config(&repo_dir).map_err(|e| FixError::Config(e.to_string()))?;
        let llm = LlmClient::from_config(&review_config)
            .map_err(|e| FixError::Config(format!("Failed to initialize LLM: {}", e)))?;

        Ok(Self {
            repo_dir,
            config,
            llm,
        })
    }

    pub async fn run(
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

    pub async fn run_task(&self, task: FixTask, dry_run: bool) -> Result<FixResult> {
        let target_file = self.resolve_target_file(&task.issue.file)?;
        let original_content = tokio::fs::read_to_string(&target_file).await?;
        let prompt = self.build_fix_prompt(&task.issue, &original_content);
        let patch = self
            .llm
            .extract::<FixPatch>(FIX_SYSTEM_PROMPT, &prompt)
            .await
            .map_err(|e| FixError::Llm(e.to_string()))?;

        self.validate_patch(&task.issue, &patch)?;

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

    async fn load_review(&self, review_file: &Path) -> Result<ReviewResponse> {
        let content = tokio::fs::read_to_string(review_file).await?;
        Ok(serde_json::from_str(&content)?)
    }

    fn select_issue<'a>(&self, review: &'a ReviewResponse, issue_index: usize) -> Result<&'a Issue> {
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

    fn resolve_target_file(&self, relative_path: &str) -> Result<PathBuf> {
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

    fn build_fix_prompt(&self, issue: &Issue, file_content: &str) -> String {
        let total_lines = file_content.lines().count();
        let issue_end = issue.end_line.unwrap_or(issue.line);
        let context = self.config.fix.context_lines;
        let start = issue.line.saturating_sub(context).max(1);
        let end = (issue_end + context).min(total_lines.max(1));
        let snippet = slice_lines(file_content, start, end);

        format!(
            r#"Issue:
- Severity: {severity:?}
- File: {file}
- Line: {line}
- End line: {end_line}
- Title: {title}
- Description: {description}
- Suggested fix: {suggestion}
- Confidence: {confidence:?}

Constraints:
- Keep the fix minimal and localized.
- Replacement must cover only the necessary line range.
- If the safest action is to avoid automatic changes, return needs_human.

File excerpt ({start}-{end} of {total_lines}):
```text
{snippet}
```
"#,
            severity = issue.severity,
            file = issue.file,
            line = issue.line,
            end_line = issue.end_line.unwrap_or(issue.line),
            title = issue.title,
            description = issue.description,
            suggestion = issue.suggestion,
            confidence = issue.confidence,
            start = start,
            end = end,
            total_lines = total_lines,
            snippet = snippet,
        )
    }

    fn validate_patch(&self, issue: &Issue, patch: &FixPatch) -> Result<()> {
        if patch.file != issue.file {
            return Err(FixError::PatchValidation(format!(
                "Patch file '{}' does not match issue file '{}'.",
                patch.file, issue.file
            )));
        }

        if patch.start_line == 0 || patch.end_line == 0 || patch.start_line > patch.end_line {
            return Err(FixError::PatchValidation(
                "Patch line range is invalid.".to_string(),
            ));
        }

        let replacement_lines = patch.replacement.lines().count();
        if replacement_lines > self.config.fix.max_replacement_lines {
            return Err(FixError::PatchValidation(format!(
                "Replacement too large: {} lines exceeds configured limit {}.",
                replacement_lines, self.config.fix.max_replacement_lines
            )));
        }

        Ok(())
    }
}

fn slice_lines(content: &str, start: usize, end: usize) -> String {
    content
        .lines()
        .enumerate()
        .filter_map(|(idx, line)| {
            let line_no = idx + 1;
            if line_no >= start && line_no <= end {
                Some(format!("{:>4}: {}", line_no, line))
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn apply_replacement(content: &str, start_line: usize, end_line: usize, replacement: &str) -> Result<String> {
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
}
