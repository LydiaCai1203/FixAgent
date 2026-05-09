use crate::config::{FixAgentConfig, load_reviewagent_config};
use crate::error::{FixError, Result};
use crate::models::FixResult;
use crate::orchestrator::FixOrchestrator;
use std::path::PathBuf;

pub struct FixRunner {
    orchestrator: FixOrchestrator,
}

impl FixRunner {
    pub async fn new(repo_dir: PathBuf) -> Result<Self> {
        let orchestrator = FixOrchestrator::new(repo_dir).await?;
        Ok(Self { orchestrator })
    }

    pub async fn run(
        &self,
        review_file: PathBuf,
        issue_index: usize,
        dry_run: bool,
    ) -> Result<FixResult> {
        self.orchestrator.run_from_file(review_file, issue_index, dry_run).await
    }

    pub async fn run_task(
        &self,
        task: crate::models::FixTask,
        dry_run: bool,
    ) -> Result<FixResult> {
        self.orchestrator.run_task(task, dry_run).await
    }
}
