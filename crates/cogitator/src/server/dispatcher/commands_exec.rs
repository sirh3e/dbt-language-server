//! workspace/executeCommand: the dbt.* roster router and the per-
//! command shells (dbt runs, profiles switches, reports).

use std::sync::atomic::Ordering;

use cogitator_analysis::features;

use cogitator_lsp::{error_codes, Envelope, ErrorResponse, ExecuteCommandParams, ResultResponse};

use super::{arg_position, Dispatcher};

impl Dispatcher {
    pub(super) fn execute_command(&mut self, content: &[u8]) {
        let Ok(req) = serde_json::from_slice::<Envelope<ExecuteCommandParams>>(content) else {
            self.invalid_params(content, "workspace/executeCommand");
            return;
        };
        let Some(params) = req.params else {
            self.invalid_params(content, "workspace/executeCommand");
            return;
        };
        // a malformed argument used to hang the client on a silent drop
        let Some(arg) = params.arguments.first().and_then(|a| a.as_object()) else {
            let guidance = cogitator_error::guidance::Guidance::new(format!(
                "'{}' got no usable argument.",
                params.command
            ))
            .because("every dbt.* command takes exactly one argument OBJECT.")
            .fix("send arguments: [{\"uri\": \"file://…\", …}] (spec §1.6).");
            self.send(&ErrorResponse::new(
                req.id,
                error_codes::INVALID_PARAMS,
                guidance.to_message(),
            ));
            return;
        };
        let uri = arg
            .get("uri")
            .and_then(|u| u.as_str())
            .unwrap_or_default()
            .to_string();
        match params.command.as_ref() {
            "dbt.columnLineage" => {
                self.column_lineage(req.id, &uri);
                return;
            }
            "dbt.openSchema" => {
                self.open_schema(req.id, &uri);
                return;
            }
            "dbt.showLineage" => {
                self.show_lineage(req.id, &uri);
                return;
            }
            "dbt.sourceFreshness" => {
                self.run_dbt(req.id, "source freshness", &uri);
                return;
            }
            "dbt.splitPlan" => {
                self.split_plan(req.id, uri);
                return;
            }
            "dbt.incrementality" => {
                self.incrementality(req.id);
                return;
            }
            "dbt.deps" => {
                self.run_dbt(req.id, "deps", &uri);
                return;
            }
            "dbt.preview" => {
                let limit = arg
                    .get("limit")
                    .and_then(|l| l.as_u64())
                    .filter(|&l| l > 0)
                    .unwrap_or(50);
                self.preview(req.id, &uri, limit);
                return;
            }
            "dbt.cancel" => {
                let token = arg.get("token").and_then(|t| t.as_u64()).unwrap_or(0);
                match self.jobs.cancel(token) {
                    Ok(cancelled) => {
                        self.show_message(
                            cogitator_lsp::message_type::INFO,
                            format!("cancelled {} (job {token}).", cancelled.label),
                        );
                        self.send(&ResultResponse::new(req.id, cancelled));
                    }
                    Err(guidance) => {
                        let response = ErrorResponse::new(
                            req.id,
                            error_codes::INVALID_PARAMS,
                            guidance.to_message(),
                        );
                        self.send(&response);
                    }
                }
                return;
            }
            "dbt.runSelection" => {
                let selector = arg
                    .get("selector")
                    .and_then(|s| s.as_str())
                    .unwrap_or_default()
                    .to_string();
                if let Err(guidance) = crate::server::jobs::validate_selector(&selector) {
                    let response = ErrorResponse::new(
                        req.id,
                        error_codes::INVALID_PARAMS,
                        guidance.to_message(),
                    );
                    self.send(&response);
                    return;
                }
                let kind = match arg.get("kind").and_then(|k| k.as_str()) {
                    None | Some("run") => "run",
                    Some("test") => "test",
                    Some("build") => "build",
                    Some("compile") => "compile",
                    Some(other) => {
                        let guidance = cogitator_error::guidance::Guidance::new(format!(
                            "'{other}' is not a dbt.runSelection kind."
                        ))
                        .suggest(cogitator_error::guidance::closest_matches(
                            other,
                            ["run", "test", "build", "compile"],
                        ))
                        .fix("pass kind run, test, build or compile (default: run).");
                        let response = ErrorResponse::new(
                            req.id,
                            error_codes::INVALID_PARAMS,
                            guidance.to_message(),
                        );
                        self.send(&response);
                        return;
                    }
                };
                self.run_dbt_selected(req.id, kind, &uri, Some(selector));
                return;
            }
            "dbt.run" | "dbt.test" | "dbt.compile" | "dbt.parse" => {
                let kind = params.command.trim_start_matches("dbt.");
                let kind: &'static str = match kind {
                    "run" => "run",
                    "test" => "test",
                    "parse" => "parse",
                    _ => "compile",
                };
                self.run_dbt(req.id, kind, &uri);
                return;
            }
            "dbt.extractModelYaml" => {
                self.extract_model_yaml(req.id, &uri);
                return;
            }
            "dbt.modelTests" => {
                let snap = self.snapshot_for(&uri);
                let derived = self.derived.clone();
                self.offload_blocking(req.id, "dbt.modelTests", move || {
                    crate::server::handlers::commands::model_tests(&snap, &derived, &uri)
                });
                return;
            }
            "dbt.goToTests" => {
                self.go_to_tests(req.id, &uri);
                return;
            }
            "dbt.profiles" => {
                self.profiles_report(req.id);
                return;
            }
            "dbt.status" => {
                self.status_report(req.id);
                return;
            }
            "dbt.impact" => {
                let model = arg
                    .get("model")
                    .and_then(|m| m.as_str())
                    .map(str::to_string)
                    .or_else(|| features::model_name_from_uri(&uri));
                let column = arg
                    .get("column")
                    .and_then(|c| c.as_str())
                    .map(str::to_string);
                let Some(model) = model else {
                    let guidance =
                        cogitator_error::guidance::Guidance::new("dbt.impact needs a model.")
                            .because("neither a model argument nor a model-file uri was given.")
                            .fix("pass {\"model\": \"customers\"} or call it from a model buffer.");
                    let response = ErrorResponse::new(
                        req.id,
                        error_codes::INVALID_PARAMS,
                        guidance.to_message(),
                    );
                    self.send(&response);
                    return;
                };
                let snap = self.snapshot_for(&uri);
                let derived = self.derived.clone();
                let artifact_cap = self.limits.artifact_cap_bytes.load(Ordering::Relaxed);
                self.offload_blocking(req.id, "dbt.impact", move || {
                    // typed lineage per downstream model, capped: each file
                    // is read and parsed from disk, stars expand against the
                    // project schemas, control flow only from fresh SQL
                    let typed = crate::server::handlers::lineage::TypedLineage::new(
                        &snap,
                        &derived,
                        artifact_cap,
                    );
                    let mut lineages = std::collections::HashMap::new();
                    for (name, path) in features::impact_targets(&snap.ctx, &model) {
                        let uri = cogitator_lsp::fs_path_to_uri(&path);
                        if let Some(lineage) = typed.for_saved_file(&uri) {
                            lineages.insert(name, lineage);
                        }
                    }
                    crate::server::reply::Reply::result(&features::impact(
                        &snap.ctx,
                        &model,
                        column.as_deref(),
                        &lineages,
                    ))
                });
                return;
            }
            "dbt.expandJinja" => {
                let snap = self.snapshot_for(&uri);
                let derived = self.derived.clone();
                self.offload_blocking(req.id, "dbt.expandJinja", move || {
                    let Some(doc) = snap.doc.as_deref() else {
                        return crate::server::reply::Reply::error(
                            error_codes::INVALID_PARAMS,
                            cogitator_error::guidance::Guidance::new(
                                "dbt.expandJinja needs an open model buffer.",
                            )
                            .fix("pass the uri of a document the editor has opened.")
                            .to_message(),
                        );
                    };
                    let inputs = crate::server::handlers::commands::preview_inputs(&snap, &derived);
                    match features::expand_jinja(&doc.text, &inputs) {
                        Ok(expanded) => crate::server::reply::Reply::result(&expanded),
                        Err(guidance) => crate::server::reply::Reply::error(
                            error_codes::INVALID_PARAMS,
                            guidance.to_message(),
                        ),
                    }
                });
                return;
            }
            "dbt.astGrep" => {
                let snap = self.snapshot_for(&uri);
                let arg = arg.clone();
                self.offload_blocking(req.id, "dbt.astGrep", move || {
                    crate::server::astgrep::search_reply(&snap, &arg, &uri)
                });
                return;
            }
            "dbt.highlight" => {
                let snap = self.snapshot_for(&uri);
                let source = HighlightSource::of(arg, &uri);
                self.offload_blocking(req.id, "dbt.highlight", move || {
                    highlight_reply(&snap, source)
                });
                return;
            }
            "dbt.syntaxTree" => {
                let snap = self.snapshot_for(&uri);
                self.offload_blocking(req.id, "dbt.syntaxTree", move || {
                    syntax_tree_reply(snap.doc.as_deref().map(|d| &*d.text))
                });
                return;
            }
            "dbt.doctor" => {
                let snap = self.snapshot_for("");
                let derived = self.derived.clone();
                let lints = self.toggles.lints.load(Ordering::Relaxed);
                let jobs = self.jobs.list();
                let config = self
                    .config_status
                    .lock()
                    .expect("config status lock")
                    .clone();
                let artifact_cap = self.limits.artifact_cap_bytes.load(Ordering::Relaxed);
                let perf = crate::server::status::PerfStatus::of(&self.perf_settings());
                self.offload_blocking(req.id, "dbt.doctor", move || {
                    crate::server::handlers::doctor::doctor(
                        &snap,
                        &derived,
                        lints,
                        jobs,
                        config,
                        artifact_cap,
                        perf,
                    )
                });
                return;
            }
            "dbt.deadModels" => {
                let ctx = self.snapshot();
                self.offload_blocking(req.id, "dbt.deadModels", move || {
                    #[derive(serde::Serialize)]
                    struct DeadModels {
                        models: Vec<features::DeadModel>,
                    }
                    crate::server::reply::Reply::result(&DeadModels {
                        models: features::dead_models(&ctx),
                    })
                });
                return;
            }
            "dbt.projectGraph" => {
                let ctx = self.snapshot();
                self.offload_blocking(req.id, "dbt.projectGraph", move || {
                    crate::server::reply::Reply::result(&features::project_graph_json(&ctx))
                });
                return;
            }
            "dbt.setTarget" => {
                let target = arg
                    .get("target")
                    .and_then(|t| t.as_str())
                    .unwrap_or_default()
                    .to_string();
                let profile = arg
                    .get("profile")
                    .and_then(|p| p.as_str())
                    .map(str::to_string);
                self.set_target(req.id, target, profile);
                return;
            }
            "dbt.cache" => {
                let action = arg
                    .get("action")
                    .and_then(|a| a.as_str())
                    .unwrap_or("stats");
                let reply = crate::server::handlers::commands::cache(&self.derived, action);
                self.transmit(Some(req.id), reply);
                return;
            }
            "dbt.setProfilesDir" => {
                let dir = arg
                    .get("dir")
                    .and_then(|d| d.as_str())
                    .unwrap_or_default()
                    .to_string();
                self.set_profiles_dir(req.id, dir);
                return;
            }
            "dbt.modelGraph" => {
                let model = arg.get("model").and_then(|m| m.as_str());
                let ctx = self.ctx_for(&uri);
                let result = features::model_graph(&ctx, &uri, model);
                self.send(&ResultResponse::new(req.id, result));
                return;
            }
            "dbt.modelInfo" => {
                let model = arg
                    .get("model")
                    .and_then(|m| m.as_str())
                    .map(str::to_string);
                let snap = self.snapshot_for(&uri);
                let derived = self.derived.clone();
                self.offload_blocking(req.id, "dbt.modelInfo", move || {
                    crate::server::handlers::commands::model_info(
                        &snap,
                        &derived,
                        &uri,
                        model.as_deref(),
                    )
                });
                return;
            }
            "dbt.openModel" => {
                let model = arg.get("model").and_then(|m| m.as_str());
                let snap = self.snapshot_for(&uri);
                let reply = crate::server::handlers::navigation::open_model(&snap, &uri, model);
                self.transmit(Some(req.id), reply);
                return;
            }
            "dbt.compiledSql" => {
                let model = arg
                    .get("model")
                    .and_then(|m| m.as_str())
                    .map(str::to_string);
                let snap = self.snapshot_for(&uri);
                self.offload_blocking(req.id, "dbt.compiledSql", move || {
                    crate::server::handlers::commands::compiled_sql(&snap, &uri, model.as_deref())
                });
                return;
            }
            "dbt.relation" => {
                let pos = arg_position(arg);
                let snap = self.snapshot_for(&uri);
                let derived = self.derived.clone();
                self.offload_blocking(req.id, "dbt.relation", move || {
                    crate::server::handlers::profiles::relation(&snap, &derived, &uri, pos)
                });
                return;
            }
            "dbt.queryPlan" => return self.query_plan(req.id, uri),
            "dbt.reloadConfig" => {
                // the watcher stats, reads and applies off this task;
                // the null answer just acknowledges the trigger
                let _ = self.config_poke_tx.send(());
                self.send(&ResultResponse::new(req.id, serde_json::Value::Null));
                return;
            }
            "dbt.goToSchema" => {}
            other => {
                let guidance = cogitator_error::guidance::Guidance::new(format!(
                    "Unknown command '{other}'."
                ))
                .suggest(cogitator_error::guidance::closest_matches(
                    other,
                    cogitator_lsp::COMMANDS.iter().copied(),
                ))
                .fix("the full roster is in the initialize response's executeCommandProvider.");
                let response =
                    ErrorResponse::new(req.id, error_codes::INVALID_PARAMS, guidance.to_message());
                self.send(&response);
                return;
            }
        }
        let pos = arg_position(arg).unwrap_or_default();
        let ctx = self.ctx_for(&uri);
        let result = features::go_to_schema(&ctx, self.documents.get(&uri), &uri, pos);
        self.send(&ResultResponse::new(req.id, result));
    }
}

