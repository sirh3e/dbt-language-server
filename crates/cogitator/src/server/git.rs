//! A thin git shell for dbt.diffAt: `git show <rev>:<path>` and
//! `git log` through `tokio::process`, under a timeout, from an
//! offloaded (blocking) task. Every failure is `Guidance`: git missing,
//! not a repository, an unknown revision, a path the revision lacks.

use std::path::{Path, PathBuf};
use std::time::Duration;

use cogitator_error::guidance::Guidance;

/// No git call takes longer than this; a hung credential prompt or a
/// huge repository answers with guidance instead of a stuck request.
const TIMEOUT: Duration = Duration::from_secs(10);

/// The commit a revision names, with its subject.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Commit {
    pub sha: String,
    pub subject: String,
}

/// The repository's top level containing `dir`.
pub fn toplevel(dir: &Path) -> Result<PathBuf, Guidance> {
    let out = git(dir, &["rev-parse", "--show-toplevel"])?;
    Ok(PathBuf::from(out.trim()))
}

/// `git show <rev>:<path>` of a file (any path inside the repository).
pub fn show(file: &Path, rev: &str) -> Result<(String, Commit), Guidance> {
    let dir = file.parent().unwrap_or(Path::new("."));
    let top = toplevel(dir)?;
    let canonical = file.canonicalize().unwrap_or_else(|_| file.to_path_buf());
    let top_canonical = top.canonicalize().unwrap_or(top.clone());
    let relative = canonical
        .strip_prefix(&top_canonical)
        .map_err(|_| {
            Guidance::new("the file is outside its git repository.")
                .at(file.display().to_string())
                .fix("open the model from inside the repository.")
        })?
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/");
    let commit = resolve(&top, rev)?;
    let text = git(&top, &["show", &format!("{}:{relative}", commit.sha)]).map_err(|_| {
        Guidance::new(format!("'{relative}' does not exist at {rev}."))
            .because(format!(
                "{} ({}) has no such file.",
                short(&commit.sha),
                commit.subject
            ))
            .fix("pick an older revision, or the file was added since.")
    })?;
    Ok((text, commit))
}

/// The commit `rev` names (`HEAD`, `main~3`, a sha, a tag).
pub fn resolve(top: &Path, rev: &str) -> Result<Commit, Guidance> {
    if rev.starts_with('-') || rev.contains(['\n', '\0']) {
        return Err(Guidance::new(format!("'{rev}' is not a revision."))
            .fix("pass HEAD, a branch, a tag or a sha."));
    }
    let out = git(top, &["log", "-1", "--format=%H%x09%s", rev, "--"]).map_err(|g| {
        Guidance::new(format!("'{rev}' is not a revision of this repository."))
            .because(g.to_message())
            .fix("pass HEAD, a branch, a tag or a sha (git log shows them).")
    })?;
    let line = out.lines().next().unwrap_or_default();
    let (sha, subject) = line.split_once('\t').unwrap_or((line, ""));
    if sha.is_empty() {
        return Err(Guidance::new(format!("'{rev}' names no commit."))
            .fix("the repository may have no commits yet."));
    }
    Ok(Commit {
        sha: sha.to_string(),
        subject: subject.to_string(),
    })
}

fn short(sha: &str) -> &str {
    &sha[..sha.len().min(10)]
}

fn git(dir: &Path, args: &[&str]) -> Result<String, Guidance> {
    let handle = tokio::runtime::Handle::try_current().map_err(|_| {
        Guidance::new("git ran outside the server runtime.")
            .fix("this is a cogitator bug; report it.")
    })?;
    let mut command = tokio::process::Command::new("git");
    command
        .args(["-c", "core.quotepath=off"])
        .args(args)
        .current_dir(dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);
    let output = handle
        .block_on(async { tokio::time::timeout(TIMEOUT, command.output()).await })
        .map_err(|_| {
            Guidance::new(format!(
                "git {} took longer than {} s.",
                args[0],
                TIMEOUT.as_secs()
            ))
            .at(dir.display().to_string())
            .fix("check the repository (a lock, a huge history), then retry.")
        })?
        .map_err(|err| {
            Guidance::new("git could not be started.")
                .because(format!("{err}."))
                .fix("install git and put it on the PATH of the editor.")
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let guidance = if stderr.contains("not a git repository") {
            Guidance::new("the project is not in a git repository.")
                .at(dir.display().to_string())
                .fix("dbt.diffAt compares against git revisions; use rev: \"last-run\" instead.")
        } else {
            Guidance::new(format!("git {} failed.", args[0])).because(stderr.trim().to_string())
        };
        return Err(guidance);
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
