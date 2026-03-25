use std::path::PathBuf;

use clap::{Parser, Subcommand};
use fridi_core::backlog::{Backlog, backlog_path};

mod run;
mod spawner;

#[derive(Parser)]
#[command(name = "fridi", about = "AI workflow orchestrator")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a workflow from a YAML file
    Run {
        /// Path to the workflow YAML file
        workflow: PathBuf,

        /// Repository in owner/repo format
        #[arg(long)]
        repo: Option<String>,

        /// Directory containing agent definition YAML files
        #[arg(long, default_value = "agents")]
        agents_dir: PathBuf,

        /// Directory for session storage
        #[arg(long, default_value = ".fridi/sessions")]
        sessions_dir: PathBuf,
    },

    /// Append an item to the local backlog
    Backlog {
        /// The backlog item text (supports inline #tags and !/!! priority)
        text: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Run {
            workflow,
            repo,
            agents_dir,
            sessions_dir,
        } => run::execute(workflow, repo, agents_dir, sessions_dir).await,
        Commands::Backlog { text } => {
            let path = backlog_path();
            let mut backlog = Backlog::load(&path)?;
            backlog.add(&text, None);
            backlog.save()?;
            println!("Added to backlog: {text}");
            Ok(())
        }
    }
}
