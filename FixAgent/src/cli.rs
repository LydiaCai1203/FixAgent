use clap::{Parser, Subcommand};
use std::path::PathBuf;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(name = "fixagent")]
#[command(about = "Apply minimal fixes from review issues", long_about = None)]
#[command(version = VERSION)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Fix a single issue from a ReviewAgent JSON report
    Run {
        /// Repository root where target files live
        #[arg(long, default_value = ".")]
        repo_dir: PathBuf,

        /// ReviewAgent JSON output file
        #[arg(long)]
        review_file: PathBuf,

        /// 1-based issue index in the review JSON
        #[arg(long, default_value_t = 1)]
        issue_index: usize,

        /// Output file for structured fix result
        #[arg(long)]
        output: Option<PathBuf>,

        /// Do not write changes back to disk
        #[arg(long)]
        dry_run: bool,
    },
}
