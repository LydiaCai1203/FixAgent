use crate::db;
use crate::error::{OrchestratorError, Result};
use crate::models::{
    IngestReviewResult, IssueSummary, PrStats, ProjectSummary, PullRequestSummary, RunFixResult,
    RunUntilStableResult, RunWorkflowResult, VerifyFixResult, WorkflowRoundResult,
    WorkflowRoundSummary, WorkflowRunDetail, WorkflowRunSummary,
};
use chrono::Utc;
use fixagent::models::{FixExecutionStatus, FixTask};
use fixagent::runner::FixRunner;
use reviewagent::orchestrator::{ReviewInput, ReviewOrchestrator};
use reviewagent::platform;
use reviewagent::llm::ReviewResponse;
use sqlx::PgPool;
use std::path::PathBuf;

#[derive(Clone)]
pub struct OrchestratorService {
    pool: PgPool,
}

impl OrchestratorService {
    pub async fn new_from_env() -> Result<Self> {
        let database_url = std::env::var("DATABASE_URL")
            .map_err(|_| OrchestratorError::Config("DATABASE_URL is required".to_string()))?;
        let pool = db::connect(&database_url).await?;
        let service = Self { pool };
        service.recover_interrupted_runs().await?;
        Ok(service)
    }

    pub async fn run_review(
        &self,
        repo_dir: PathBuf,
        project_key: String,
        project_name: String,
        pr_url: String,
    ) -> Result<IngestReviewResult> {
        let repo_dir = std::fs::canonicalize(repo_dir)?;
        let config = load_reviewagent_config(&repo_dir)?;
        let github_token = config.publish.github_token();
        let gitlab_token = config.publish.gitlab_token();
        let gitee_token = config.publish.gitee_token();
        let gitea_token = config.publish.gitea_token();

        let detected = platform::detect_platform(
            &pr_url,
            github_token.as_deref(),
            gitlab_token.as_deref(),
            gitee_token.as_deref(),
            gitea_token.as_deref(),
        )
        .map_err(|e| OrchestratorError::Config(e.to_string()))?;

        let review_orchestrator = ReviewOrchestrator::new(config, &repo_dir);
        let prepared = review_orchestrator
            .prepare(ReviewInput::Url(pr_url.clone()), Some(detected.as_ref()))
            .await
            .map_err(|e| OrchestratorError::Config(e.to_string()))?;

        let result = review_orchestrator
            .review(&prepared.diff_analysis)
            .await
            .map_err(|e| OrchestratorError::Config(e.to_string()))?;
        let final_review = result.into_final_review();

        let (platform_name, pr_number) = detect_platform_name_and_pr_number(&pr_url)
            .ok_or_else(|| OrchestratorError::Config("Unable to parse platform or PR number from URL".to_string()))?;

        self.ingest_review_response(
            project_key,
            project_name,
            platform_name,
            pr_number,
            pr_url,
            prepared.diff_analysis.commit_sha,
            final_review,
        )
        .await
    }

