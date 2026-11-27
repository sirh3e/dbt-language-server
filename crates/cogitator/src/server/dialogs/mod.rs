//! The server's dialogs, for a client with no command surface (Zed):
//! `initializationOptions.dialogs = true` makes the code-action menu offer
//! "dbt…", which opens [`menu::root`] as a `window/showMessageRequest`; the
//! answer (a response the dispatcher used to ignore) asks the next question
//! or runs the chosen `dbt.*` command through the dispatcher's own
//! `workspace/executeCommand` path, under an id of the server's own (they
//! count down from -1, so they never meet a client's). The command's
//! response never leaves the server: [`ClientLink::send`] hands it to
//! [`finish`], which opens it as a document, says it, or asks the next
//! question it makes possible.
//!
//! [`ClientLink::send`]: super::client_link::ClientLink::send

pub mod finds;
pub mod menu;
pub mod render;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
use std::sync::Mutex;

use serde::Serialize;
use serde_json::Value;

use menu::{Present, Run, Shown};

/// A command the dialogs are waiting on.
#[derive(Debug, Clone)]
pub struct Pending {
    pub run: Run,
    /// Where its document is written: `<project>/target/.cogitator/dialogs`.
    pub dir: PathBuf,
}

/// The prompts on screen and the commands the dialogs wait on, shared by
/// every clone of the dispatcher's [`ClientLink`](super::client_link::ClientLink).
#[derive(Debug)]
pub struct Dialogs {
    shown: Mutex<HashMap<i64, Shown>>,
    runs: Mutex<HashMap<i64, Pending>>,
    next: AtomicI64,
    /// `runs.len()`, read on every frame sent: zero costs nothing.
    awaited: AtomicUsize,
}

impl Default for Dialogs {
    fn default() -> Self {
        Dialogs {
            shown: Mutex::default(),
            runs: Mutex::default(),
            next: AtomicI64::new(-1),
            awaited: AtomicUsize::new(0),
        }
    }
}

impl Dialogs {
    /// An id for a command the dialogs run: negative, never a client's.
    pub fn internal_id(&self) -> i64 {
        self.next.fetch_sub(1, Ordering::Relaxed)
    }

    pub fn expect(&self, id: i64, pending: Pending) {
        let mut runs = self.runs.lock().expect("dialog runs lock");
        runs.insert(id, pending);
        self.awaited.store(runs.len(), Ordering::Relaxed);
    }

    pub fn awaiting(&self) -> bool {
        self.awaited.load(Ordering::Relaxed) > 0
    }

    pub fn take(&self, id: i64) -> Option<Pending> {
        let mut runs = self.runs.lock().expect("dialog runs lock");
        let pending = runs.remove(&id);
        self.awaited.store(runs.len(), Ordering::Relaxed);
        pending
    }

    pub fn remember(&self, id: i64, shown: Shown) {
        self.shown
            .lock()
            .expect("dialog prompts lock")
            .insert(id, shown);
    }

    /// The prompt a response answers, once.
    pub fn answered(&self, id: i64) -> Option<Shown> {
        self.shown.lock().expect("dialog prompts lock").remove(&id)
    }
}

/// `window/showMessageRequest`'s params.
#[derive(Debug, Serialize)]
pub struct ShowMessageRequestParams {
    #[serde(rename = "type")]
    pub message_type: i32,
    pub message: String,
    pub actions: Vec<MessageActionItem>,
}

#[derive(Debug, Serialize)]
pub struct MessageActionItem {
    pub title: String,
}

impl ShowMessageRequestParams {
    pub fn of(shown: &Shown) -> Self {
        ShowMessageRequestParams {
            message_type: cogitator_lsp::message_type::INFO,
            message: shown.message(),
            actions: shown
                .titles()
                .into_iter()
                .map(|title| MessageActionItem { title })
                .collect(),
        }
    }
}

/// `window/showDocument` to open outside the editor (the lineage site).
#[derive(Debug, Serialize)]
pub struct ShowExternal {
    pub uri: String,
    pub external: bool,
}

/// The code action that opens the dbt menu ("dbt…"), at the cursor.
pub fn menu_action(uri: &str, position: cogitator_lsp::Position) -> cogitator_lsp::CodeAction {
    let title = "dbt… (every dbt command)".to_string();
    cogitator_lsp::CodeAction::command_only(
        title.clone(),
        cogitator_lsp::Command {
            title,
            command: "dbt.menu".to_string(),
            arguments: vec![serde_json::json!({
                "uri": uri,
                "position": { "line": position.line, "character": position.character },
            })],
        },
    )
}