/// Cap on the nodes one `dbt.syntaxTree` answer carries; a 2 MiB model
/// stays a few hundred KiB of JSON.
const SYNTAX_TREE_MAX_NODES: usize = 20_000;

/// `dbt.syntaxTree {uri}`: the tree-sitter-dbt tree of an open document,
/// its named nodes in pre-order with depth and UTF-16 ranges.
fn syntax_tree_reply(text: Option<&str>) -> crate::server::reply::Reply {
    let Some(text) = text else {
        return crate::server::reply::Reply::error(
            error_codes::INVALID_PARAMS,
            cogitator_error::guidance::Guidance::new("dbt.syntaxTree needs an open document.")
                .fix("pass the uri of a document the editor has opened.")
                .to_message(),
        );
    };
    let Some(tree) = cogitator_treesitter::syntax_tree(text, SYNTAX_TREE_MAX_NODES) else {
        return crate::server::reply::Reply::error(
            error_codes::INTERNAL_ERROR,
            "the tree-sitter-dbt grammar could not be loaded".to_string(),
        );
    };
    let position = |p: cogitator_treesitter::Point| serde_json::json!({"line": p.line, "character": p.character});
    let nodes: Vec<serde_json::Value> = tree
        .nodes
        .into_iter()
        .map(|n| {
            serde_json::json!({
                "depth": n.depth,
                "kind": n.kind,
                "field": n.field,
                "error": n.error,
                "missing": n.missing,
                "range": {"start": position(n.start), "end": position(n.end)},
            })
        })
        .collect();
    crate::server::reply::Reply::result(&serde_json::json!({
        "nodes": nodes,
        "truncated": tree.truncated,
    }))
}

