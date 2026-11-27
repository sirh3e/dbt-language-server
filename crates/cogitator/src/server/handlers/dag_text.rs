//! `dbt.dagText {uri?, model?, depth?, show?}`: the project DAG — or, with a
//! `model` or a `depth`, a model's neighbourhood — drawn as text, the
//! picture `:DbtDag` draws in Neovim. With `show: true` the text is written
//! under `target/.cogitator/dag/` and opened through showDocument, so a
//! client that can only open documents (Zed) shows it as it is.

use cogitator_analysis::context::DbtContext;
use cogitator_analysis::features::{self, dag_text};
use cogitator_error::guidance::Guidance;
use cogitator_lsp::{error_codes, ShowDocumentParams};

use super::super::reply::{Effect, Reply};

/// Hops each way when a model is asked for without a depth (Neovim's
/// `dag.focus_depth`).
pub const DEFAULT_DEPTH: usize = 2;
/// The most hops a neighbourhood takes.
pub const MAX_DEPTH: usize = 10;

/// What was asked.
#[derive(Debug, Clone, Default)]
pub struct DagAsk {
    pub uri: String,
    pub model: Option<String>,
    pub depth: Option<usize>,
    pub show: bool,
}

#[tracing::instrument(skip_all, fields(model = ?ask.model, depth = ?ask.depth, show = ask.show))]
pub fn dag_text(ctx: &DbtContext, ask: &DagAsk) -> Reply {
    let dag = dag_text::Dag::of(&features::project_graph_json(ctx));
    let model = match (&ask.model, ask.depth) {
        (Some(model), _) => Some(model.clone()),
        (None, Some(_)) => match features::model_name_from_uri(&ask.uri) {
            Some(model) => Some(model),
            None => {
                return invalid(
                    Guidance::new("dbt.dagText was given a depth but no model.")
                        .at(&ask.uri)
                        .fix("pass `model`, or the uri of a model's .sql file."),
                )
            }
        },
        (None, None) => None,
    };
    let answer = match &model {
        None => dag_text::render(
            &dag,
            &dag_text::Subject::Project,
            &dag_text::Expanded::None,
            dag_text::MAX_NODES,
        ),
        Some(model) => {
            let id = format!("model:{model}");
            if !dag.nodes.iter().any(|n| n.id == id) {
                return invalid(
                    Guidance::new(format!("'{model}' is not a model of the project."))
                        .fix("open a model's .sql file, or pass the name of one."),
                );
            }
            let depth = ask.depth.unwrap_or(DEFAULT_DEPTH).clamp(1, MAX_DEPTH);
            dag_text::render(
                &dag_text::focus(&dag, &id, depth),
                &dag_text::Subject::Around { model, depth },
                &dag_text::Expanded::None,
                dag_text::MAX_NODES,
            )
        }
    };
    if !ask.show {
        return Reply::result(&answer);
    }
    match write_text(ctx, model.as_deref(), &answer.text) {
        Some(uri) => Reply::result(&answer).with(Effect::ShowDocument(ShowDocumentParams {
            uri,
            take_focus: true,
            selection: None,
        })),
        None => Reply::result(&answer),
    }
}

fn write_text(ctx: &DbtContext, model: Option<&str>, text: &str) -> Option<String> {
    let root = ctx.project_root.as_deref()?;
    let dir = root.join("target/.cogitator/dag");
    let path = dir.join(dag_text::file_name(model));
    if let Err(err) = std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&path, text)) {
        tracing::warn!(%err, "could not write the DAG text; returning it only");
        return None;
    }
    Some(cogitator_lsp::fs_path_to_uri(&path))
}

fn invalid(guidance: Guidance) -> Reply {
    Reply::error(error_codes::INVALID_PARAMS, guidance.to_message())
}