    pub async fn list_projects(&self) -> Result<Vec<ProjectSummary>> {
        let rows = sqlx::query_as::<_, (i64, String, String, chrono::DateTime<Utc>, chrono::DateTime<Utc>)>(
            r#"
            SELECT id, project_key, project_name, created_at, updated_at
            FROM projects
            ORDER BY updated_at DESC, id DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(id, project_key, project_name, created_at, updated_at)| ProjectSummary {
                id,
                project_key,
                project_name,
                created_at,
                updated_at,
            })
            .collect())
    }

    pub async fn create_project(&self, project_name: String) -> Result<ProjectSummary> {
        let project_name = project_name.trim().to_string();
        if project_name.is_empty() {
            return Err(OrchestratorError::Config("project_name is required".to_string()));
        }

        let project_key = build_project_key(&project_name);
        let row = sqlx::query_as::<_, (i64, String, String, chrono::DateTime<Utc>, chrono::DateTime<Utc>)>(
            r#"
            INSERT INTO projects (project_key, project_name)
            VALUES ($1, $2)
            ON CONFLICT (project_key)
            DO UPDATE SET project_name = EXCLUDED.project_name, updated_at = NOW()
            RETURNING id, project_key, project_name, created_at, updated_at
            "#,
        )
        .bind(&project_key)
        .bind(&project_name)
        .fetch_one(&self.pool)
        .await?;

        Ok(ProjectSummary {
            id: row.0,
            project_key: row.1,
            project_name: row.2,
            created_at: row.3,
            updated_at: row.4,
        })
    }

    pub async fn delete_project(&self, project_key: String) -> Result<()> {
        let project_key = project_key.trim().to_string();
        if project_key.is_empty() {
            return Err(OrchestratorError::Config("project_key is required".to_string()));
        }

        let mut tx = self.pool.begin().await?;

        let project_id = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT id
            FROM projects
            WHERE project_key = $1
            "#,
        )
        .bind(&project_key)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| OrchestratorError::Config(format!("Project not found: {}", project_key)))?;

        sqlx::query(
            r#"
            DELETE FROM verifications
            WHERE issue_id IN (
                SELECT i.id
                FROM issues i
                JOIN pull_requests pr ON pr.id = i.pull_request_id
                WHERE pr.project_id = $1
            )
            "#,
        )
        .bind(project_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            DELETE FROM fix_runs
            WHERE issue_id IN (
                SELECT i.id
                FROM issues i
                JOIN pull_requests pr ON pr.id = i.pull_request_id
                WHERE pr.project_id = $1
            )
            "#,
        )
        .bind(project_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            DELETE FROM issues
            WHERE pull_request_id IN (
                SELECT id FROM pull_requests WHERE project_id = $1
            )
            "#,
        )
        .bind(project_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            DELETE FROM review_runs
            WHERE pull_request_id IN (
                SELECT id FROM pull_requests WHERE project_id = $1
            )
            "#,
        )
        .bind(project_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            DELETE FROM pull_requests
            WHERE project_id = $1
            "#,
        )
        .bind(project_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            DELETE FROM workflow_rounds
            WHERE workflow_run_id IN (
                SELECT id FROM workflow_runs WHERE project_key = $1
            )
            "#,
        )
        .bind(&project_key)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            DELETE FROM workflow_runs
            WHERE project_key = $1
            "#,
        )
        .bind(&project_key)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            DELETE FROM projects
            WHERE id = $1
            "#,
        )
        .bind(project_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn list_prs(&self, project_key: String) -> Result<Vec<PullRequestSummary>> {
        let rows = sqlx::query_as::<_, (i64, i64, String, i64, String, Option<String>, chrono::DateTime<Utc>, chrono::DateTime<Utc>)>(
            r#"
            SELECT pr.id, pr.project_id, pr.platform, pr.pr_number, pr.pr_url, pr.latest_commit_sha, pr.created_at, pr.updated_at
            FROM pull_requests pr
            JOIN projects p ON p.id = pr.project_id
            WHERE p.project_key = $1
            ORDER BY pr.updated_at DESC, pr.id DESC
            "#,
        )
        .bind(project_key)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(id, project_id, platform, pr_number, pr_url, latest_commit_sha, created_at, updated_at)| PullRequestSummary {
                id,
                project_id,
                platform,
                pr_number,
                pr_url,
                latest_commit_sha,
                created_at,
                updated_at,
            })
            .collect())
    }

    pub async fn create_pr(&self, project_key: String, pr_url: String) -> Result<PullRequestSummary> {
        let project_key = project_key.trim().to_string();
        let pr_url = pr_url.trim().to_string();

        if project_key.is_empty() {
            return Err(OrchestratorError::Config("project_key is required".to_string()));
        }
        if pr_url.is_empty() {
            return Err(OrchestratorError::Config("pr_url is required".to_string()));
        }

        let (platform, pr_number) = detect_platform_name_and_pr_number(&pr_url).ok_or_else(|| {
            OrchestratorError::Config("Unable to parse platform or PR number from URL".to_string())
        })?;

        let project_id = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT id
            FROM projects
            WHERE project_key = $1
            "#,
        )
        .bind(&project_key)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| OrchestratorError::Config(format!("Project not found: {}", project_key)))?;

        let row = sqlx::query_as::<_, (i64, i64, String, i64, String, Option<String>, chrono::DateTime<Utc>, chrono::DateTime<Utc>)>(
            r#"
            INSERT INTO pull_requests (project_id, platform, pr_number, pr_url, latest_commit_sha)
            VALUES ($1, $2, $3, $4, NULL)
            ON CONFLICT (project_id, platform, pr_number)
            DO UPDATE SET pr_url = EXCLUDED.pr_url, updated_at = NOW()
            RETURNING id, project_id, platform, pr_number, pr_url, latest_commit_sha, created_at, updated_at
            "#,
        )
        .bind(project_id)
        .bind(&platform)
        .bind(pr_number)
        .bind(&pr_url)
        .fetch_one(&self.pool)
        .await?;

        Ok(PullRequestSummary {
            id: row.0,
            project_id: row.1,
            platform: row.2,
            pr_number: row.3,
            pr_url: row.4,
            latest_commit_sha: row.5,
            created_at: row.6,
            updated_at: row.7,
        })
    }

    pub async fn list_issues(
        &self,
        project_key: Option<String>,
        platform: Option<String>,
        pr_number: Option<i64>,
        status: Option<String>,
    ) -> Result<Vec<IssueSummary>> {
        let rows = sqlx::query_as::<_, IssueSummary>(
            r#"
            SELECT
                i.id,
                p.project_key,
                p.project_name,
                pr.platform,
                pr.pr_number,
                i.pull_request_id,
                i.review_run_id,
                i.severity,
                i.file_path,
                i.start_line,
                i.end_line,
                i.title,
                i.description,
                i.suggestion,
                i.suggestion_code,
                i.original_code,
                i.status,
                i.confidence,
                i.created_at,
                i.updated_at
            FROM issues i
            JOIN pull_requests pr ON pr.id = i.pull_request_id
            JOIN projects p ON p.id = pr.project_id
            WHERE ($1::text IS NULL OR p.project_key = $1)
              AND ($2::text IS NULL OR pr.platform = $2)
              AND ($3::bigint IS NULL OR pr.pr_number = $3)
              AND ($4::text IS NULL OR i.status = $4)
            ORDER BY i.updated_at DESC, i.id DESC
            "#,
        )
        .bind(project_key)
        .bind(platform)
        .bind(pr_number)
        .bind(status)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    pub async fn update_issue_status(
        &self,
        issue_id: i64,
        new_status: String,
    ) -> Result<IssueSummary> {
        let valid_statuses = ["open", "reopened", "resolved", "needs_human", "invalid", "claimed"];
        if !valid_statuses.contains(&new_status.as_str()) {
            return Err(OrchestratorError::Config(format!(
                "Invalid status '{}'. Valid statuses are: {:?}",
                new_status, valid_statuses
            )));
        }

        let row = sqlx::query_as::<_, IssueSummary>(
            r#"
            UPDATE issues i
            SET status = $1,
                updated_at = NOW()
            FROM pull_requests pr, projects p
            WHERE i.id = $2
              AND pr.id = i.pull_request_id
              AND p.id = pr.project_id
            RETURNING
                i.id,
                p.project_key,
                p.project_name,
                pr.platform,
                pr.pr_number,
                i.pull_request_id,
                i.review_run_id,
                i.severity,
                i.file_path,
                i.start_line,
                i.end_line,
                i.title,
                i.description,
                i.suggestion,
                i.suggestion_code,
                i.original_code,
                i.status,
                i.confidence,
                i.created_at,
                i.updated_at
            "#,
        )
        .bind(&new_status)
        .bind(issue_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| OrchestratorError::Config(format!("Issue not found: {}", issue_id)))?;

        Ok(row)
    }

    pub async fn list_workflows(
        &self,
        project_key: Option<String>,
    ) -> Result<Vec<WorkflowRunSummary>> {
        let rows = if let Some(project_key) = project_key {
            sqlx::query_as::<_, (i64, String, String, String, i64, String, String, Option<String>, i32, Option<String>, chrono::DateTime<Utc>, Option<chrono::DateTime<Utc>>)>(
                r#"
                SELECT id, project_key, project_name, platform, pr_number, pr_url, status, stop_reason, max_rounds, summary, started_at, completed_at
                FROM workflow_runs
                WHERE project_key = $1
                ORDER BY started_at DESC, id DESC
                "#,
            )
            .bind(project_key)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, (i64, String, String, String, i64, String, String, Option<String>, i32, Option<String>, chrono::DateTime<Utc>, Option<chrono::DateTime<Utc>>)>(
                r#"
                SELECT id, project_key, project_name, platform, pr_number, pr_url, status, stop_reason, max_rounds, summary, started_at, completed_at
                FROM workflow_runs
                ORDER BY started_at DESC, id DESC
                "#,
            )
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows
            .into_iter()
            .map(|(id, project_key, project_name, platform, pr_number, pr_url, status, stop_reason, max_rounds, summary, started_at, completed_at)| WorkflowRunSummary {
                id,
                project_key,
                project_name,
                platform,
                pr_number,
                pr_url,
                status,
                stop_reason,
                max_rounds,
                summary,
                started_at,
                completed_at,
            })
            .collect())
    }

    pub async fn workflow_detail(&self, workflow_run_id: i64) -> Result<WorkflowRunDetail> {
        let workflow = self.workflow_run_summary(workflow_run_id).await?;
        let rounds = self.workflow_rounds(workflow_run_id).await?;
        Ok(WorkflowRunDetail { workflow, rounds })
    }

    pub async fn workflow_rounds(
        &self,
        workflow_run_id: i64,
    ) -> Result<Vec<WorkflowRoundSummary>> {
        let rows = sqlx::query_as::<_, (i64, i64, i32, Option<i64>, Option<i64>, Option<i64>, Option<i64>, String, Option<String>, Option<String>, chrono::DateTime<Utc>, Option<chrono::DateTime<Utc>>)>(
            r#"
            SELECT id, workflow_run_id, round_number, review_run_id, issue_id, fix_run_id, verification_id, status, stop_reason, summary, started_at, completed_at
            FROM workflow_rounds
            WHERE workflow_run_id = $1
            ORDER BY round_number ASC, id ASC
            "#,
        )
        .bind(workflow_run_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(id, workflow_run_id, round_number, review_run_id, issue_id, fix_run_id, verification_id, status, stop_reason, summary, started_at, completed_at)| WorkflowRoundSummary {
                id,
                workflow_run_id,
                round_number,
                review_run_id,
                issue_id,
                fix_run_id,
                verification_id,
                status,
                stop_reason,
                summary,
                started_at,
                completed_at,
            })
            .collect())
    }

    pub async fn pr_stats(
        &self,
        project_key: String,
        platform: String,
        pr_number: i64,
    ) -> Result<PrStats> {
        let (total_issues, open_issues, fixed_pending_verification, resolved_issues, needs_human_issues, invalid_issues, reopened_issues, total_fix_runs, total_verifications) =
            sqlx::query_as::<_, (i64, i64, i64, i64, i64, i64, i64, i64, i64)>(
                r#"
                SELECT
                    COUNT(i.id) AS total_issues,
                    COUNT(*) FILTER (WHERE i.status = 'open') AS open_issues,
                    COUNT(*) FILTER (WHERE i.status = 'fixed_pending_verification') AS fixed_pending_verification,
                    COUNT(*) FILTER (WHERE i.status = 'resolved') AS resolved_issues,
                    COUNT(*) FILTER (WHERE i.status = 'needs_human') AS needs_human_issues,
                    COUNT(*) FILTER (WHERE i.status = 'invalid') AS invalid_issues,
                    COUNT(*) FILTER (WHERE i.status = 'reopened') AS reopened_issues,
                    COUNT(DISTINCT fr.id) AS total_fix_runs,
                    COUNT(DISTINCT v.id) AS total_verifications
                FROM pull_requests pr
                JOIN projects p ON p.id = pr.project_id
                LEFT JOIN issues i ON i.pull_request_id = pr.id
                LEFT JOIN fix_runs fr ON fr.issue_id = i.id
                LEFT JOIN verifications v ON v.issue_id = i.id
                WHERE p.project_key = $1
                  AND pr.platform = $2
                  AND pr.pr_number = $3
                GROUP BY pr.id
                "#,
            )
            .bind(&project_key)
            .bind(&platform)
            .bind(pr_number)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| {
                OrchestratorError::Config(format!(
                    "PR/MR not found for project_key={}, platform={}, pr_number={}",
                    project_key, platform, pr_number
                ))
            })?;

        Ok(PrStats {
            project_key,
            platform,
            pr_number,
            total_issues,
            open_issues,
            fixed_pending_verification,
            resolved_issues,
            needs_human_issues,
            invalid_issues,
            reopened_issues,
            total_fix_runs,
            total_verifications,
        })
    }

    pub async fn run_workflow(
        &self,
        repo_dir: PathBuf,
        project_key: String,
        project_name: String,
        pr_url: String,
        claimed_by: String,
        dry_run: bool,
    ) -> Result<RunWorkflowResult> {
        let review = self
            .run_review(
                repo_dir.clone(),
                project_key.clone(),
                project_name,
                pr_url.clone(),
            )
            .await?;

        let (platform, pr_number) = detect_platform_name_and_pr_number(&pr_url)
            .ok_or_else(|| OrchestratorError::Config("Unable to parse platform or PR number from URL".to_string()))?;

        let fix = match self
            .run_fix_for_next_issue(
                repo_dir,
                project_key,
                platform,
                pr_number,
                claimed_by,
                dry_run,
            )
            .await
        {
            Ok(result) => Some(result),
            Err(OrchestratorError::Config(msg)) if msg.contains("No open issue found") => None,
            Err(e) => return Err(e),
        };

        let (workflow_status, summary) = if let Some(fix_result) = &fix {
            (
                "review_and_fix_completed".to_string(),
                format!(
                    "Review ingested {} issues and processed issue {} into status {}.",
                    review.issue_count, fix_result.issue_id, fix_result.issue_status
                ),
            )
        } else {
            (
                "review_completed".to_string(),
                format!(
                    "Review ingested {} issues and found no open issue to fix in this workflow run.",
                    review.issue_count
                ),
            )
        };

        Ok(RunWorkflowResult {
            review,
            fix,
            workflow_status,
            summary,
            completed_at: Utc::now(),
        })
    }

    pub async fn run_until_stable(
        &self,
        repo_dir: PathBuf,
        project_key: String,
        project_name: String,
        pr_url: String,
        claimed_by: String,
        max_rounds: i32,
        dry_run: bool,
    ) -> Result<RunUntilStableResult> {
        let workflow_run_id = self
            .start_workflow(
                project_key.clone(),
                project_name.clone(),
                pr_url.clone(),
                max_rounds,
            )
            .await?;

        self.execute_workflow_run(
            workflow_run_id,
            repo_dir,
            project_key,
            project_name,
            pr_url,
            claimed_by,
            max_rounds,
            dry_run,
        )
        .await
    }

    pub async fn start_review_run(
        &self,
        project_key: String,
        project_name: String,
        pr_url: String,
    ) -> Result<i64> {
        let (platform, pr_number) = detect_platform_name_and_pr_number(&pr_url).ok_or_else(|| {
            OrchestratorError::Config("Unable to parse platform or PR number from URL".to_string())
        })?;

        let workflow_run_id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO workflow_runs (
                project_key,
                project_name,
                platform,
                pr_number,
                pr_url,
                status,
                max_rounds,
                summary
            ) VALUES ($1, $2, $3, $4, $5, 'running', 1, $6)
            RETURNING id
            "#,
        )
        .bind(&project_key)
        .bind(&project_name)
        .bind(&platform)
        .bind(pr_number)
        .bind(&pr_url)
        .bind("Review task accepted and waiting to start.")
        .fetch_one(&self.pool)
        .await?;

        Ok(workflow_run_id)
    }

    pub async fn execute_review_run(
        &self,
        workflow_run_id: i64,
        repo_dir: PathBuf,
        project_key: String,
        project_name: String,
        pr_url: String,
    ) -> Result<IngestReviewResult> {
        sqlx::query(
            r#"
            INSERT INTO workflow_rounds (workflow_run_id, round_number, status, summary)
            VALUES ($1, 1, 'running', $2)
            ON CONFLICT (workflow_run_id, round_number)
            DO UPDATE SET status = EXCLUDED.status, summary = EXCLUDED.summary, completed_at = NULL
            "#,
        )
        .bind(workflow_run_id)
        .bind("ReviewAgent is analyzing the PR diff.")
        .execute(&self.pool)
        .await?;

        self.update_workflow_progress(
            workflow_run_id,
            Some("ReviewAgent is analyzing the PR diff."),
            Some(1),
            "ReviewAgent is analyzing the PR diff.",
        )
        .await?;

        let review = match self.run_review(repo_dir, project_key, project_name, pr_url).await {
            Ok(review) => review,
            Err(error) => {
                sqlx::query(
                    r#"
                    UPDATE workflow_rounds
                    SET status = 'failed',
                        stop_reason = 'execution_failed',
                        summary = $2,
                        completed_at = NOW()
                    WHERE workflow_run_id = $1 AND round_number = 1
                    "#,
                )
                .bind(workflow_run_id)
                .bind(error.to_string())
                .execute(&self.pool)
                .await?;

                return Err(error);
            }
        };

        sqlx::query(
            r#"
            UPDATE workflow_rounds
            SET review_run_id = $3,
                status = 'completed',
                stop_reason = 'review_completed',
                summary = $4,
                completed_at = NOW()
            WHERE workflow_run_id = $1 AND round_number = $2
            "#,
        )
        .bind(workflow_run_id)
        .bind(1)
        .bind(review.review_run_id)
        .bind(format!(
            "ReviewAgent completed review_run_id={} and refreshed the issue pool.",
            review.review_run_id
        ))
        .execute(&self.pool)
        .await?;

        self.finish_workflow_run(
            workflow_run_id,
            "completed",
            "review_completed",
            Some(&format!(
                "ReviewAgent completed review_run_id={} and refreshed the issue pool.",
                review.review_run_id
            )),
        )
        .await?;

        Ok(review)
    }

    pub async fn start_workflow(
        &self,
        project_key: String,
        project_name: String,
        pr_url: String,
        max_rounds: i32,
    ) -> Result<i64> {
        if max_rounds <= 0 {
            return Err(OrchestratorError::Config(
                "max_rounds must be greater than 0".to_string(),
            ));
        }

        let (platform, pr_number) = detect_platform_name_and_pr_number(&pr_url).ok_or_else(|| {
            OrchestratorError::Config("Unable to parse platform or PR number from URL".to_string())
        })?;

        let workflow_run_id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO workflow_runs (
                project_key,
                project_name,
                platform,
                pr_number,
                pr_url,
                status,
                max_rounds,
                summary
            ) VALUES ($1, $2, $3, $4, $5, 'running', $6, $7)
            RETURNING id
            "#,
        )
        .bind(&project_key)
        .bind(&project_name)
        .bind(&platform)
        .bind(pr_number)
        .bind(&pr_url)
        .bind(max_rounds)
        .bind("Workflow accepted and waiting to start.")
        .fetch_one(&self.pool)
        .await?;

        Ok(workflow_run_id)
    }

    pub async fn execute_workflow_run(
        &self,
        workflow_run_id: i64,
        repo_dir: PathBuf,
        project_key: String,
        project_name: String,
        pr_url: String,
        claimed_by: String,
        max_rounds: i32,
        dry_run: bool,
    ) -> Result<RunUntilStableResult> {
        let started_at = self.workflow_run_summary(workflow_run_id).await?.started_at;
        let (platform, pr_number) = detect_platform_name_and_pr_number(&pr_url).ok_or_else(|| {
            OrchestratorError::Config("Unable to parse platform or PR number from URL".to_string())
        })?;

        let mut rounds = Vec::new();
        let mut final_status = "completed".to_string();
        let final_stop_reason: String;

        for round_number in 1..=max_rounds {
            self.update_workflow_progress(
                workflow_run_id,
                Some(&format!("Round {} is running.", round_number)),
                None,
                &format!("Round {} is being prepared.", round_number),
            )
            .await?;

            sqlx::query(
                r#"
                INSERT INTO workflow_rounds (workflow_run_id, round_number, status, summary)
                VALUES ($1, $2, 'running', $3)
                "#,
            )
            .bind(workflow_run_id)
            .bind(round_number)
            .bind(format!("Round {}: ReviewAgent is analyzing the PR diff.", round_number))
            .execute(&self.pool)
            .await?;

            self.update_workflow_progress(
                workflow_run_id,
                Some(&format!("Round {}: ReviewAgent is analyzing the PR diff.", round_number)),
                Some(round_number),
                &format!("Round {}: ReviewAgent is analyzing the PR diff.", round_number),
            )
            .await?;

            let review = self
                .run_review(
                    repo_dir.clone(),
                    project_key.clone(),
                    project_name.clone(),
                    pr_url.clone(),
                )
                .await?;

            self.update_workflow_progress(
                workflow_run_id,
                Some(&format!(
                    "Round {}: ReviewAgent completed review_run_id={} and refreshed the issue pool.",
                    round_number, review.review_run_id
                )),
                Some(round_number),
                &format!(
                    "Round {}: ReviewAgent completed review_run_id={} and refreshed the issue pool.",
                    round_number, review.review_run_id
                ),
            )
            .await?;

            let actionable_count = self
                .count_actionable_issues(&project_key, &platform, pr_number)
                .await?;

            if actionable_count == 0 {
                let summary = "No important open issues remain; only low-priority or already-processed items are left.".to_string();
                let stop_reason = "no_actionable_issues".to_string();

                sqlx::query(
                    r#"
                    UPDATE workflow_rounds
                    SET review_run_id = $3,
                        status = 'completed',
                        stop_reason = $4,
                        summary = $5,
                        completed_at = NOW()
                    WHERE workflow_run_id = $1 AND round_number = $2
                    "#,
                )
                .bind(workflow_run_id)
                .bind(round_number)
                .bind(review.review_run_id)
                .bind(&stop_reason)
                .bind(&summary)
                .execute(&self.pool)
                .await?;

                rounds.push(WorkflowRoundResult {
                    round_number,
                    review_run_id: review.review_run_id,
                    issue_id: None,
                    fix_run_id: None,
                    verification_id: None,
                    verification_status: None,
                    status: "completed".to_string(),
                    stop_reason: Some(stop_reason.clone()),
                    summary,
                });

                final_stop_reason = stop_reason;
                self.finish_workflow_run(
                    workflow_run_id,
                    &final_status,
                    &final_stop_reason,
                    Some("Workflow stopped because no important open issues remain."),
                )
                .await?;

                return Ok(RunUntilStableResult {
                    workflow_run_id,
                    project_key,
                    platform,
                    pr_number,
                    pr_url,
                    status: final_status,
                    stop_reason: final_stop_reason,
                    max_rounds,
                    completed_rounds: rounds.len() as i32,
                    rounds,
                    summary: "Workflow converged with no important open issues remaining.".to_string(),
                    started_at,
                    completed_at: Utc::now(),
                });
            }

            let fix = self
                .run_fix_for_next_issue(
                    repo_dir.clone(),
                    project_key.clone(),
                    platform.clone(),
                    pr_number,
                    claimed_by.clone(),
                    dry_run,
                )
                .await?;

            let mut verification_id = None;
            let mut verification_status = None;

            let stop_reason = if fix.issue_status == "needs_human" {
                Some("needs_human".to_string())
            } else if dry_run {
                Some("dry_run_verification_required".to_string())
            } else {
                None
            };

            let summary = if dry_run {
                let verification = self
                    .verify_fix(
                        fix.issue_id,
                        "not_verifiable_in_current_env".to_string(),
                        "Dry-run mode did not modify repository files, so automated verification could not be completed.".to_string(),
                        Some("FixAgent executed in dry-run mode only".to_string()),
                        Some("The candidate fix was not applied to the worktree".to_string()),
                        Some("The original issue may still be present until the patch is applied".to_string()),
                        Some("Re-run without --dry-run to allow automatic verification".to_string()),
                    )
                    .await?;

                verification_id = Some(verification.verification_id);
                verification_status = Some(verification.verification_status.clone());

                format!(
                    "Round {} reviewed the PR, generated a candidate fix for issue {}, and recorded verification status {} because dry-run mode prevented applying the patch.",
                    round_number, fix.issue_id, verification.verification_status
                )
            } else if fix.issue_status == "fixed_pending_verification" {
                self.update_workflow_progress(
                    workflow_run_id,
                    Some(&format!(
                        "Round {}: Verifier is rerunning ReviewAgent to validate issue {}.",
                        round_number, fix.issue_id
                    )),
                    Some(round_number),
                    &format!(
                        "Round {}: Verifier is rerunning ReviewAgent to validate issue {}.",
                        round_number, fix.issue_id
                    ),
                )
                .await?;

                let post_fix_review = self
                    .run_review(
                        repo_dir.clone(),
                        project_key.clone(),
                        project_name.clone(),
                        pr_url.clone(),
                    )
                    .await?;

                let verification = self
                    .auto_verify_issue_from_latest_review(
                        fix.issue_id,
                        post_fix_review.review_run_id,
                    )
                    .await?;

                verification_id = Some(verification.verification_id);
                verification_status = Some(verification.verification_status.clone());

                format!(
                    "Round {} reviewed the PR, processed issue {}, reran review as verification, and recorded verification status {}.",
                    round_number, fix.issue_id, verification.verification_status
                )
            } else {
                format!(
                    "Round {} reviewed the PR and processed issue {} into status {}.",
                    round_number, fix.issue_id, fix.issue_status
                )
            };

            sqlx::query(
                r#"
                UPDATE workflow_rounds
                SET review_run_id = $3,
                    issue_id = $4,
                    fix_run_id = $5,
                    verification_id = $6,
                    status = 'completed',
                    stop_reason = $7,
                    summary = $8,
                    completed_at = NOW()
                WHERE workflow_run_id = $1 AND round_number = $2
                "#,
            )
            .bind(workflow_run_id)
            .bind(round_number)
            .bind(review.review_run_id)
            .bind(fix.issue_id)
            .bind(fix.fix_run_id)
            .bind(verification_id)
            .bind(&stop_reason)
            .bind(&summary)
            .execute(&self.pool)
            .await?;

            rounds.push(WorkflowRoundResult {
                round_number,
                review_run_id: review.review_run_id,
                issue_id: Some(fix.issue_id),
                fix_run_id: Some(fix.fix_run_id),
                verification_id,
                verification_status,
                status: "completed".to_string(),
                stop_reason: stop_reason.clone(),
                summary: summary.clone(),
            });

            if stop_reason.as_deref() == Some("needs_human") {
                final_status = "needs_human".to_string();
                final_stop_reason = "needs_human".to_string();
                self.finish_workflow_run(
                    workflow_run_id,
                    &final_status,
                    &final_stop_reason,
                    Some("Workflow stopped because the latest important issue requires human intervention."),
                )
                .await?;

                return Ok(RunUntilStableResult {
                    workflow_run_id,
                    project_key,
                    platform,
                    pr_number,
                    pr_url,
                    status: final_status,
                    stop_reason: final_stop_reason,
                    max_rounds,
                    completed_rounds: rounds.len() as i32,
                    rounds,
                    summary: "Workflow stopped because at least one important issue requires human intervention.".to_string(),
                    started_at,
                    completed_at: Utc::now(),
                });
            }

            self.update_workflow_progress(
                workflow_run_id,
                Some(&summary),
                Some(round_number),
                &summary,
            )
            .await?;
        }

        final_stop_reason = "max_rounds_reached".to_string();
        self.finish_workflow_run(
            workflow_run_id,
            &final_status,
            &final_stop_reason,
            Some("Workflow stopped after reaching max_rounds."),
        )
        .await?;

        Ok(RunUntilStableResult {
            workflow_run_id,
            project_key,
            platform,
            pr_number,
            pr_url,
            status: final_status,
            stop_reason: final_stop_reason,
            max_rounds,
            completed_rounds: rounds.len() as i32,
            rounds,
            summary: "Workflow stopped after reaching the maximum configured rounds.".to_string(),
            started_at,
            completed_at: Utc::now(),
        })
    }

    pub async fn mark_workflow_failed(&self, workflow_run_id: i64, error_message: &str) -> Result<()> {
        self.finish_workflow_run(
            workflow_run_id,
            "failed",
            "execution_failed",
            Some(error_message),
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn ingest_review(
        &self,
        project_key: String,
        project_name: String,
        platform: String,
        pr_number: i64,
        pr_url: String,
        review_file: PathBuf,
        commit_sha: Option<String>,
    ) -> Result<IngestReviewResult> {
        let content = tokio::fs::read_to_string(review_file).await?;
        let review: ReviewResponse = serde_json::from_str(&content)?;

        self.ingest_review_response(
            project_key,
            project_name,
            platform,
            pr_number,
            pr_url,
            commit_sha,
            review,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn ingest_review_response(
        &self,
        project_key: String,
        project_name: String,
        platform: String,
        pr_number: i64,
        pr_url: String,
        commit_sha: Option<String>,
        review: ReviewResponse,
    ) -> Result<IngestReviewResult> {

        let mut tx = self.pool.begin().await?;

        let project_id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO projects (project_key, project_name)
            VALUES ($1, $2)
            ON CONFLICT (project_key)
            DO UPDATE SET project_name = EXCLUDED.project_name, updated_at = NOW()
            RETURNING id
            "#,
        )
        .bind(&project_key)
        .bind(&project_name)
        .fetch_one(&mut *tx)
        .await?;

        let pull_request_id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO pull_requests (project_id, platform, pr_number, pr_url, latest_commit_sha)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (project_id, platform, pr_number)
            DO UPDATE SET pr_url = EXCLUDED.pr_url,
                          latest_commit_sha = EXCLUDED.latest_commit_sha,
                          updated_at = NOW()
            RETURNING id
            "#,
        )
        .bind(project_id)
        .bind(&platform)
        .bind(pr_number)
        .bind(&pr_url)
        .bind(&commit_sha)
        .fetch_one(&mut *tx)
        .await?;

        let review_run_id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO review_runs (pull_request_id, summary, recommendation, raw_report)
            VALUES ($1, $2, $3, $4)
            RETURNING id
            "#,
        )
        .bind(pull_request_id)
        .bind(&review.summary)
        .bind(format!("{:?}", review.recommendation).to_lowercase())
        .bind(serde_json::to_value(&review)?)
        .fetch_one(&mut *tx)
        .await?;

        for issue in &review.issues {
            let fingerprint = build_issue_fingerprint(pull_request_id, issue);
            sqlx::query(
                r#"
                INSERT INTO issues (
                    pull_request_id,
                    review_run_id,
                    fingerprint,
                    severity,
                    file_path,
                    start_line,
                    end_line,
                    title,
                    description,
                    suggestion,
                    suggestion_code,
                    original_code,
                    confidence,
                    status,
                    source_bot
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, 'open', 'reviewagent')
                ON CONFLICT (pull_request_id, fingerprint)
                DO UPDATE SET
                    review_run_id = EXCLUDED.review_run_id,
                    severity = EXCLUDED.severity,
                    file_path = EXCLUDED.file_path,
                    start_line = EXCLUDED.start_line,
                    end_line = EXCLUDED.end_line,
                    title = EXCLUDED.title,
                    description = EXCLUDED.description,
                    suggestion = EXCLUDED.suggestion,
                    suggestion_code = EXCLUDED.suggestion_code,
                    original_code = EXCLUDED.original_code,
                    confidence = EXCLUDED.confidence,
                    updated_at = NOW()
                "#,
            )
            .bind(pull_request_id)
            .bind(review_run_id)
            .bind(fingerprint)
            .bind(format!("{:?}", issue.severity).to_lowercase())
            .bind(&issue.file)
            .bind(issue.line as i64)
            .bind(issue.end_line.map(|v| v as i64).unwrap_or(issue.line as i64))
            .bind(&issue.title)
            .bind(&issue.description)
            .bind(&issue.suggestion)
            .bind(&issue.suggestion_code)
            .bind(&issue.original_code)
            .bind(issue.confidence.map(|v| v as i32))
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        Ok(IngestReviewResult {
            project_id,
            pull_request_id,
            review_run_id,
            issue_count: review.issues.len(),
            ingested_at: Utc::now(),
        })
    }

    pub async fn run_fix_for_next_issue(
        &self,
        repo_dir: PathBuf,
        project_key: String,
        platform: String,
        pr_number: i64,
        claimed_by: String,
        dry_run: bool,
    ) -> Result<RunFixResult> {
        let mut tx = self.pool.begin().await?;

        let row = sqlx::query_as::<_, (i64, i64, String, String, i64, i64, String, String, String, Option<String>, Option<String>, Option<i32>)>(
            r#"
            UPDATE issues i
            SET status = 'claimed',
                claimed_by = $1,
                claimed_at = NOW(),
                updated_at = NOW()
            WHERE i.id = (
                SELECT i2.id
                FROM issues i2
                JOIN pull_requests pr ON pr.id = i2.pull_request_id
                JOIN projects p ON p.id = pr.project_id
                WHERE p.project_key = $2
                  AND pr.platform = $3
                  AND pr.pr_number = $4
                  AND i2.status IN ('open', 'reopened')
                ORDER BY
                  CASE i2.severity
                    WHEN 'critical' THEN 1
                    WHEN 'warning' THEN 2
                    ELSE 3
                  END,
                  COALESCE(i2.confidence, 0) DESC,
                  i2.updated_at DESC
                LIMIT 1
                FOR UPDATE SKIP LOCKED
            )
            RETURNING i.id, i.pull_request_id, i.severity, i.file_path, i.start_line, i.end_line, i.title, i.description, i.suggestion, i.suggestion_code, i.original_code, i.confidence
            "#,
        )
        .bind(&claimed_by)
        .bind(&project_key)
        .bind(&platform)
        .bind(pr_number)
        .fetch_optional(&mut *tx)
        .await?;

        let (issue_id, pull_request_id, severity, file_path, start_line, end_line, title, description, suggestion, suggestion_code, original_code, confidence) =
            row.ok_or_else(|| OrchestratorError::Config("No open issue found for the specified PR/MR".to_string()))?;

        tx.commit().await?;

        self.execute_fix_for_issue_data(
            repo_dir,
            issue_id,
            pull_request_id,
            severity,
            file_path,
            start_line,
            end_line,
            title,
            description,
            suggestion,
            suggestion_code,
            original_code,
            confidence,
            dry_run,
        )
        .await
    }

    pub async fn start_issue_fix_run(
        &self,
        issue_id: i64,
        repo_dir: PathBuf,
        claimed_by: String,
        dry_run: bool,
    ) -> Result<i64> {
        let (project_key, project_name, _platform, _pr_number, pr_url) = self.issue_workflow_context(issue_id).await?;
        let workflow_run_id = self
            .start_workflow(project_key.clone(), project_name, pr_url, 1)
            .await?;

        let background_service = self.clone();
        tokio::spawn(async move {
            if let Err(error) = background_service
                .execute_issue_fix_run(workflow_run_id, issue_id, repo_dir, claimed_by, dry_run)
                .await
            {
                let _ = background_service
                    .mark_workflow_failed(workflow_run_id, &error.to_string())
                    .await;
            }
        });
        Ok(workflow_run_id)
    }

    pub async fn start_pr_fix_all_run(
        &self,
        pr_id: i64,
        repo_dir: PathBuf,
        claimed_by: String,
        dry_run: bool,
    ) -> Result<i64> {
        let (project_key, project_name, _platform, _pr_number, pr_url) = self.pr_workflow_context(pr_id).await?;
        let issue_ids = self.open_issue_ids_for_pr(pr_id).await?;
        let workflow_run_id = self
            .start_workflow(project_key.clone(), project_name, pr_url, issue_ids.len().max(1) as i32)
            .await?;

        let background_service = self.clone();
        tokio::spawn(async move {
            if let Err(error) = background_service
                .execute_pr_fix_all_run(workflow_run_id, pr_id, repo_dir, claimed_by, dry_run)
                .await
            {
                let _ = background_service
                    .mark_workflow_failed(workflow_run_id, &error.to_string())
                    .await;
            }
        });

        Ok(workflow_run_id)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn verify_fix(
        &self,
        issue_id: i64,
        status: String,
        summary: String,
        evidence: Option<String>,
        gaps: Option<String>,
        residual_risks: Option<String>,
        next_actions: Option<String>,
    ) -> Result<VerifyFixResult> {
        let normalized_status = normalize_verification_status(&status)?;

        let mut tx = self.pool.begin().await?;

        let fix_run_id: i64 = sqlx::query_scalar(
            r#"
            SELECT id
            FROM fix_runs
            WHERE issue_id = $1
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(issue_id)
        .fetch_one(&mut *tx)
        .await?;

        let verification_id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO verifications (
                issue_id,
                fix_run_id,
                status,
                summary,
                evidence,
                gaps,
                residual_risks,
                next_actions
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id
            "#,
        )
        .bind(issue_id)
        .bind(fix_run_id)
        .bind(&normalized_status)
        .bind(&summary)
        .bind(serde_json::to_value(split_lines(evidence))?)
        .bind(serde_json::to_value(split_lines(gaps))?)
        .bind(serde_json::to_value(split_lines(residual_risks))?)
        .bind(serde_json::to_value(split_lines(next_actions))?)
        .fetch_one(&mut *tx)
        .await?;

        let issue_status = map_verification_status_to_issue_status(&normalized_status).to_string();

        sqlx::query(
            r#"
            UPDATE issues
            SET status = $2,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(issue_id)
        .bind(&issue_status)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(VerifyFixResult {
            issue_id,
            fix_run_id,
            verification_id,
            verification_status: normalized_status,
            issue_status,
            verified_at: Utc::now(),
        })
    }

    async fn count_actionable_issues(
        &self,
        project_key: &str,
        platform: &str,
        pr_number: i64,
    ) -> Result<i64> {
        let count = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM issues i
            JOIN pull_requests pr ON pr.id = i.pull_request_id
            JOIN projects p ON p.id = pr.project_id
            WHERE p.project_key = $1
              AND pr.platform = $2
              AND pr.pr_number = $3
              AND i.status IN ('open', 'reopened')
              AND i.severity IN ('critical', 'warning')
            "#,
        )
        .bind(project_key)
        .bind(platform)
        .bind(pr_number)
        .fetch_one(&self.pool)
        .await?;

        Ok(count)
    }

    async fn execute_issue_fix_run(
        &self,
        workflow_run_id: i64,
        issue_id: i64,
        repo_dir: PathBuf,
        claimed_by: String,
        dry_run: bool,
    ) -> Result<RunFixResult> {
        let (_project_key, _project_name, _platform, _pr_number, _pr_url) = self.issue_workflow_context(issue_id).await?;

        sqlx::query(
            r#"
            INSERT INTO workflow_rounds (workflow_run_id, round_number, status, summary)
            VALUES ($1, 1, 'running', $2)
            ON CONFLICT (workflow_run_id, round_number)
            DO UPDATE SET status = EXCLUDED.status, summary = EXCLUDED.summary, completed_at = NULL
            "#,
        )
        .bind(workflow_run_id)
        .bind(format!("FixAgent is processing issue {}.", issue_id))
        .execute(&self.pool)
        .await?;

        self.update_workflow_progress(
            workflow_run_id,
            Some(&format!("FixAgent is processing issue {}.", issue_id)),
            Some(1),
            &format!("FixAgent is processing issue {}.", issue_id),
        )
        .await?;

        let fix = self
            .run_fix_for_issue(repo_dir, issue_id, claimed_by, dry_run)
            .await?;

        let summary = format!(
            "FixAgent processed issue {} with issue status {} and fix status {}.",
            fix.issue_id, fix.issue_status, fix.fix_status
        );

        sqlx::query(
            r#"
            UPDATE workflow_rounds
            SET issue_id = $3,
                fix_run_id = $4,
                status = 'completed',
                stop_reason = 'issue_fix_completed',
                summary = $5,
                completed_at = NOW()
            WHERE workflow_run_id = $1 AND round_number = $2
            "#,
        )
        .bind(workflow_run_id)
        .bind(1)
        .bind(fix.issue_id)
        .bind(fix.fix_run_id)
        .bind(&summary)
        .execute(&self.pool)
        .await?;

        self.finish_workflow_run(
            workflow_run_id,
            "completed",
            "issue_fix_completed",
            Some(&summary),
        )
        .await?;
        Ok(fix)
    }

    async fn execute_pr_fix_all_run(
        &self,
        workflow_run_id: i64,
        pr_id: i64,
        repo_dir: PathBuf,
        claimed_by: String,
        dry_run: bool,
    ) -> Result<()> {
        let (project_key, _project_name, platform, pr_number, _pr_url) = self.pr_workflow_context(pr_id).await?;

        let issue_ids = self.open_issue_ids_for_pr(pr_id).await?;
        if issue_ids.is_empty() {
            self.finish_workflow_run(
                workflow_run_id,
                "completed",
                "no_open_issues",
                Some("Fix All skipped because this PR has no open issues."),
            )
            .await?;
            return Ok(());
        }

        let mut completed = 0usize;
        for (index, issue_id) in issue_ids.iter().enumerate() {
            let round_number = (index + 1) as i32;
            let round_summary = format!("FixAgent is processing issue {} ({}/{}).", issue_id, index + 1, issue_ids.len());

            sqlx::query(
                r#"
                INSERT INTO workflow_rounds (workflow_run_id, round_number, status, summary)
                VALUES ($1, $2, 'running', $3)
                "#,
            )
            .bind(workflow_run_id)
            .bind(round_number)
            .bind(&round_summary)
            .execute(&self.pool)
            .await?;

            self.update_workflow_progress(
                workflow_run_id,
                Some(&round_summary),
                Some(round_number),
                &round_summary,
            )
            .await?;

            let fix = self
                .run_fix_for_issue(repo_dir.clone(), *issue_id, claimed_by.clone(), dry_run)
                .await?;

            let summary = format!(
                "FixAgent processed issue {} with issue status {} and fix status {}.",
                fix.issue_id, fix.issue_status, fix.fix_status
            );

            sqlx::query(
                r#"
                UPDATE workflow_rounds
                SET issue_id = $3,
                    fix_run_id = $4,
                    status = 'completed',
                    stop_reason = 'issue_fix_completed',
                    summary = $5,
                    completed_at = NOW()
                WHERE workflow_run_id = $1 AND round_number = $2
                "#,
            )
            .bind(workflow_run_id)
            .bind(round_number)
            .bind(fix.issue_id)
            .bind(fix.fix_run_id)
            .bind(&summary)
            .execute(&self.pool)
            .await?;

            completed += 1;
        }

        let summary = format!("Fix All processed {} open issues for PR #{} on {}.", completed, pr_number, platform);
        self.finish_workflow_run(
            workflow_run_id,
            "completed",
            "fix_all_completed",
            Some(&summary),
        )
        .await?;

        let _ = project_key;

        Ok(())
    }

    async fn run_fix_for_issue(
        &self,
        repo_dir: PathBuf,
        issue_id: i64,
        claimed_by: String,
        dry_run: bool,
    ) -> Result<RunFixResult> {
        let mut tx = self.pool.begin().await?;

        let row = sqlx::query_as::<_, (i64, i64, String, String, i64, i64, String, String, String, Option<String>, Option<String>, Option<i32>)>(
            r#"
            UPDATE issues i
            SET status = 'claimed',
                claimed_by = $1,
                claimed_at = NOW(),
                updated_at = NOW()
            WHERE i.id = $2
              AND i.status IN ('open', 'reopened')
            RETURNING i.id, i.pull_request_id, i.severity, i.file_path, i.start_line, i.end_line, i.title, i.description, i.suggestion, i.suggestion_code, i.original_code, i.confidence
            "#,
        )
        .bind(&claimed_by)
        .bind(issue_id)
        .fetch_optional(&mut *tx)
        .await?;

        let (issue_id, pull_request_id, severity, file_path, start_line, end_line, title, description, suggestion, suggestion_code, original_code, confidence) =
            row.ok_or_else(|| OrchestratorError::Config(format!("No open issue found for issue_id={}", issue_id)))?;

        tx.commit().await?;

        self.execute_fix_for_issue_data(
            repo_dir,
            issue_id,
            pull_request_id,
            severity,
            file_path,
            start_line,
            end_line,
            title,
            description,
            suggestion,
            suggestion_code,
            original_code,
            confidence,
            dry_run,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn execute_fix_for_issue_data(
        &self,
        repo_dir: PathBuf,
        issue_id: i64,
        pull_request_id: i64,
        severity: String,
        file_path: String,
        start_line: i64,
        end_line: i64,
        title: String,
        description: String,
        suggestion: String,
        suggestion_code: Option<String>,
        original_code: Option<String>,
        confidence: Option<i32>,
        dry_run: bool,
    ) -> Result<RunFixResult> {
        let runner = FixRunner::new(repo_dir)
            .await
            .map_err(|e| OrchestratorError::Config(e.to_string()))?;

        let task = FixTask {
            issue_id: Some(issue_id),
            issue_index: 1,
            issue: reviewagent::llm::Issue {
                severity: parse_severity(&severity),
                file: file_path,
                line: start_line as usize,
                end_line: Some(end_line as usize),
                title,
                description,
                suggestion,
                suggestion_code,
                original_code,
                confidence: confidence.map(|v| v as u8),
            },
        };

        let fix_result = runner
            .run_task(task, dry_run)
            .await
            .map_err(|e| OrchestratorError::Config(e.to_string()))?;

        let fix_status = map_fix_status(&fix_result.status).to_string();
        let issue_status = map_issue_status(&fix_result.status).to_string();

        let mut tx = self.pool.begin().await?;

        let fix_run_id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO fix_runs (issue_id, status, summary, rationale, verification_steps, replacement_preview)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id
            "#,
        )
        .bind(issue_id)
        .bind(&fix_status)
        .bind(&fix_result.summary)
        .bind(&fix_result.rationale)
        .bind(serde_json::to_value(&fix_result.verification_steps)?)
        .bind(&fix_result.replacement_preview)
        .fetch_one(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            UPDATE issues
            SET status = $2,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(issue_id)
        .bind(&issue_status)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(RunFixResult {
            issue_id,
            pull_request_id,
            fix_run_id,
            issue_status,
            fix_status,
            processed_at: Utc::now(),
        })
    }

    async fn recover_interrupted_runs(&self) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE workflow_runs
            SET status = 'failed',
                stop_reason = 'service_restarted',
                summary = 'Task interrupted because the orchestrator service restarted before completion.',
                completed_at = NOW()
            WHERE status = 'running'
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            UPDATE workflow_rounds
            SET status = 'failed',
                stop_reason = 'service_restarted',
                summary = 'Round interrupted because the orchestrator service restarted before completion.',
                completed_at = NOW()
            WHERE status = 'running'
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn issue_workflow_context(
        &self,
        issue_id: i64,
    ) -> Result<(String, String, String, i64, String)> {
        sqlx::query_as::<_, (String, String, String, i64, String)>(
            r#"
            SELECT p.project_key, p.project_name, pr.platform, pr.pr_number, pr.pr_url
            FROM issues i
            JOIN pull_requests pr ON pr.id = i.pull_request_id
            JOIN projects p ON p.id = pr.project_id
            WHERE i.id = $1
            "#,
        )
        .bind(issue_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| OrchestratorError::Config(format!("Issue not found: {}", issue_id)))
    }

    async fn pr_workflow_context(
        &self,
        pr_id: i64,
    ) -> Result<(String, String, String, i64, String)> {
        sqlx::query_as::<_, (String, String, String, i64, String)>(
            r#"
            SELECT p.project_key, p.project_name, pr.platform, pr.pr_number, pr.pr_url
            FROM pull_requests pr
            JOIN projects p ON p.id = pr.project_id
            WHERE pr.id = $1
            "#,
        )
        .bind(pr_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| OrchestratorError::Config(format!("PR not found: {}", pr_id)))
    }

    async fn open_issue_ids_for_pr(&self, pr_id: i64) -> Result<Vec<i64>> {
        let rows = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT i.id
            FROM issues i
            WHERE i.pull_request_id = $1
              AND i.status = 'open'
            ORDER BY
              CASE i.severity
                WHEN 'critical' THEN 1
                WHEN 'warning' THEN 2
                ELSE 3
              END,
              COALESCE(i.confidence, 0) DESC,
              i.updated_at DESC
            "#,
        )
        .bind(pr_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    async fn finish_workflow_run(
        &self,
        workflow_run_id: i64,
        status: &str,
        stop_reason: &str,
        summary: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE workflow_runs
            SET status = $2,
                stop_reason = $3,
                summary = $4,
                completed_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(workflow_run_id)
        .bind(status)
        .bind(stop_reason)
        .bind(summary)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn update_workflow_progress(
        &self,
        workflow_run_id: i64,
        workflow_summary: Option<&str>,
        round_number: Option<i32>,
        round_summary: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE workflow_runs
            SET summary = $2
            WHERE id = $1
            "#,
        )
        .bind(workflow_run_id)
        .bind(workflow_summary.unwrap_or(round_summary))
        .execute(&self.pool)
        .await?;

        if let Some(round_number) = round_number {
            sqlx::query(
                r#"
                UPDATE workflow_rounds
                SET summary = $3
                WHERE workflow_run_id = $1 AND round_number = $2
                "#,
            )
            .bind(workflow_run_id)
            .bind(round_number)
            .bind(round_summary)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    async fn auto_verify_issue_from_latest_review(
        &self,
        issue_id: i64,
        latest_review_run_id: i64,
    ) -> Result<VerifyFixResult> {
        let still_present = self
            .issue_present_in_review_run(issue_id, latest_review_run_id)
            .await?;

        if still_present {
            self.verify_fix(
                issue_id,
                "failed".to_string(),
                "The post-fix review still reported the same issue fingerprint, so the fix did not fully resolve the problem.".to_string(),
                Some(format!(
                    "ReviewAgent reported the same issue again in review_run_id={}",
                    latest_review_run_id
                )),
                Some("The issue remains reproducible in automated review output".to_string()),
                Some("The next fix attempt may require a broader code change".to_string()),
                Some("Queue the issue for another fix attempt or escalate to human review if repeated failures continue".to_string()),
            )
            .await
        } else {
            self.verify_fix(
                issue_id,
                "verified".to_string(),
                "The post-fix review no longer reported the same issue fingerprint, so the fix is considered resolved by automated verification.".to_string(),
                Some(format!(
                    "ReviewAgent did not report the issue in review_run_id={}",
                    latest_review_run_id
                )),
                None,
                Some("Automated review cannot guarantee behavior-level correctness beyond the detected issue fingerprint".to_string()),
                Some("Optionally validate with tests or staging checks for higher confidence".to_string()),
            )
            .await
        }
    }

    async fn issue_present_in_review_run(
        &self,
        issue_id: i64,
        review_run_id: i64,
    ) -> Result<bool> {
        let found = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM issues
            WHERE id = $1
              AND review_run_id = $2
            "#,
        )
        .bind(issue_id)
        .bind(review_run_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(found > 0)
    }

    async fn workflow_run_summary(&self, workflow_run_id: i64) -> Result<WorkflowRunSummary> {
        let row = sqlx::query_as::<_, (i64, String, String, String, i64, String, String, Option<String>, i32, Option<String>, chrono::DateTime<Utc>, Option<chrono::DateTime<Utc>>)>(
            r#"
            SELECT id, project_key, project_name, platform, pr_number, pr_url, status, stop_reason, max_rounds, summary, started_at, completed_at
            FROM workflow_runs
            WHERE id = $1
            "#,
        )
        .bind(workflow_run_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| {
            OrchestratorError::Config(format!(
                "Workflow run not found: {}",
                workflow_run_id
            ))
        })?;

        Ok(WorkflowRunSummary {
            id: row.0,
            project_key: row.1,
            project_name: row.2,
            platform: row.3,
            pr_number: row.4,
            pr_url: row.5,
            status: row.6,
            stop_reason: row.7,
            max_rounds: row.8,
            summary: row.9,
            started_at: row.10,
            completed_at: row.11,
        })
    }
}

fn build_issue_fingerprint(
    pull_request_id: i64,
    issue: &reviewagent::llm::Issue,
) -> String {
    let end_line = issue.end_line.unwrap_or(issue.line);
    format!(
        "{}:{}:{}:{}:{}",
        pull_request_id,
        issue.file,
        issue.line,
        end_line,
        normalize_title(&issue.title)
    )
}

fn normalize_title(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
}

fn build_project_key(project_name: &str) -> String {
    let mut key = String::with_capacity(project_name.len());
    let mut last_was_dash = false;

    for ch in project_name.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            key.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash && !key.is_empty() {
            key.push('-');
            last_was_dash = true;
        }
    }

    while key.ends_with('-') {
        key.pop();
    }

    if key.is_empty() {
        "project".to_string()
    } else {
        key
    }
}

fn map_fix_status(status: &FixExecutionStatus) -> &'static str {
    match status {
        FixExecutionStatus::Applied => "applied",
        FixExecutionStatus::Suggested => "suggested",
        FixExecutionStatus::NeedsHuman => "needs_human",
        FixExecutionStatus::InvalidCandidate => "invalid_candidate",
        FixExecutionStatus::NotReproducible => "not_reproducible",
    }
}

fn map_issue_status(status: &FixExecutionStatus) -> &'static str {
    match status {
        FixExecutionStatus::Applied => "fixed_pending_verification",
        FixExecutionStatus::Suggested => "fixed_pending_verification",
        FixExecutionStatus::NeedsHuman => "needs_human",
        FixExecutionStatus::InvalidCandidate => "invalid",
        FixExecutionStatus::NotReproducible => "not_reproducible",
    }
}

fn parse_severity(value: &str) -> reviewagent::llm::Severity {
    match value {
        "critical" => reviewagent::llm::Severity::Critical,
        "suggestion" => reviewagent::llm::Severity::Suggestion,
        _ => reviewagent::llm::Severity::Warning,
    }
}

fn normalize_verification_status(value: &str) -> Result<String> {
    let normalized = value.trim().to_lowercase();
    match normalized.as_str() {
        "verified" | "partially_verified" | "not_verifiable_in_current_env"
        | "external_validation_required" | "failed" | "needs_human" => Ok(normalized),
        _ => Err(OrchestratorError::Config(format!(
            "Unsupported verification status: {}",
            value
        ))),
    }
}

fn map_verification_status_to_issue_status(status: &str) -> &'static str {
    match status {
        "verified" => "resolved",
        "partially_verified" => "fixed_pending_verification",
        "not_verifiable_in_current_env" => "external_validation_required",
        "external_validation_required" => "external_validation_required",
        "failed" => "reopened",
        "needs_human" => "needs_human",
        _ => "needs_human",
    }
}

fn split_lines(value: Option<String>) -> Vec<String> {
    value
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn load_reviewagent_config(repo_dir: &std::path::Path) -> Result<reviewagent::config::Config> {
    for candidate in [".reviewagent.toml", "reviewagent.toml"] {
        let path = repo_dir.join(candidate);
        if path.exists() {
            let mut config = reviewagent::config::Config::load(&path)
                .map_err(|e| OrchestratorError::Config(e.to_string()))?;
            apply_workspace_env_overrides(repo_dir, &mut config)?;
            return Ok(config);
        }
    }
    let mut config = reviewagent::config::Config::default();
    apply_workspace_env_overrides(repo_dir, &mut config)?;
    Ok(config)
}

fn apply_workspace_env_overrides(
    repo_dir: &std::path::Path,
    config: &mut reviewagent::config::Config,
) -> Result<()> {
    let env_path = repo_dir.join("env");
    if !env_path.exists() {
        return Ok(());
    }

    let content = std::fs::read_to_string(&env_path)?;
    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };

        let value = value.trim().trim_matches('"').trim_matches('\'').to_string();
        match key.trim().to_ascii_lowercase().as_str() {
            "baseurl" => {
                config.llm.base_url = Some(value.clone());
                config.llm_lite.base_url = Some(value);
            }
            "apikey" => {
                config.llm.api_key = Some(value.clone());
                config.llm_lite.api_key = Some(value);
            }
            "model" => {
                config.llm.model = value.clone();
                config.llm_lite.model = value;
            }
            _ => {}
        }
    }

    Ok(())
}

fn detect_platform_name_and_pr_number(url: &str) -> Option<(String, i64)> {
    let github = regex::Regex::new(r"^https://github\.com/[^/]+/[^/]+/pull/(\d+)").ok()?;
    if let Some(caps) = github.captures(url) {
        return caps
            .get(1)?
            .as_str()
            .parse()
            .ok()
            .map(|n| ("github".to_string(), n));
    }

    let gitee = regex::Regex::new(r"^https://gitee\.com/[^/]+/[^/]+/pulls/(\d+)").ok()?;
    if let Some(caps) = gitee.captures(url) {
        return caps
            .get(1)?
            .as_str()
            .parse()
            .ok()
            .map(|n| ("gitee".to_string(), n));
    }

    let gitlab = regex::Regex::new(r"^https?://[^/]+/.+?/-/merge_requests/(\d+)").ok()?;
    if let Some(caps) = gitlab.captures(url) {
        return caps
            .get(1)?
            .as_str()
            .parse()
            .ok()
            .map(|n| ("gitlab".to_string(), n));
    }

    let gitea = regex::Regex::new(r"^https?://[^/]+/[^/]+/[^/]+/pulls/(\d+)").ok()?;
    if let Some(caps) = gitea.captures(url) {
        return caps
            .get(1)?
            .as_str()
            .parse()
            .ok()
            .map(|n| ("gitea".to_string(), n));
    }

    None
}
