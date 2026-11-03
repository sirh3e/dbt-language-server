//! cogitator — the dbt language server binary: CLI entry (specs/rust-port/04 §1) and runtime
//! bootstrap (05 §6.2).

mod metrics;
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
    /// Directory containing profiles.yml (`~` is expanded). Takes precedence
    /// over DBT_PROFILES_DIR and the workspace lookups; a client-side
    /// initializationOptions.profilesDir replaces it at initialize time.
    #[arg(short = 'p', long = "profiles-dir")]
    profiles_dir: Option<String>,
    /// Active dbt target (replaces the profile's default `target:`); also
    /// passed as `--target` to dbt run/test/compile. Switchable at runtime
    /// via the dbt.setTarget command.
    #[arg(short = 't', long = "target")]
    target: Option<String>,
    /// Profiles readable besides the project's own (comma-separated).
    /// By default only dbt_project.yml's profile is parsed and exposed.
    #[arg(long = "profiles-allow", value_delimiter = ',')]
    profiles_allow: Vec<String>,
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
        // Deliberately conservative: no self-mutating curl|sh from inside
        // the server. Point the user at this project's own installer (the
        // old one belonged to the upstream Go releases).
        eprintln!("cogitator: self-upgrade does not run from inside the server.");
        eprintln!("Solution: re-run this project's installer:");
        eprintln!("  curl -fsSL https://raw.githubusercontent.com/sirh3e/dbt-language-server/main/install.sh | sh");
        eprintln!("(Windows: install.ps1), or download an asset from");
        eprintln!("  https://github.com/sirh3e/dbt-language-server/releases");
        std::process::exit(2);
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
        server::run(
            cli.fusion.unwrap_or_default(),
            !cli.no_tree_sitter,
            server::ProfilesConfig {
                dir_override: cli.profiles_dir.unwrap_or_default(),
                allow: cli.profiles_allow,
                profile: String::new(),
                target: cli.target.unwrap_or_default(),
            },
        )
        .await
    })
}