/// What a finished command leads to.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    Nothing,
    Say {
        error: bool,
        text: String,
    },
    /// Write `text` to `path`, then open it.
    Write {
        path: PathBuf,
        text: String,
    },
    Open {
        uri: String,
        selection: Option<cogitator_lsp::Range>,
    },
    OpenOutside(String),
    Ask(menu::Prompt),
}

/// The file a dialog writes a command's answer to, by what was chosen.
fn file_for(pending: &Pending, extension: &str) -> PathBuf {
    let command = pending.run.command.trim_start_matches("dbt.");
    let model = pending
        .run
        .argument
        .get("uri")
        .and_then(Value::as_str)
        .and_then(cogitator_analysis::features::model_name_from_uri);
    let name = match model {
        Some(model) => format!("{command}.{model}.{extension}"),
        None => format!("{command}.{extension}"),
    };
    pending.dir.join(name)
}

/// The project a dialog's documents are written in: `dir` is
/// `<project>/target/.cogitator/dialogs`.
fn project_of(pending: &Pending) -> Option<&std::path::Path> {
    pending.dir.ancestors().nth(3)
}

/// The facts a dialog adds to a command's answer before [`outcome`] reads
/// it, which need file I/O (so not in the pure menu): which profiles.yml
/// exist, for "Switch the profiles directory".
fn with_facts(pending: &Pending, mut response: Value) -> Value {
    if pending.run.present == Present::Choose(menu::Chooser::ProfilesDirs) {
        if let Some(result) = response.get_mut("result") {
            *result = finds::with_existing_profiles(result.take(), |p| p.is_file());
        }
    }
    response
}

/// What a command's response (`{id, result}` or `{id, error}`) leads to.
/// Pure: [`finish`] carries it out.
pub fn outcome(pending: &Pending, response: &Value) -> Outcome {
    let run = &pending.run;
    if let Some(error) = response.get("error") {
        let text = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("the command failed");
        return Outcome::Say {
            error: true,
            text: format!("{}: {text}", run.title),
        };
    }
    let result = response.get("result").unwrap_or(&Value::Null);
    let nothing = || Outcome::Say {
        error: false,
        text: format!("{}: nothing to show.", run.title),
    };
    match &run.present {
        Present::Itself => Outcome::Nothing,
        Present::Started => Outcome::Say {
            error: false,
            text: format!(
                "{}: started{}. Its outcome follows as a message; \
                 \"Cancel a dbt job\" in the dbt menu stops it.",
                run.title,
                result
                    .get("token")
                    .and_then(Value::as_u64)
                    .map(|t| format!(" (job {t})"))
                    .unwrap_or_default()
            ),
        },
        _ if result.is_null() => nothing(),
        Present::Document => Outcome::Write {
            path: file_for(pending, "md"),
            text: render::markdown(&run.title, result),
        },
        Present::Field { field, extension } => match result.get(*field).and_then(Value::as_str) {
            Some(text) => Outcome::Write {
                path: file_for(pending, extension),
                text: if text.ends_with('\n') {
                    text.to_string()
                } else {
                    format!("{text}\n")
                },
            },
            None => Outcome::Write {
                path: file_for(pending, "md"),
                text: render::markdown(&run.title, result),
            },
        },
        Present::Matches => Outcome::Write {
            path: file_for(pending, "md"),
            text: finds::markdown(&run.title, result, project_of(pending)),
        },
        Present::Location => match result.get("uri").and_then(Value::as_str) {
            Some(uri) => Outcome::Open {
                uri: uri.to_string(),
                selection: result
                    .get("range")
                    .and_then(|r| serde_json::from_value(r.clone()).ok()),
            },
            None => nothing(),
        },
        Present::Site => match result.get("uri").and_then(Value::as_str) {
            Some(uri) => Outcome::OpenOutside(uri.to_string()),
            None => nothing(),
        },
        Present::Choose(chooser) => {
            let uri = run
                .argument
                .get("uri")
                .and_then(Value::as_str)
                .unwrap_or_default();
            match menu::choose(*chooser, uri, result) {
                Ok(prompt) => Outcome::Ask(prompt),
                Err(text) => Outcome::Say { error: false, text },
            }
        }
    }
}

