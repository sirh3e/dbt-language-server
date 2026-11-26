//! The command line: what to format, by which rules, and what it says.
//! Exit codes: 0 done, 1 `--check` found files to reformat, 2 a usage,
//! config or I/O error. A file the formatter declines is reported and
//! left as it is; it fails `--check` only with `--strict`.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use cogitator_format::{config, format, Config, Declined};

#[derive(Debug, Parser)]
#[command(name = "cogitator-format", version, about)]
struct Args {
    /// Files or directories (their `.sql` files) to format; `-` is stdin.
    paths: Vec<PathBuf>,
    /// Change nothing: list the files that would change, exit 1 if any.
    #[arg(long)]
    check: bool,
    /// With --check, print each file's unified diff.
    #[arg(long, requires = "check")]
    diff: bool,
    /// With --check, a declined file counts as one to reformat.
    #[arg(long, requires = "check")]
    strict: bool,
    /// The rules (TOML); default: the nearest cogitator-format.toml at or
    /// above each file.
    #[arg(long)]
    config: Option<PathBuf>,
    /// The dialect, over the config's.
    #[arg(long)]
    dialect: Option<String>,
    /// With `-`, the path stdin stands for (to find its config).
    #[arg(long)]
    stdin_filename: Option<PathBuf>,
    /// Print the rules a file would be formatted by, and exit.
    #[arg(long)]
    print_config: bool,
    /// Say, for each file, how its Jinja was masked and why a way failed.
    #[arg(long)]
    explain: bool,
}

const USAGE: u8 = 2;

pub fn run() -> ExitCode {
    let args = Args::parse();
    match run_args(&args) {
        Ok(code) => ExitCode::from(code),
        Err(message) => {
            eprintln!("cogitator-format: {message}");
            ExitCode::from(USAGE)
        }
    }
}

fn run_args(args: &Args) -> Result<u8, String> {
    let rules = |at: &Path| -> Result<Config, String> {
        let base = match &args.config {
            Some(path) => std::fs::read_to_string(path)
                .map_err(|e| format!("{}: {e}", path.display()))
                .and_then(|text| Config::from_toml(&text, path).map_err(|e| e.to_string()))?,
            None => config::load_for(at).map_err(|e| e.to_string())?,
        };
        match &args.dialect {
            Some(dialect) => Config {
                dialect: dialect.clone(),
                ..base
            }
            .checked()
            .map_err(|e| e.to_string()),
            None => Ok(base),
        }
    };
    let here = std::env::current_dir().map_err(|e| e.to_string())?;
    if args.print_config {
        let at = args.paths.first().unwrap_or(&here);
        println!("{:#?}", rules(at)?);
        return Ok(0);
    }
    if args.explain {
        for file in sql_files(&args.paths).map_err(|e| e.to_string())? {
            let text =
                std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
            println!("{}:", file.display());
            for line in cogitator_format::explain(&text, &rules(&file)?) {
                println!("  {line}");
            }
        }
        return Ok(0);
    }
    if args.paths.iter().any(|p| p.as_os_str() == "-") {
        let at = args.stdin_filename.clone().unwrap_or(here);
        return stdin(&rules(&at)?);
    }
    let files = sql_files(&args.paths).map_err(|e| e.to_string())?;
    let mut tally = Tally::default();
    for file in &files {
        tally.add(file, &rules(file)?, args)?;
    }
    eprintln!(
        "cogitator-format: {} file(s) {}, {} unchanged, {} declined",
        tally.changed,
        if args.check { "would be reformatted" } else { "reformatted" },
        files.len() - tally.changed - tally.declined,
        tally.declined
    );
    let failed = tally.changed > 0 || (args.strict && tally.declined > 0);
    Ok(u8::from(args.check && failed))
}

fn stdin(rules: &Config) -> Result<u8, String> {
    let mut text = String::new();
    std::io::stdin()
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    let out = format(&text, rules).unwrap_or_else(|declined| {
        eprintln!("cogitator-format: left as it is: {declined}");
        text.clone()
    });
    std::io::stdout()
        .write_all(out.as_bytes())
        .map_err(|e| e.to_string())?;
    Ok(0)
}

#[derive(Default)]
struct Tally {
    changed: usize,
    declined: usize,
}

impl Tally {
    fn add(&mut self, file: &Path, rules: &Config, args: &Args) -> Result<(), String> {
        let text = std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
        match format(&text, rules) {
            Ok(out) if out == text => {}
            Ok(out) => {
                self.changed += 1;
                if args.check {
                    println!("would reformat {}", file.display());
                    if args.diff {
                        print!("{}", diff(file, &text, &out));
                    }
                } else {
                    std::fs::write(file, &out).map_err(|e| format!("{}: {e}", file.display()))?;
                    println!("reformatted {}", file.display());
                }
            }
            Err(declined) => {
                self.declined += 1;
                eprintln!("declined {}: {}", file.display(), reason(&declined));
            }
        }
        Ok(())
    }
}

fn reason(declined: &Declined) -> String {
    declined.to_string()
}

fn diff(path: &Path, before: &str, after: &str) -> String {
    let name = path.display().to_string();
    similar::TextDiff::from_lines(before, after)
        .unified_diff()
        .header(&format!("a/{name}"), &format!("b/{name}"))
        .to_string()
}

/// The `.sql` files of the paths, sorted; directories are walked, leaving
/// out hidden ones and what dbt builds or installs.
fn sql_files(paths: &[PathBuf]) -> std::io::Result<Vec<PathBuf>> {
    let mut files = paths
        .iter()
        .map(|path| walk(path))
        .collect::<std::io::Result<Vec<_>>>()?
        .concat();
    files.sort();
    files.dedup();
    Ok(files)
}

fn walk(path: &Path) -> std::io::Result<Vec<PathBuf>> {
    if !path.is_dir() {
        return Ok(vec![path.to_path_buf()]);
    }
    let skipped = |name: &str| {
        name.starts_with('.') || matches!(name, "target" | "dbt_packages" | "dbt_modules" | "logs")
    };
    std::fs::read_dir(path)?
        .map(|entry| {
            let entry = entry?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            Ok(if entry.file_type()?.is_dir() {
                if skipped(&name) {
                    Vec::new()
                } else {
                    walk(&path)?
                }
            } else if path.extension().is_some_and(|e| e == "sql") {
                vec![path]
            } else {
                Vec::new()
            })
        })
        .collect::<std::io::Result<Vec<_>>>()
        .map(|nested| nested.concat())
}
