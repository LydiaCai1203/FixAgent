use anyhow::Result;
use clap::Parser;
use orchestrator::cli::{Cli, Commands};
use orchestrator::service::OrchestratorService;
use orchestrator::web::serve_http;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .init();

    let service = OrchestratorService::new_from_env().await?;

    match cli.command {
        Commands::RunReview {
            repo_dir,
            project_key,
            project_name,
            pr_url,
        } => {
            let result = service
                .run_review(repo_dir, project_key, project_name, pr_url)
                .await?;

            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::RunWorkflow {
            repo_dir,
            project_key,
            project_name,
            pr_url,
            claimed_by,
            dry_run,
        } => {
            let result = service
                .run_workflow(repo_dir, project_key, project_name, pr_url, claimed_by, dry_run)
                .await?;

            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::RunUntilStable {
            repo_dir,
            project_key,
            project_name,
            pr_url,
            claimed_by,
            max_rounds,
            dry_run,
        } => {
            let result = service
                .run_until_stable(
                    repo_dir,
                    project_key,
                    project_name,
                    pr_url,
                    claimed_by,
                    max_rounds,
                    dry_run,
                )
                .await?;

            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::ServeHttp { host, port } => {
            serve_http(service, host, port).await?;
        }
        Commands::IngestReview {
            project_key,
            project_name,
            platform,
            pr_number,
            pr_url,
            review_file,
            commit_sha,
        } => {
            let result = service
                .ingest_review(
                    project_key,
                    project_name,
                    platform,
                    pr_number,
                    pr_url,
                    review_file,
                    commit_sha,
                )
                .await?;

            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::RunFix {
            repo_dir,
            project_key,
            platform,
            pr_number,
            claimed_by,
            dry_run,
        } => {
            let result = service
                .run_fix_for_next_issue(
                    repo_dir,
                    project_key,
                    platform,
                    pr_number,
                    claimed_by,
                    dry_run,
                )
                .await?;

            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::VerifyFix {
            issue_id,
            status,
            summary,
            evidence,
            gaps,
            residual_risks,
            next_actions,
        } => {
            let result = service
                .verify_fix(
                    issue_id,
                    status,
                    summary,
                    evidence,
                    gaps,
                    residual_risks,
                    next_actions,
                )
                .await?;

            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::ListProjects => {
            let result = service.list_projects().await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::ListPrs { project_key } => {
            let result = service.list_prs(project_key).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::ListIssues {
            project_key,
            platform,
            pr_number,
            status,
        } => {
            let result = service
                .list_issues(Some(project_key), Some(platform), Some(pr_number), status)
                .await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::PrStats {
            project_key,
            platform,
            pr_number,
        } => {
            let result = service.pr_stats(project_key, platform, pr_number).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::ListWorkflows { project_key } => {
            let result = service.list_workflows(project_key).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::WorkflowDetail { workflow_run_id } => {
            let result = service.workflow_detail(workflow_run_id).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::WorkflowRounds { workflow_run_id } => {
            let result = service.workflow_rounds(workflow_run_id).await?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
    }

    Ok(())
}
