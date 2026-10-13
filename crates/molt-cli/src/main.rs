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
    /// Index source syntax, contract candidates and coverage issues into local SQLite.
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
    /// Find a top-level JS/TS declaration by exact name.
    Symbols {
        name: String,
        #[arg(long)]
        path: Option<String>,
        #[arg(long, default_value_t = 25)]
        limit: u32,
        #[arg(long, default_value_t = 0)]
        offset: u32,
    },
    /// List static imports/re-exports and file-level resolution for an indexed relative path.
    Imports {
        path: String,
        #[arg(long, default_value_t = 25)]
        limit: u32,
        #[arg(long, default_value_t = 0)]
        offset: u32,
    },
    /// List indexed files importing an exact relative target file (resolved edges only).
    Dependents {
        path: String,
        #[arg(long, default_value_t = 25)]
        limit: u32,
        #[arg(long, default_value_t = 0)]
        offset: u32,
    },
    /// Query syntax-derived GraphQL fields, Spring mappings or HTTP call candidates.
    Contracts {
        #[arg(long, value_parser = ["graphql", "http", "outbound_http_candidate"])]
        category: String,
        #[arg(long)]
        path: Option<String>,
        #[arg(long)]
        method: Option<String>,
        #[arg(long)]
        route: Option<String>,
        #[arg(long, default_value_t = 25)]
        limit: u32,
        #[arg(long, default_value_t = 0)]
        offset: u32,
    },
    /// Show literal HTTP call / Spring route path matches (unverified candidates).
    ContractIssues {
        #[arg(long)]
        path: Option<String>,
        #[arg(long, default_value_t = 25)]
        limit: u32,
        #[arg(long, default_value_t = 0)]
        offset: u32,
    },
    /// Show literal HTTP call / Spring route path matches (unverified candidates).
    HttpLinks {
        #[arg(long)]
        route: Option<String>,
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
            Query::Symbols {
                name,
                path,
                limit,
                offset,
            } => serde_json::to_value(molt_index::symbols(
                &source,
                &name,
                path.as_deref(),
                limit,
                offset,
            )?)?,
            Query::Imports {
                path,
                limit,
                offset,
            } => serde_json::to_value(molt_index::imports(&source, &path, limit, offset)?)?,
            Query::Dependents {
                path,
                limit,
                offset,
            } => serde_json::to_value(molt_index::dependents(&source, &path, limit, offset)?)?,
            Query::Contracts {
                category,
                path,
                method,
                route,
                limit,
                offset,
            } => serde_json::to_value(molt_index::contracts(
                &source,
                &category,
                path.as_deref(),
                method.as_deref(),
                route.as_deref(),
                limit,
                offset,
            )?)?,
            Query::ContractIssues {
                path,
                limit,
                offset,
            } => serde_json::to_value(molt_index::contract_issues(
                &source,
                path.as_deref(),
                limit,
                offset,
            )?)?,
            Query::HttpLinks {
                route,
                limit,
                offset,
            } => serde_json::to_value(molt_index::http_links(
                &source,
                route.as_deref(),
                limit,
                offset,
            )?)?,
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
