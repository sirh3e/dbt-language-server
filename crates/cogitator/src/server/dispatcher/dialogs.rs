//! The dialogs on the dispatcher's side (see `server::dialogs`): the
//! `dbt.menu` command that opens them, the answers to their prompts, and
//! running the command a reader chose.

use cogitator_analysis::features;
use serde_json::{json, Value};

use super::Dispatcher;
use crate::server::dialogs::menu::{self, Answer, Here, Shown, Step};
use crate::server::dialogs::Pending;

impl Dispatcher {
    /// `dbt.menu {uri, position?}`: answers `null` at once and opens the
    /// menu's first prompt.
    pub(super) fn dialog_menu(
        &mut self,
        id: i64,
        uri: &str,
        position: Option<cogitator_lsp::Position>,
    ) {
        self.send(&cogitator_lsp::ResultResponse::new(id, Value::Null));
        let here = self.here(uri, position);
        self.link.ask(Shown::new(menu::root(&here)));
    }

    /// What the menu knows of where it was opened. No file I/O: the
    /// context and the open document hold all of it.
    fn here(&self, uri: &str, position: Option<cogitator_lsp::Position>) -> Here {
        let ctx = self.ctx_for(uri);
        let model = features::model_name_from_uri(uri)
            .filter(|m| uri.ends_with(".sql") && ctx.model_detail_map.contains_key(m));
        let ctes = match (&model, self.documents.get(uri)) {
            (Some(_), Some(doc)) => {
                let mut defs: Vec<(u32, String)> = doc
                    .parsed
                    .def_tokens
                    .iter()
                    .map(|(name, token)| (token.start, name.to_string()))
                    .collect();
                defs.sort();
                defs.into_iter().map(|(_, name)| name).collect()
            }
            _ => Vec::new(),
        };
        let mut tags: Vec<String> = ctx
            .model_detail_map
            .values()
            .flat_map(|m| m.tags.iter().cloned())
            .collect();
        tags.sort();
        tags.dedup();
        let mut selectors: Vec<String> = ctx
            .selectors
            .definitions
            .iter()
            .map(|d| d.name.clone())
            .collect();
        selectors.sort();
        selectors.dedup();
        Here {
            uri: uri.to_string(),
            position: position.map(|p| (p.line, p.character)),
            model,
            ctes,
            tags,
            selectors,
            jobs: self.jobs.list(),
        }
    }

    /// A response from the client to one of the server's requests: the
    /// answer to a dialog's prompt when it is one; every other response is
    /// fire-and-forget, as before.
    pub(super) fn client_response(&mut self, content: &[u8]) {
        #[derive(serde::Deserialize)]
        struct Response {
            id: Option<i64>,
            #[serde(default)]
            result: Option<Value>,
        }
        let Ok(Response {
            id: Some(id),
            result,
        }) = serde_json::from_slice::<Response>(content)
        else {
            return;
        };
        let Some(shown) = self.link.dialogs().answered(id) else {
            tracing::trace!(id, "client response ignored");
            return;
        };
        let title = result
            .as_ref()
            .and_then(|r| r.get("title"))
            .and_then(Value::as_str);
        match shown.answer(title) {
            Answer::Nothing => {}
            Answer::Page(next) => self.link.ask(next),
            Answer::Step(Step::Prompt(prompt)) => self.link.ask(Shown::new(prompt)),
            Answer::Step(Step::Say(text)) => {
                self.show_message(cogitator_lsp::message_type::INFO, text)
            }
            Answer::Step(Step::Run(run)) => self.dialog_run(run),
        }
    }

    /// Run the command a reader chose, through the same path a client's
    /// `workspace/executeCommand` takes; its response is presented by
    /// `server::dialogs::finish` instead of being sent.
    fn dialog_run(&mut self, run: menu::Run) {
        let uri = run
            .argument
            .get("uri")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let ctx = self.ctx_for(&uri);
        let root = ctx
            .project_root
            .clone()
            .unwrap_or_else(|| std::path::PathBuf::from(&self.root_path));
        let id = self.link.dialogs().internal_id();
        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "workspace/executeCommand",
            "params": { "command": run.command, "arguments": [run.argument] },
        });
        tracing::info!(command = run.command, "dialog runs a command");
        self.link.dialogs().expect(
            id,
            Pending {
                run,
                dir: root.join("target").join(".cogitator").join("dialogs"),
            },
        );
        let bytes = serde_json::to_vec(&request).expect("a request serializes");
        self.execute_command(&bytes);
    }
}
