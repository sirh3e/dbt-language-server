//! Workspace commands behind the code lenses: open-schema and show-lineage
//! entry points, and per-model `dbt run/test/compile` execution. Every
//! failure path reports through window/showMessage with a reason and a
//! suggested solution (user-feedback requirement).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use cogitator_lsp::{message_type, rpc, Range, ServerRequest, ShowDocumentParams};
use serde::Serialize;
use tokio::sync::mpsc;

/// Writer-channel handle for background tasks (the dispatcher owns its own).
#[derive(Clone)]
pub struct ClientLink {
    write_tx: mpsc::UnboundedSender<Vec<u8>>,
    ids: Arc<AtomicI64>,
}

impl ClientLink {
    pub fn new(write_tx: mpsc::UnboundedSender<Vec<u8>>, ids: Arc<AtomicI64>) -> Self {
        ClientLink { write_tx, ids }
    }

    pub fn show_message(&self, message_type: i32, message: impl Into<String>) {
        let note = cogitator_lsp::show_message(message_type, message);
        self.send(&note);
    }

    pub fn show_document(&self, uri: String, selection: Option<Range>) {
        let id = self.ids.fetch_add(1, Ordering::Relaxed);
        let request = ServerRequest {
            jsonrpc: "2.0",
            id,
            method: "window/showDocument",
            params: ShowDocumentParams {
                uri,
                take_focus: true,
                selection,
            },
        };
        self.send(&request);
    }

    fn send<T: Serialize>(&self, msg: &T) {
        if let Ok(frame) = rpc::encode_frame_vec(msg) {
            let _ = self.write_tx.send(frame);
        }
    }
}

pub struct DbtInvocation {
    pub kind: &'static str, // run | test | compile
    pub binary: String,
    pub project_root: PathBuf,
    pub project_name: String,
    pub selector: String,
    pub model_path: PathBuf,
    /// Active target (dbt.setTarget / --target); appended as `--target`.
    pub target: Option<String>,
}

/// Spawn `dbt <kind> --select <selector>` for one model and report the
/// outcome; a successful compile opens the compiled SQL in the editor.
pub fn spawn_dbt(link: ClientLink, invocation: DbtInvocation) {
    tokio::spawn(async move {
        let DbtInvocation {
            kind,
            binary,
            project_root,
            project_name,
            selector,
            model_path,
            target,
        } = invocation;
        let target_suffix = target
            .as_deref()
            .map(|t| format!(" (target {t})"))
            .unwrap_or_default();
        link.show_message(
            message_type::INFO,
            format!("dbt {kind} started for '{selector}'{target_suffix}…"),
        );
        let mut command = tokio::process::Command::new(&binary);
        command
            .arg(kind)
            .args(["--select", &selector])
            .current_dir(&project_root);
        if let Some(t) = &target {
            command.args(["--target", t]);
        }
        let result = command.output().await;

        let output = match result {
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                link.show_message(
                    message_type::ERROR,
                    format!(
                        "Failed to launch '{binary}'.\n\
                         Reason: the executable was not found on PATH.\n\
                         Solution: install dbt, or start cogitator with \
                         --fusion <path-to-dbt-binary>."
                    ),
                );
                return;
            }
            Err(err) => {
                link.show_message(
                    message_type::ERROR,
                    format!(
                        "Failed to launch '{binary}': {err}.\n\
                         Solution: check that the binary is executable and \
                         the project directory is readable."
                    ),
                );
                return;
            }
            Ok(output) => output,
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let detail = tail(
                if stderr.trim().is_empty() {
                    &stdout
                } else {
                    &stderr
                },
                400,
            );
            let retry = match &target {
                Some(t) => format!("{binary} {kind} --select {selector} --target {t}"),
                None => format!("{binary} {kind} --select {selector}"),
            };
            link.show_message(
                message_type::ERROR,
                format!(
                    "dbt {kind} failed for '{selector}' (exit {}).\n\
                     {detail}\n\
                     Solution: run `{retry}` in {} for the full output.",
                    output.status.code().unwrap_or(-1),
                    project_root.display(),
                ),
            );
            return;
        }

        link.show_message(
            message_type::INFO,
            format!("dbt {kind} succeeded for '{selector}'."),
        );

        if kind == "compile" {
            open_compiled(&link, &project_root, &project_name, &model_path);
        }
    });
}

/// After a successful compile: locate target/compiled/<project>/<model path>
/// and open it, or explain where we looked.
fn open_compiled(link: &ClientLink, root: &Path, project: &str, model_path: &Path) {
    let relative = model_path.strip_prefix(root).unwrap_or(model_path);
    let compiled = root
        .join("target")
        .join("compiled")
        .join(project)
        .join(relative);
    if compiled.is_file() {
        link.show_document(cogitator_lsp::fs_path_to_uri(&compiled), None);
    } else {
        link.show_message(
            message_type::WARNING,
            format!(
                "Compiled SQL not found at {}.\n\
                 Reason: this dbt version or your target-path setting writes \
                 compiled output elsewhere.\n\
                 Solution: check `target-path` in dbt_project.yml and look \
                 under <target-path>/compiled/.",
                compiled.display()
            ),
        );
    }
}

fn tail(text: &str, max: usize) -> String {
    let trimmed = text.trim();
    if trimmed.len() <= max {
        return trimmed.to_string();
    }
    let start = trimmed.len() - max;
    let start = (start..trimmed.len())
        .find(|&i| trimmed.is_char_boundary(i))
        .unwrap_or(start);
    format!("…{}", &trimmed[start..])
}
