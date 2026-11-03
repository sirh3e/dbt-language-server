//! dbt Fusion compile → diagnostics (specs/rust-port/04 §2), async
//! (05 §6.2 — fixes Q9): child process with line-streamed JSON logs,
//! publishing through the writer channel after every accepted diagnostic
//! and once more (possibly empty) at the end.

use std::path::PathBuf;
use std::sync::LazyLock;

use cogitator_lsp::{diagnostic_severity, publish_diagnostics, rpc, Diagnostic, Position, Range};
use regex::Regex;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;

/// `<fusion> --version` must report a dbt-fusion binary (04 §1.3).
#[tracing::instrument(skip_all)]
pub async fn validate_fusion(path: &str) -> bool {
    match tokio::process::Command::new(path)
        .arg("--version")
        .output()
        .await
    {
        Ok(output) => {
            let valid = output.stdout.starts_with(b"dbt-fusion");
            if !valid {
                tracing::warn!(%path, "not a dbt-fusion binary");
            }
            valid
        }
        Err(err) => {
            tracing::warn!(%path, %err, "fusion validation failed");
            false
        }
    }
}

pub struct FusionJob {
    pub fusion_path: String,
    pub project_name: String,
    pub uri: String,
    pub write_tx: mpsc::UnboundedSender<Vec<u8>>,
}

impl FusionJob {
    #[tracing::instrument(skip_all, fields(uri = %self.uri, diagnostics = tracing::field::Empty))]
    pub async fn run(self) {
        let Some(artifacts) = fusion_artifact_path(&self.project_name) else {
            tracing::warn!("cannot create fusion artifact dirs");
            return;
        };
        let selector = model_selector_from_uri(&self.uri);
        let mut cmd = tokio::process::Command::new(&self.fusion_path);
        cmd.args([
            "compile",
            "-q",
            "--static-analysis",
            "on",
            "--log-format",
            "json",
            "--no-write-json",
            "--target-path",
        ])
        .arg(artifacts.join("target"))
        .arg("--log-path")
        .arg(artifacts.join("log"))
        .arg("--select")
        .arg(&selector)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
        tracing::debug!(?selector, "running fusion compile");

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(err) => {
                tracing::warn!(%err, "failed to spawn fusion");
                return;
            }
        };

        let mut stdout = child.stdout.take().map(|s| BufReader::new(s).lines());
        let mut stderr = child.stderr.take().map(|s| BufReader::new(s).lines());

        let mut diagnostics: Vec<Diagnostic> = Vec::new();
        let publish = |diags: &[Diagnostic]| {
            let note = publish_diagnostics(self.uri.clone(), diags.to_vec());
            if let Ok(frame) = rpc::encode_frame_vec(&note) {
                let _ = self.write_tx.send(frame);
            }
        };

        loop {
            let line = tokio::select! {
                line = next_line(&mut stdout) => line,
                line = next_line(&mut stderr) => line,
                else => break,
            };
            let Some(line) = line else { break };
            tracing::trace!(%line, "fusion log");
            let Ok(entry) = serde_json::from_str::<FusionLog>(&line) else {
                continue;
            };
            let (diag_uri, diagnostic) = parse_compile_log(&entry);
            if !diag_uri.is_empty() && self.uri.contains(&diag_uri) {
                diagnostics.push(diagnostic);
                publish(&diagnostics);
            }
        }

        if let Err(err) = child.wait().await {
            tracing::warn!(%err, "fusion compile failed");
        }
        tracing::Span::current().record("diagnostics", diagnostics.len());
        publish(&diagnostics); // final (possibly empty) publish clears stale diagnostics
    }
}

async fn next_line<R: AsyncBufReadExt + Unpin>(
    lines: &mut Option<tokio::io::Lines<R>>,
) -> Option<String> {
    match lines {
        Some(reader) => match reader.next_line().await {
            Ok(Some(line)) => Some(line),
            _ => {
                *lines = None;
                None
            }
        },
        None => std::future::pending().await,
    }
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(default)]
pub struct FusionLog {
    pub data: FusionData,
    pub info: FusionInfo,
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(default)]
pub struct FusionData {
    pub version: String,
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(default)]
pub struct FusionInfo {
    pub code: String,
    pub level: String,
    pub msg: String,
}

