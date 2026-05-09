//! Patch validator: validates and corrects LLM-generated patches.
//!
//! `PatchValidator` checks patch correctness against the original issue and file,
//! and applies safe corrections when possible.

use std::path::PathBuf;

use crate::error::{FixError, Result};
use crate::models::FixPatch;
use reviewagent::llm::Issue;

/// Validation result for a single patch.
#[derive(Debug, Clone)]
pub struct PatchValidation {
    pub is_valid: bool,
    pub corrections: Vec<String>,
    pub reason: Option<String>,
}

/// Validates and optionally corrects LLM-generated patches.
pub struct PatchValidator {
    max_replacement_lines: usize,
}

impl PatchValidator {
    pub fn new(max_replacement_lines: usize) -> Self {
        Self {
            max_replacement_lines,
        }
    }

    /// Validate a patch against the original issue and return a validation result.
    pub fn validate(
        &self,
        issue: &Issue,
        patch: &FixPatch,
        file_content: &str,
    ) -> PatchValidation {
        let mut corrections = Vec::new();

        // 1. File path validation
        if patch.file != issue.file {
            return PatchValidation {
                is_valid: false,
                corrections,
                reason: Some(format!(
                    "Patch file '{}' does not match issue file '{}'",
                    patch.file, issue.file
                )),
            };
        }

        // 2. Line range validation
        if patch.start_line == 0 || patch.end_line == 0 {
            return PatchValidation {
                is_valid: false,
                corrections,
                reason: Some("Patch line range contains 0, which is invalid".to_string()),
            };
        }

        if patch.start_line > patch.end_line {
            return PatchValidation {
                is_valid: false,
                corrections,
                reason: Some(format!(
                    "Patch start_line ({}) > end_line ({})",
                    patch.start_line, patch.end_line
                )),
            };
        }

        // 3. File content bounds validation
        let total_lines = file_content.lines().count();
        if patch.end_line > total_lines {
            return PatchValidation {
                is_valid: false,
                corrections,
                reason: Some(format!(
                    "Patch end_line ({}) exceeds file line count ({})",
                    patch.end_line, total_lines
                )),
            };
        }

        // 4. Replacement size validation
        let replacement_lines = patch.replacement.lines().count();
        if replacement_lines > self.max_replacement_lines {
            return PatchValidation {
                is_valid: false,
                corrections,
                reason: Some(format!(
                    "Replacement too large: {} lines exceeds limit {}",
                    replacement_lines, self.max_replacement_lines
                )),
            };
        }

        // 5. Issue line range alignment check (warning only)
        let issue_end = issue.end_line.unwrap_or(issue.line);
        if patch.start_line > issue_end || patch.end_line < issue.line {
            corrections.push(format!(
                "Warning: Patch range {}-{} does not overlap with issue range {}-{}",
                patch.start_line, patch.end_line, issue.line, issue_end
            ));
        }

        PatchValidation {
            is_valid: true,
            corrections,
            reason: None,
        }
    }

    /// Validate and return Result, failing on invalid patches.
    pub fn validate_strict(
        &self,
        issue: &Issue,
        patch: &FixPatch,
        file_content: &str,
    ) -> Result<Vec<String>> {
        let validation = self.validate(issue, patch, file_content);
        if validation.is_valid {
            Ok(validation.corrections)
        } else {
            Err(FixError::PatchValidation(
                validation.reason.unwrap_or_else(|| "Invalid patch".to_string()),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{FixPatch, FixPatchOutcome};
    use reviewagent::llm::{Issue, Severity};

    fn make_issue(file: &str, line: usize, end_line: Option<usize>) -> Issue {
        Issue {
            severity: Severity::Warning,
            file: file.to_string(),
            line,
            end_line,
            title: "Test issue".to_string(),
            description: "Test description".to_string(),
            suggestion: "Fix it".to_string(),
            suggestion_code: None,
            confidence: Some(80),
        }
    }

    fn make_patch(file: &str, start: usize, end: usize, replacement: &str) -> FixPatch {
        FixPatch {
            summary: "Test patch".to_string(),
            outcome: FixPatchOutcome::Apply,
            file: file.to_string(),
            start_line: start,
            end_line: end,
            replacement: replacement.to_string(),
            rationale: "Test rationale".to_string(),
            verification_steps: vec![],
        }
    }

    #[test]
    fn validates_correct_patch() {
        let validator = PatchValidator::new(120);
        let issue = make_issue("src/test.rs", 2, Some(3));
        let patch = make_patch("src/test.rs", 2, 3, "x\ny");
        let content = "a\nb\nc\nd\n";

        let result = validator.validate(&issue, &patch, content);
        assert!(result.is_valid);
        assert!(result.reason.is_none());
    }

    #[test]
    fn rejects_wrong_file() {
        let validator = PatchValidator::new(120);
        let issue = make_issue("src/test.rs", 2, None);
        let patch = make_patch("src/other.rs", 2, 2, "x");
        let content = "a\nb\nc\n";

        let result = validator.validate(&issue, &patch, content);
        assert!(!result.is_valid);
        assert!(result.reason.unwrap().contains("does not match issue file"));
    }

    #[test]
    fn rejects_zero_line() {
        let validator = PatchValidator::new(120);
        let issue = make_issue("src/test.rs", 2, None);
        let patch = make_patch("src/test.rs", 0, 2, "x");
        let content = "a\nb\nc\n";

        let result = validator.validate(&issue, &patch, content);
        assert!(!result.is_valid);
    }

    #[test]
    fn rejects_out_of_bounds() {
        let validator = PatchValidator::new(120);
        let issue = make_issue("src/test.rs", 2, None);
        let patch = make_patch("src/test.rs", 2, 10, "x");
        let content = "a\nb\nc\n";

        let result = validator.validate(&issue, &patch, content);
        assert!(!result.is_valid);
        assert!(result.reason.unwrap().contains("exceeds file line count"));
    }

    #[test]
    fn rejects_too_large_replacement() {
        let validator = PatchValidator::new(2);
        let issue = make_issue("src/test.rs", 2, None);
        let patch = make_patch("src/test.rs", 2, 2, "a\nb\nc");
        let content = "a\nb\nc\n";

        let result = validator.validate(&issue, &patch, content);
        assert!(!result.is_valid);
        assert!(result.reason.unwrap().contains("exceeds limit"));
    }
}
