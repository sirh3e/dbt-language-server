//! The system layer (05 §7.1): drives the REAL editor suites — headless
//! Neovim and batch Emacs — against the exact server binary cargo just
//! built, parsing their ok/FAIL lines into one deterministic verdict.
//!
//! Opt-in by design: `#[ignore]`d under plain `cargo test`; run them
//! with `just system-test`. A missing editor SKIPS with a message —
//! unless COGITATOR_REQUIRE_EDITORS=1 (CI), where absence FAILS, so
//! the pipeline can never go silently green.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

/// The whole-suite deadline (a full editor suite, debug server):
/// 600 s default, COGITATOR_SYSTEM_DEADLINE_MS overrides.
fn suite_deadline() -> Duration {
    std::env::var("COGITATOR_SYSTEM_DEADLINE_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .map(Duration::from_millis)
        .unwrap_or(Duration::from_secs(600))
}

fn find_in_path(name: &str) -> Option<PathBuf> {
    let candidates: Vec<String> = if cfg!(windows) {
        vec![
            name.to_string(),
            format!("{name}.exe"),
            format!("{name}.cmd"),
        ]
    } else {
        vec![name.to_string()]
    };
    std::env::split_paths(&std::env::var_os("PATH")?)
        .flat_map(|dir| {
            candidates
                .iter()
                .map(move |c| dir.join(c))
                .collect::<Vec<_>>()
        })
        .find(|p| p.is_file())
}

/// `true` when the suite may run; otherwise prints the skip (or
/// panics under COGITATOR_REQUIRE_EDITORS=1).
fn editor_available(name: &str, hint: &str) -> bool {
    if find_in_path(name).is_some() {
        return true;
    }
    let message = format!("{name} not found on PATH ({hint})");
    if std::env::var("COGITATOR_REQUIRE_EDITORS").as_deref() == Ok("1") {
        panic!("editor suite required but {message}");
    }
    println!("SKIPPED: {message}");
    false
}

struct SuiteReport {
    ok: Vec<String>,
    failed: Vec<String>,
    all_lines: Vec<String>,
    exit: Option<i32>,
    timed_out: bool,
}

/// Run one suite process to completion (or the deadline), relaying and
/// collecting its output.
fn run_suite(label: &str, mut command: Command) -> SuiteReport {
    command
        .env("COGITATOR_BIN", env!("CARGO_BIN_EXE_cogitator"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().unwrap_or_else(|e| panic!("{label}: {e}"));

    let (tx, rx) = mpsc::channel::<String>();
    let out = child.stdout.take().unwrap();
    let err = child.stderr.take().unwrap();
    for stream in [
        Box::new(out) as Box<dyn std::io::Read + Send>,
        Box::new(err),
    ] {
        let tx = tx.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stream).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    return;
                }
            }
        });
    }
    drop(tx);

    let due = Instant::now() + suite_deadline();
    let mut report = SuiteReport {
        ok: Vec::new(),
        failed: Vec::new(),
        all_lines: Vec::new(),
        exit: None,
        timed_out: false,
    };
    loop {
        match rx.recv_timeout(due.saturating_duration_since(Instant::now())) {
            Ok(line) => {
                println!("[{label}] {line}");
                // the suites' contract: one "ok   <step>" / "FAIL <step>"
                // line per step (editors/*/tests)
                if let Some(step) = line.strip_prefix("ok   ") {
                    report.ok.push(step.to_string());
                } else if let Some(step) = line.strip_prefix("FAIL ") {
                    report.failed.push(step.to_string());
                }
                report.all_lines.push(line);
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break, // output over
            Err(mpsc::RecvTimeoutError::Timeout) => {
                report.timed_out = true;
                let _ = child.kill();
                break;
            }
        }
    }
    report.exit = child.wait().ok().and_then(|s| s.code());
    report
}

/// One verdict per suite: every FAIL named, nonzero/timeout fatal,
/// and zero parsed steps is itself a failure (a crashed suite must
/// never read as green).
fn assert_green(label: &str, report: SuiteReport) {
    let summary = format!(
        "{label}: {} ok, {} failed, exit {:?}{}",
        report.ok.len(),
        report.failed.len(),
        report.exit,
        if report.timed_out { ", TIMED OUT" } else { "" },
    );
    println!("{summary}");
    if report.timed_out
        || report.exit != Some(0)
        || !report.failed.is_empty()
        || report.ok.is_empty()
    {
        panic!(
            "{summary}\n--- failed steps ---\n{}\n--- full output ---\n{}",
            report.failed.join("\n"),
            report.all_lines.join("\n"),
        );
    }
}

/// A bounded setup command (npm install, tsc): all output relayed,
/// any failure fatal — setup is never part of the step report.
fn run_setup(label: &str, mut command: Command) {
    command.env("COGITATOR_BIN", env!("CARGO_BIN_EXE_cogitator"));
    let output = command.output().unwrap_or_else(|e| panic!("{label}: {e}"));
    if !output.status.success() {
        panic!(
            "{label} failed ({:?})\n--- stdout ---\n{}\n--- stderr ---\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }
}

fn npm() -> &'static str {
    if cfg!(windows) {
        "npm.cmd"
    } else {
        "npm"
    }
}

#[test]
#[ignore = "editor system suite — run via `just system-test`"]
fn nvim_suite() {
    if !editor_available("nvim", "install Neovim >= 0.11") {
        return;
    }
    let mut command = Command::new("nvim");
    command
        .current_dir(repo_root())
        .arg("--headless")
        .arg("-l")
        .arg(repo_root().join("editors/nvim/tests/run.lua"));
    assert_green("nvim", run_suite("nvim", command));
}

#[test]
#[ignore = "editor system suite — run via `just system-test`"]
fn emacs_suite() {
    if !editor_available("emacs", "install Emacs >= 29") {
        return;
    }
    let mut command = Command::new("emacs");
    command
        .current_dir(repo_root())
        .arg("-Q")
        .arg("--batch")
        .arg("-L")
        .arg(repo_root().join("editors/emacs"))
        .arg("-l")
        .arg(repo_root().join("editors/emacs/tests/run.el"));
    assert_green("emacs", run_suite("emacs", command));
}

/// The extension-host suite: @vscode/test-electron downloads a cached
/// VS Code build (editors/vscode/.vscode-test/) and runs
/// src/test/suite inside it — same ok/FAIL contract. Linux CI wraps
/// the whole orchestrator in xvfb-run.
#[test]
#[ignore = "editor system suite — run via `just system-test`"]
fn vscode_suite() {
    if !editor_available("npm", "install Node.js >= 20") {
        return;
    }
    let ext = repo_root().join("editors/vscode");
    let mut install = Command::new(npm());
    install
        .current_dir(&ext)
        .args(["install", "--no-audit", "--no-fund"]);
    run_setup("vscode: npm install", install);
    let mut compile = Command::new(npm());
    compile.current_dir(&ext).args(["run", "compile"]);
    run_setup("vscode: npm run compile", compile);

    let mut command = Command::new("node");
    command.current_dir(&ext).arg("./out/test/runTest.js");
    assert_green("vscode", run_suite("vscode", command));
}
