use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use reviewagent::llm::Issue;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixTask {
    pub issue_id: Option<i64>,
    pub issue_index: usize,
    pub issue: Issue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixResult {
    pub issue_id: Option<i64>,
    pub issue_index: usize,
    pub issue_title: String,
    pub status: FixExecutionStatus,
    pub dry_run: bool,
    pub file: String,
    pub applied_range: LineRange,
    pub summary: String,
    pub rationale: String,
    pub verification_steps: Vec<String>,
    pub replacement_preview: String,
    pub confirmation: Option<FixConfirmation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixConfirmation {
    pub confirmed: bool,
    pub confidence: u8,
    pub findings: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixExecutionStatus {
    Applied,
    Suggested,
    NeedsHuman,
    InvalidCandidate,
    NotReproducible,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FixPatch {
    pub summary: String,
    pub outcome: FixPatchOutcome,
    pub file: String,
    pub start_line: usize,
    pub end_line: usize,
    pub replacement: String,
    pub rationale: String,
    #[serde(default)]
    pub verification_steps: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FixPatchOutcome {
    Apply,
    NeedsHuman,
    InvalidCandidate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineRange {
    pub start: usize,
    pub end: usize,
}
