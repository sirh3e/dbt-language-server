//! Test-only stand-in for the `dbt` CLI (built behind the `e2e`
//! feature, never shipped): behavior is scripted entirely through env
//! vars so the e2e suites control it cross-platform — no shell shims.
//!
//! - FAKE_DBT_ARGS_LOG: append one line of argv per invocation
//! - FAKE_DBT_SLEEP_MS: sleep before doing anything else
//! - FAKE_DBT_STDOUT / FAKE_DBT_STDERR: verbatim output
//! - FAKE_DBT_EXIT_CODE: exit status (default 0)
//! - FAKE_DBT_SHOW_ROWS: rows `show --inline … --output json` answers
//!   (default 3) when FAKE_DBT_STDOUT is unset, as dbt prints them: a
//!   log line, then `{"node": …, "show": [{"n": …, "sql": <the query>}]}`
//!   with no more rows than `--limit`

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
    } else if let Some(table) = show_inline(&std::env::args().skip(1).collect::<Vec<_>>()) {
        print!("{table}");
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

/// dbt show's JSON answer for `show --inline <sql> … --output json`.
fn show_inline(args: &[String]) -> Option<String> {
    let value_of = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
    };
    if args.first().map(String::as_str) != Some("show")
        || value_of("--output").map(String::as_str) != Some("json")
    {
        return None;
    }
    let sql = value_of("--inline")?;
    let wanted: usize = std::env::var("FAKE_DBT_SHOW_ROWS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3);
    let limit: usize = value_of("--limit")
        .and_then(|l| l.parse().ok())
        .unwrap_or(5);
    let rows: Vec<serde_json::Value> = (1..=wanted.min(limit))
        .map(|n| serde_json::json!({"n": n, "sql": sql}))
        .collect();
    Some(format!(
        "12:00:00  Running with dbt=1.8.0\n{}\n",
        serde_json::json!({"node": "inline_query", "show": rows})
    ))
}
