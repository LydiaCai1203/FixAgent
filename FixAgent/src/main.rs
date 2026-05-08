use anyhow::Result;
use clap::Parser;
use fixagent::cli::{Cli, Commands};
use fixagent::runner::FixRunner;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .init();

    match cli.command {
        Commands::Run {
            repo_dir,
            review_file,
            issue_index,
            output,
            dry_run,
        } => {
            let runner = FixRunner::new(repo_dir).await?;
            let result = runner.run(review_file, issue_index, dry_run).await?;
            let json = serde_json::to_string_pretty(&result)?;

            if let Some(output_path) = output {
                tokio::fs::write(output_path, &json).await?;
            } else {
                println!("{}", json);
            }
        }
    }

    Ok(())
}
