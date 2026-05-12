use clap::{Parser, Subcommand};
use std::path::PathBuf;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(name = "orchestrator")]
#[command(about = "Persist review and fix workflow data", long_about = None)]
#[command(version = VERSION)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Run ReviewAgent directly and ingest the result into PostgreSQL
    RunReview {
        #[arg(long, default_value = ".")]
        repo_dir: PathBuf,

        #[arg(long)]
        project_key: String,

        #[arg(long)]
        project_name: String,

        #[arg(long)]
        pr_url: String,
    },

    /// Run a minimal workflow: review first, then fix one issue if available
    RunWorkflow {
        #[arg(long, default_value = ".")]
        repo_dir: PathBuf,

        #[arg(long)]
        project_key: String,

        #[arg(long)]
        project_name: String,

        #[arg(long)]
        pr_url: String,

        #[arg(long, default_value = "orchestrator")]
        claimed_by: String,

        #[arg(long)]
        dry_run: bool,
    },

    /// Run review/fix rounds until no important issue remains
    RunUntilStable {
        #[arg(long, default_value = ".")]
        repo_dir: PathBuf,

        #[arg(long)]
        project_key: String,

        #[arg(long)]
        project_name: String,

        #[arg(long)]
        pr_url: String,

        #[arg(long, default_value = "orchestrator")]
        claimed_by: String,

        #[arg(long, default_value_t = 5)]
        max_rounds: i32,

        #[arg(long)]
        dry_run: bool,
    },

    /// Serve a minimal HTTP API for workflow execution and queries
    ServeHttp {
        #[arg(long)]
        host: Option<String>,

        #[arg(long)]
        port: Option<u16>,
    },

    /// Ingest a ReviewAgent JSON result into PostgreSQL
    IngestReview {
        #[arg(long)]
        project_key: String,

        #[arg(long)]
        project_name: String,

        #[arg(long)]
        platform: String,

        #[arg(long)]
        pr_number: i64,

        #[arg(long)]
        pr_url: String,

        #[arg(long)]
        review_file: PathBuf,

        #[arg(long)]
        commit_sha: Option<String>,
    },

    /// Claim one open issue and persist the FixAgent result
    RunFix {
        #[arg(long, default_value = ".")]
        repo_dir: PathBuf,

        #[arg(long)]
        project_key: String,

        #[arg(long)]
        platform: String,

        #[arg(long)]
        pr_number: i64,

        #[arg(long, default_value = "orchestrator")]
        claimed_by: String,

        #[arg(long)]
        dry_run: bool,
    },

    /// Record verification for the latest fix run of an issue
    VerifyFix {
        #[arg(long)]
        issue_id: i64,

        #[arg(long)]
        status: String,

        #[arg(long)]
        summary: String,

        #[arg(long)]
        evidence: Option<String>,

        #[arg(long)]
        gaps: Option<String>,

        #[arg(long)]
        residual_risks: Option<String>,

        #[arg(long)]
        next_actions: Option<String>,
    },

    /// List all tracked projects
    ListProjects,

    /// List PR/MR records for a project
    ListPrs {
        #[arg(long)]
        project_key: String,
    },

    /// List issues for a PR/MR
    ListIssues {
        #[arg(long)]
        project_key: String,

        #[arg(long)]
        platform: String,

        #[arg(long)]
        pr_number: i64,

        #[arg(long)]
        status: Option<String>,
    },

    /// Show aggregated stats for a PR/MR
    PrStats {
        #[arg(long)]
        project_key: String,

        #[arg(long)]
        platform: String,

        #[arg(long)]
        pr_number: i64,
    },

    /// List workflow runs, optionally filtered by project
    ListWorkflows {
        #[arg(long)]
        project_key: Option<String>,
    },

    /// Show workflow run detail with rounds
    WorkflowDetail {
        #[arg(long)]
        workflow_run_id: i64,
    },

    /// List rounds for a workflow run
    WorkflowRounds {
        #[arg(long)]
        workflow_run_id: i64,
    },
}
