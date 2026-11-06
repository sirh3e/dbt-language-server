//! Test-only stand-in for the `dbt` CLI (built behind the `e2e`
//! feature, never shipped): behavior is scripted entirely through env
//! vars so the e2e suites control it cross-platform — no shell shims.
//!
//! - FAKE_DBT_ARGS_LOG: append one line of argv per invocation
//! - FAKE_DBT_SLEEP_MS: sleep before doing anything else
//! - FAKE_DBT_STDOUT / FAKE_DBT_STDERR: verbatim output
//! - FAKE_DBT_EXIT_CODE: exit status (default 0)

use std::io::Write;

fn main() {
    if let Ok(log) = std::env::var("FAKE_DBT_ARGS_LOG") {
        let line = std::env::args().skip(1).collect::<Vec<_>>().join(" ");
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log)
        {
            let _ = writeln!(file, "{line}");
        }
    }
    if let Some(ms) = std::env::var("FAKE_DBT_SLEEP_MS")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }
    if let Ok(out) = std::env::var("FAKE_DBT_STDOUT") {
        print!("{out}");
        let _ = std::io::stdout().flush();
    }
    if let Ok(err) = std::env::var("FAKE_DBT_STDERR") {
        eprint!("{err}");
    }
    let code = std::env::var("FAKE_DBT_EXIT_CODE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    std::process::exit(code);
}