/// Cap on the spans one `dbt.highlight` answer carries.
const HIGHLIGHT_MAX_SPANS: usize = 50_000;
/// Larger texts are not highlighted (2 MiB, as the compiled SQL's cap).
const HIGHLIGHT_MAX_BYTES: usize = 2 * 1024 * 1024;

/// What `dbt.highlight` was asked to colour.
enum HighlightSource {
    /// `{text, language?}`: any text, `dbt` unless it says `sql`.
    Text(String, Option<String>),
    /// `{model?, compiled: true}`: the SQL dbt compiled the model to.
    Compiled(String, Option<String>),
    /// `{uri}`: the open document, else the file on disk.
    Uri(String),
}

impl HighlightSource {
    fn of(arg: &serde_json::Map<String, serde_json::Value>, uri: &str) -> Self {
        let text = |k: &str| arg.get(k).and_then(|v| v.as_str()).map(str::to_string);
        if let Some(t) = text("text") {
            return Self::Text(t, text("language"));
        }
        if arg.get("compiled").and_then(|v| v.as_bool()) == Some(true) {
            return Self::Compiled(uri.to_string(), text("model"));
        }
        Self::Uri(uri.to_string())
    }
}

/// `dbt.highlight`: tree-sitter-dbt's highlights (and the SQL grammar's
/// between the tags) as `{language, spans: [{range, capture}], truncated}`.
fn highlight_reply(
    snap: &crate::server::snapshot::Snapshot,
    source: HighlightSource,
) -> crate::server::reply::Reply {
    use crate::server::reply::Reply;
    use cogitator_treesitter::HighlightLanguage;
    let refuse = |what: String, fix: &str| {
        Reply::error(
            error_codes::INVALID_PARAMS,
            cogitator_error::guidance::Guidance::new(what)
                .fix(fix)
                .to_message(),
        )
    };
    let (text, language) = match source {
        HighlightSource::Text(text, language) => {
            let language = match language.as_deref() {
                None => HighlightLanguage::Dbt,
                Some(name) => match HighlightLanguage::parse(name) {
                    Some(l) => l,
                    None => {
                        return refuse(
                            format!("dbt.highlight does not know the language '{name}'."),
                            "pass language \"dbt\" (Jinja over SQL) or \"sql\".",
                        )
                    }
                },
            };
            (text, language)
        }
        HighlightSource::Compiled(uri, model) => {
            let (compiled, _) = crate::server::compiled::compiled_sql(
                &snap.ctx,
                &uri,
                model.as_deref(),
                snap.compiled_cap_bytes,
            );
            match compiled.and_then(|c| c.content) {
                Some(text) => (text, HighlightLanguage::Sql),
                None => return Reply::null(),
            }
        }
        HighlightSource::Uri(uri) => {
            let text = match snap.doc.as_deref() {
                Some(doc) => doc.text.to_string(),
                None => {
                    let Some(path) = cogitator_lsp::uri_to_fs_path(&uri) else {
                        return refuse(
                            "dbt.highlight needs a uri, a text, or compiled: true.".into(),
                            "pass {\"uri\": \"file://…\"} or {\"text\": \"…\"}.",
                        );
                    };
                    match std::fs::read_to_string(&path) {
                        Ok(t) => t,
                        Err(e) => {
                            return refuse(
                                format!("cannot read {path}: {e}"),
                                "pass an existing file.",
                            )
                        }
                    }
                }
            };
            (text, HighlightLanguage::Dbt)
        }
    };
    if text.len() > HIGHLIGHT_MAX_BYTES {
        return Reply::result(&serde_json::json!({
            "language": language.name(), "spans": [], "truncated": true,
        }));
    }
    let Some(found) = cogitator_treesitter::highlight(&text, language, HIGHLIGHT_MAX_SPANS) else {
        return Reply::error(
            error_codes::INTERNAL_ERROR,
            "the tree-sitter-dbt grammar could not be loaded".to_string(),
        );
    };
    let position = |p: cogitator_treesitter::Point| serde_json::json!({"line": p.line, "character": p.character});
    let spans: Vec<serde_json::Value> = found
        .spans
        .into_iter()
        .map(|s| {
            serde_json::json!({
                "range": {"start": position(s.start), "end": position(s.end)},
                "capture": s.capture,
            })
        })
        .collect();
    Reply::result(&serde_json::json!({
        "language": language.name(),
        "spans": spans,
        "truncated": found.truncated,
    }))
}