/// 04 §2.5: severity mapping + message/location parsing.
pub fn parse_compile_log(log: &FusionLog) -> (String, Diagnostic) {
    let severity = match log.info.level.as_str() {
        "info" => diagnostic_severity::INFO,
        "warning" => diagnostic_severity::WARNING,
        "error" => diagnostic_severity::ERROR,
        _ => diagnostic_severity::HINT,
    };
    let (uri, message, range) =
        parse_fusion_log_msg(&log.info.msg, &log.info.code, &log.info.level);
    (
        uri,
        Diagnostic {
            range,
            message,
            severity,
            code: log.info.code.clone(),
            source: format!("dbt Fusion {}", log.data.version),
        },
    )
}

static COLOR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\\u001b\[[0-9;]*m|\x1b\[[0-9;]*m)").unwrap());
static MODEL_PATH_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\(in .*:\d*\))").unwrap());

/// 04 §2.5 steps 1–4. Returns ("", …) when no location is present — such
/// diagnostics never match any uri (Go's "NO URI FOUND").
pub fn parse_fusion_log_msg(msg: &str, code: &str, level: &str) -> (String, String, Range) {
    let clean = COLOR_RE.replace_all(msg, "");
    let Some((head, location_part)) = clean.split_once(" --> ") else {
        return (String::new(), clean.into_owned(), Range::default());
    };

    let mut message = head.trim().replace("\\n", "");
    let level_re = Regex::new(&format!("^{}: ", regex::escape(level))).unwrap();
    let code_re = Regex::new(&format!("^{c}: |^dbt{c}: ", c = regex::escape(code))).unwrap();
    message = level_re.replace(&message, "").into_owned();
    message = code_re.replace(&message, "").into_owned();
    message = MODEL_PATH_RE.replace_all(&message, "").trim().to_string();

    let file_part = location_part.trim().split(' ').next().unwrap_or_default();
    let parts: Vec<&str> = file_part.split(':').collect();
    if parts.len() < 3 {
        return (String::new(), message, Range::default());
    }
    let line = parts[1]
        .parse::<u32>()
        .map(|l| l.saturating_sub(1))
        .unwrap_or(0);
    let character = parts[2]
        .parse::<u32>()
        .map(|c| c.saturating_sub(1))
        .unwrap_or(0);
    let position = Position { line, character };
    (parts[0].to_string(), message, Range::zero_width(position))
}

/// 04 §2.1: path after the last "models" → dotted selector; else "*".
/// Decodes the URI first so percent-escapes and Windows drive prefixes
/// never leak into the selector.
pub fn model_selector_from_uri(uri: &str) -> String {
    let path = cogitator_lsp::uri_to_fs_path(uri)
        .unwrap_or_else(|| uri.to_string())
        .replace('\\', "/");
    match path.rfind("models") {
        None => "*".to_string(),
        Some(idx) => path[idx + "models".len()..]
            .trim_start_matches('/')
            .trim_end_matches(".sql")
            .replace('/', "."),
    }
}

/// 04 §2.2: `~/.dbt/dbt-language-server/fusion-artifacts/<project>` with
/// log/ and target/ subdirs.
fn fusion_artifact_path(project_name: &str) -> Option<PathBuf> {
    let base = cogitator_analysis::util::home_dir()?
        .join(".dbt")
        .join("dbt-language-server")
        .join("fusion-artifacts")
        .join(project_name);
    std::fs::create_dir_all(base.join("log")).ok()?;
    std::fs::create_dir_all(base.join("target")).ok()?;
    Some(base)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn selector_from_uri() {
        assert_eq!(
            model_selector_from_uri("file:///w/models/staging/stg_orders.sql"),
            "staging.stg_orders"
        );
        assert_eq!(model_selector_from_uri("file:///w/macros/util.sql"), "*");
    }

    #[test]
    fn log_msg_parsing() {
        let (uri, msg, range) = parse_fusion_log_msg(
            "error: dbt0001: something broke (in models/a.sql:3) --> models/a.sql:3:7 extra",
            "0001",
            "error",
        );
        assert_eq!(uri, "models/a.sql");
        assert_eq!(msg, "something broke");
        assert_eq!(range.start.line, 2);
        assert_eq!(range.start.character, 6);
    }

    #[test]
    fn no_location_yields_unmatchable_uri() {
        let (uri, _, range) = parse_fusion_log_msg("plain message", "c", "info");
        assert_eq!(uri, "");
        assert_eq!(range, Range::default());
    }
}
