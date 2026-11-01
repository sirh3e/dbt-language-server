//! cogitator — the dbt language server binary: CLI entry (specs/rust-port/04 §1) and runtime
//! bootstrap (05 §6.2).

mod observability;
mod server;

use anyhow::Result;
use clap::{Parser, Subcommand};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(name = "cogitator", disable_version_flag = true)]
struct Cli {
    /// Print version
    #[arg(short = 'v', long)]
    version: bool,
    /// Enable debug logging to log.txt (next to the executable)
    #[arg(short = 'd', long)]
    debug: bool,
    /// Enable dbt fusion features. Provide a path if the binary is not `dbt`.
    #[arg(short = 'f', long, num_args = 0..=1, default_missing_value = "dbt")]
    fusion: Option<String>,
    /// Use the phase-1 minijinja+sqlparser pipeline instead of the
    /// tree-sitter backend (06 §3.5 fallback switch).
    #[arg(long = "no-tree-sitter")]
    no_tree_sitter: bool,
    /// Export traces via OTLP (optional endpoint; also honors
    /// OTEL_EXPORTER_OTLP_ENDPOINT). Off by default (05 §6.3).
    #[arg(long, num_args = 0..=1, default_missing_value = "")]
    otlp: Option<String>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Self-update via the install script
    Upgrade,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if matches!(cli.command, Some(Command::Upgrade)) {
        let status = std::process::Command::new("bash")
            .arg("-c")
            .arg("curl -fsSL https://raw.githubusercontent.com/j-clemons/dbt-language-server/main/install | bash")
            .status()?;
        std::process::exit(status.code().unwrap_or(1));
    }

    if cli.version {
        println!("{VERSION}");
        return Ok(());
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        observability::init(cli.debug, cli.otlp.as_deref())?;
        tracing::info!(version = VERSION, "dbt Language Server Started!");
        server::run(cli.fusion.unwrap_or_default(), !cli.no_tree_sitter).await
    })
}
