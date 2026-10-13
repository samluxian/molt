use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "molt",
    version,
    about = "MOLT: local codebase inventory for agent-assisted Rust modernization"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Index JS/TS/Java source file paths and hashes into a local SQLite database.
    Analyze {
        #[arg(long, default_value = ".")]
        source: PathBuf,
    },
    /// Query the local index (JSON output).
    Query {
        #[arg(long, default_value = ".")]
        source: PathBuf,
        #[command(subcommand)]
        query: Query,
    },
}

#[derive(Subcommand)]
enum Query {
    /// Count indexed files by language.
    ProjectSummary,
    /// List source files with bounded pagination (not source contents).
    Files {
        #[arg(long, default_value_t = 25)]
        limit: u32,
        #[arg(long, default_value_t = 0)]
        offset: u32,
    },
}

fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Analyze { source } => serde_json::to_value(molt_index::analyze(&source)?)?,
        Command::Query { source, query } => match query {
            Query::ProjectSummary => serde_json::to_value(molt_index::summary(&source)?)?,
            Query::Files { limit, offset } => {
                serde_json::to_value(molt_index::files(&source, limit, offset)?)?
            }
        },
    };
    println!("{}", serde_json::to_string(&result)?);
    Ok(())
}

fn main() {
    if let Err(err) = run() {
        eprintln!(
            "{}",
            serde_json::json!({"status": "failed", "error_code": "COMMAND_FAILED", "message": format!("{err:#}")})
        );
        std::process::exit(1);
    }
}
