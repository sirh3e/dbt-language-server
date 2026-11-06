//! cogitator — the dbt language server binary: CLI entry (specs/rust-port/04 §1) and runtime
//! bootstrap (05 §6.2).

mod client_log;
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
    /// Config file (cogitator.yml schema) read at launch — the only way
    /// the file's `startup:` keys apply, since the workspace root (whose
    /// cogitator.yml hot-reloads the rest) is unknown before initialize.
    /// CLI flags override file values.
    #[arg(short = 'c', long = "config")]
    config: Option<String>,
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

    // --config loads BEFORE observability::init: the subscriber and the
    // parser backend are install-once, so `startup:` only works from here.
    let launch = match cli.config.as_deref() {
        Some(path) => load_launch_config(path),
        None => server::config_file::FileConfig::default(),
    };
    let debug = cli.debug || launch.startup.debug.unwrap_or(false);
    let otlp = cli.otlp.or_else(|| launch.startup.otlp.clone());
    let tree_sitter = !cli.no_tree_sitter && launch.startup.tree_sitter.unwrap_or(true);
    let fusion = cli
        .fusion
        .or_else(|| launch.startup.fusion.clone())
        .unwrap_or_default();
    // launch precedence: defaults < config file < CLI flags (the server
    // applies initializationOptions on top at initialize)
    let profiles = server::ProfilesConfig {
        dir_override: cli
            .profiles_dir
            .or_else(|| launch.profiles.dir.clone())
            .unwrap_or_default(),
        allow: if cli.profiles_allow.is_empty() {
            launch.profiles.allow.clone().unwrap_or_default()
        } else {
            cli.profiles_allow
        },
        profile: launch.profiles.profile.clone().unwrap_or_default(),
        target: cli
            .target
            .or_else(|| launch.profiles.target.clone())
            .unwrap_or_default(),
    };

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        observability::init(debug, otlp.as_deref())?;
        tracing::info!(version = VERSION, "dbt Language Server Started!");
        server::run(fusion, tree_sitter, profiles, launch).await
    })
}

/// A broken --config file is a launch error, not something to limp past:
/// its Guidance goes to stderr (stdout is reserved for LSP frames).
fn load_launch_config(path: &str) -> server::config_file::FileConfig {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => {
            eprintln!(
                "{}",
                cogitator_error::guidance::Guidance::new("--config file is unreadable.")
                    .at(path.to_string())
                    .because(err.to_string())
                    .fix("pass an existing cogitator.yml-schema file, or drop --config.")
                    .to_message()
            );
            std::process::exit(2);
        }
    };
    match server::config_file::parse(&text, std::path::Path::new(path)) {
        Ok(cfg) => cfg,
        Err(guidance) => {
            eprintln!("{}", guidance.to_message());
            std::process::exit(2);
        }
    }
}
