use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestReviewResult {
    pub project_id: i64,
    pub pull_request_id: i64,
    pub review_run_id: i64,
    pub issue_count: usize,
    pub ingested_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunFixResult {
    pub issue_id: i64,
    pub pull_request_id: i64,
    pub fix_run_id: i64,
    pub issue_status: String,
    pub fix_status: String,
    pub processed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyFixResult {
    pub issue_id: i64,
    pub fix_run_id: i64,
    pub verification_id: i64,
    pub verification_status: String,
    pub issue_status: String,
    pub verified_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunWorkflowResult {
    pub review: IngestReviewResult,
    pub fix: Option<RunFixResult>,
    pub workflow_status: String,
    pub summary: String,
    pub completed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub id: i64,
    pub project_key: String,
    pub project_name: String,
    pub repo_url: Option<String>,
    pub repo_dir: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequestSummary {
    pub id: i64,
    pub project_id: i64,
    pub platform: String,
    pub pr_number: i64,
    pub pr_url: String,
    pub latest_commit_sha: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct IssueSummary {
    pub id: i64,
    pub project_key: String,
    pub project_name: String,
    pub platform: String,
    pub pr_number: i64,
    pub pull_request_id: i64,
    pub review_run_id: i64,
    pub severity: String,
    pub file_path: String,
    pub start_line: i64,
    pub end_line: i64,
    pub title: String,
    pub description: String,
    pub suggestion: String,
    pub suggestion_code: Option<String>,
    pub original_code: Option<String>,
    pub status: String,
    pub confidence: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub fix_replacement_preview: Option<String>,
    pub fix_commit_sha: Option<String>,
    pub pr_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrStats {
    pub project_key: String,
    pub platform: String,
    pub pr_number: i64,
    pub total_issues: i64,
    pub open_issues: i64,
    pub fixed_pending_verification: i64,
    pub resolved_issues: i64,
    pub needs_human_issues: i64,
    pub invalid_issues: i64,
    pub reopened_issues: i64,
    pub total_fix_runs: i64,
    pub total_verifications: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowRoundResult {
    pub round_number: i32,
    pub review_run_id: i64,
    pub issue_id: Option<i64>,
    pub fix_run_id: Option<i64>,
    pub verification_id: Option<i64>,
    pub verification_status: Option<String>,
    pub status: String,
    pub stop_reason: Option<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunUntilStableResult {
    pub workflow_run_id: i64,
    pub project_key: String,
    pub platform: String,
    pub pr_number: i64,
    pub pr_url: String,
    pub status: String,
    pub stop_reason: String,
    pub max_rounds: i32,
    pub completed_rounds: i32,
    pub rounds: Vec<WorkflowRoundResult>,
    pub summary: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowRunSummary {
    pub id: i64,
    pub project_key: String,
    pub project_name: String,
    pub platform: String,
    pub pr_number: i64,
    pub pr_url: String,
    pub status: String,
    pub stop_reason: Option<String>,
    pub max_rounds: i32,
    pub summary: Option<String>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowRoundSummary {
    pub id: i64,
    pub workflow_run_id: i64,
    pub round_number: i32,
    pub review_run_id: Option<i64>,
    pub issue_id: Option<i64>,
    pub fix_run_id: Option<i64>,
    pub verification_id: Option<i64>,
    pub status: String,
    pub stop_reason: Option<String>,
    pub summary: Option<String>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowRunDetail {
    pub workflow: WorkflowRunSummary,
    pub rounds: Vec<WorkflowRoundSummary>,
}