/// Carry out a finished command's [`outcome`]. Writing the document is file
/// I/O, so it runs on a blocking thread, off whatever task sent the
/// response.
pub fn finish(link: super::client_link::ClientLink, pending: Pending, response: Value) {
    let work = move || match outcome(&pending, &with_facts(&pending, response)) {
        Outcome::Nothing => {}
        Outcome::Say { error, text } => link.show_message(
            if error {
                cogitator_lsp::message_type::ERROR
            } else {
                cogitator_lsp::message_type::INFO
            },
            text,
        ),
        Outcome::Write { path, text } => {
            let written = path
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| std::fs::write(&path, text));
            match written {
                Ok(()) => link.show_document(cogitator_lsp::fs_path_to_uri(&path), None),
                Err(err) => link.show_message(
                    cogitator_lsp::message_type::ERROR,
                    format!(
                        "{}: could not write {}: {err}",
                        pending.run.title,
                        path.display()
                    ),
                ),
            }
        }
        Outcome::Open { uri, selection } => link.show_document(uri, selection),
        Outcome::OpenOutside(uri) => link.send_request(
            "window/showDocument",
            ShowExternal {
                uri,
                external: true,
            },
        ),
        Outcome::Ask(prompt) => link.ask(Shown::new(prompt)),
    };
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            handle.spawn_blocking(work);
        }
        Err(_) => work(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    fn pending(present: Present, command: &'static str) -> Pending {
        Pending {
            run: Run {
                title: "Its card".into(),
                command,
                argument: json!({ "uri": "file:///p/models/orders.sql" })
                    .as_object()
                    .cloned()
                    .unwrap(),
                present,
            },
            dir: PathBuf::from("/p/target/.cogitator/dialogs"),
        }
    }

    #[test]
    fn an_error_is_said() {
        let out = outcome(
            &pending(Present::Document, "dbt.modelInfo"),
            &json!({ "id": -1, "error": { "code": -32602, "message": "no model" } }),
        );
        assert_eq!(
            out,
            Outcome::Say {
                error: true,
                text: "Its card: no model".into()
            }
        );
    }

    #[test]
    fn a_document_is_written_by_command_and_model() {
        let out = outcome(
            &pending(Present::Document, "dbt.modelInfo"),
            &json!({ "id": -1, "result": { "model": "orders" } }),
        );
        assert_eq!(
            out,
            Outcome::Write {
                path: PathBuf::from("/p/target/.cogitator/dialogs/modelInfo.orders.md"),
                text: "# Its card\n\n- **model**: orders\n".into()
            }
        );
    }

    #[test]
    fn a_field_is_its_own_file_and_null_says_nothing_to_show() {
        let field = Present::Field {
            field: "sql",
            extension: "sql",
        };
        assert_eq!(
            outcome(
                &pending(field.clone(), "dbt.expandJinja"),
                &json!({ "id": -1, "result": { "sql": "select 1" } })
            ),
            Outcome::Write {
                path: PathBuf::from("/p/target/.cogitator/dialogs/expandJinja.orders.sql"),
                text: "select 1\n".into()
            }
        );
        assert_eq!(
            outcome(
                &pending(field, "dbt.expandJinja"),
                &json!({ "id": -1, "result": null })
            ),
            Outcome::Say {
                error: false,
                text: "Its card: nothing to show.".into()
            }
        );
    }

    #[test]
    fn a_job_says_it_started_and_a_location_opens() {
        assert!(matches!(
            outcome(&pending(Present::Started, "dbt.run"), &json!({ "id": -1, "result": { "token": 4 } })),
            Outcome::Say { error: false, text } if text.contains("(job 4)")
        ));
        let range =
            json!({ "start": { "line": 2, "character": 0 }, "end": { "line": 2, "character": 5 } });
        let out = outcome(
            &pending(Present::Location, "dbt.goToSchema"),
            &json!({ "id": -1, "result": { "uri": "file:///p/models/schema.yml", "range": range } }),
        );
        let Outcome::Open { uri, selection } = out else {
            panic!("an open");
        };
        assert_eq!(uri, "file:///p/models/schema.yml");
        assert_eq!(selection.unwrap().start.line, 2);
    }

    #[test]
    fn ids_count_down_and_a_run_is_taken_once() {
        let dialogs = Dialogs::default();
        let (a, b) = (dialogs.internal_id(), dialogs.internal_id());
        assert_eq!((a, b), (-1, -2));
        assert!(!dialogs.awaiting());
        dialogs.expect(a, pending(Present::Itself, "dbt.status"));
        assert!(dialogs.awaiting());
        assert!(dialogs.take(a).is_some());
        assert!(dialogs.take(a).is_none());
        assert!(!dialogs.awaiting());
    }
}
