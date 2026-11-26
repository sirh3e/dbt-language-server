//! The textDocument/* and workspace/* REQUEST handlers: one shared
//! decode→snapshot→pure-feature→serialize composition point for the
//! position-shaped ones, offload for anything that touches the fs.

use std::sync::atomic::Ordering;

use cogitator_analysis::{features, yaml_features, DbtContext};

use cogitator_lsp::{
    Envelope, HoverResult, MaybeId, Position, PositionParams, ResultResponse, SelectionRangeParams,
    WorkspaceSymbolParams,
};

use super::Dispatcher;

impl Dispatcher {
    /// Composition point for every position-shaped request: parse → snapshot
    /// → pure feature function → serialize (functional core / imperative
    /// shell, 05 §3). Bad params on a request answer InvalidParams.
    pub(super) fn respond_position<R: serde::Serialize>(
        &mut self,
        content: &[u8],
        method: &str,
        feature: impl FnOnce(&DbtContext, Option<&cogitator_analysis::Document>, &str, Position) -> R,
    ) {
        let Some((id, uri, pos)) = Self::position_request(content) else {
            self.invalid_params(content, method);
            return;
        };
        // multi-root: the context must be the one whose root owns the
        // file, not always the primary (its siblings use snapshot_for)
        let ctx = self.ctx_for(&uri);
        let result = feature(&ctx, self.documents.get(&uri), &uri, pos);
        self.send(&ResultResponse::new(id, result));
    }

    pub(super) fn position_request<'a>(content: &'a [u8]) -> Option<(i64, String, Position)> {
        let req = serde_json::from_slice::<Envelope<PositionParams<'a>>>(content).ok()?;
        let params = req.params?;
        Some((
            req.id,
            params.text_document.uri.into_owned(),
            params.position,
        ))
    }

    pub(super) fn hover(&mut self, content: &[u8]) {
        if self.compiled_hover(content) {
            return;
        }
        if self.hover_jinja(content) {
            return;
        }
        if self.selectors_hover(content) {
            return;
        }
        let derived = self.derived.clone();
        self.respond_position(content, "textDocument/hover", |ctx, doc, uri, pos| {
            HoverResult {
                // Q8: unknown doc → empty hover
                contents: doc
                    .map(|d| {
                        if cogitator_analysis::yaml_cogitator::is_cogitator_config(uri) {
                            cogitator_analysis::yaml_cogitator::cogitator_hover(&d.text, pos)
                                .unwrap_or_default()
                        } else {
                            // dispatcher path: a cache probe, never a compute
                            let lineage = derived.lineage_cached(uri, d.generation);
                            let text = features::hover_with_lineage(
                                ctx,
                                d,
                                pos,
                                lineage.as_deref().and_then(Option::as_ref),
                            );
                            // the column's inferred type, from the published snapshot
                            with_type_line(
                                text,
                                super::yaml_types::hover(ctx, &derived, d, uri, pos),
                            )
                        }
                    })
                    .unwrap_or_default(),
            }
        });
        self.warm_lineage(content);
    }

    /// A hover inside a SQL file's `{{ … }}` with nothing else to say
    /// (not on a ref, var, macro… name that has its own hover): what the
    /// expression renders to, offloaded (the renderer reads the macro
    /// files and runs on a fuel budget). `false`: not such a position.
    fn hover_jinja(&mut self, content: &[u8]) -> bool {
        let Some((id, uri, pos)) = Self::position_request(content) else {
            return false;
        };
        if !uri.ends_with(".sql") {
            return false;
        }
        let snap = self.snapshot_for(&uri);
        let Some(doc) = snap.doc.clone() else {
            return false;
        };
        if features::jinja_expression_at(&doc, pos).is_none() {
            return false;
        }
        // a token with a hover of its own keeps exactly that hover
        let ctx = self.ctx_for(&uri);
        if !features::hover_with_lineage(&ctx, &doc, pos, None).is_empty() {
            return false;
        }
        let derived = self.derived.clone();
        self.offload_blocking(id, "textDocument/hover", move || {
            let inputs = crate::server::handlers::commands::preview_inputs(&snap, &derived);
            let renderer = features::InlineRenderer::new(&inputs);
            let contents = features::jinja_hover(&doc, pos, &renderer).unwrap_or_default();
            crate::server::reply::Reply::result(&HoverResult { contents })
        });
        true
    }

    /// Fills the lineage cache after a hover missed it, off the
    /// dispatcher, so the next hover of this text has the lineage line.
    fn warm_lineage(&self, content: &[u8]) {
        let Some((_, uri, _)) = Self::position_request(content) else {
            return;
        };
        if !uri.ends_with(".sql") {
            return;
        }
        let snap = self.snapshot_for(&uri);
        let Some(generation) = snap.doc.as_deref().map(|doc| doc.generation) else {
            return;
        };
        if self.derived.lineage_cached(&uri, generation).is_some() {
            return;
        }
        let derived = self.derived.clone();
        let cap = self
            .limits
            .artifact_cap_bytes
            .load(std::sync::atomic::Ordering::Relaxed);
        tokio::task::spawn_blocking(move || {
            crate::server::handlers::lineage::typed_lineage_for(&snap, &derived, &uri, cap);
        });
    }

    pub(super) fn definition(&mut self, content: &[u8]) {
        // a documented SQL function materializes its bundled docs as a
        // cached Markdown file and jumps there (03 §9) — offloaded,
        // since that touches the filesystem; everything else keeps the
        // inline pure path
        let Some((id, uri, pos)) = Self::position_request(content) else {
            self.invalid_params(content, "textDocument/definition");
            return;
        };
        let snap = self.snapshot_for(&uri);
        if crate::server::compiled_source::source_of(&snap.ctx, &uri).is_some() {
            self.compiled_definition(id, uri, pos, snap);
            return;
        }
        // a properties-YAML column: where its model's SQL produces it
        if self.yaml_column_definition(id, &uri, pos, &snap) {
            return;
        }
        let function = snap
            .doc
            .as_deref()
            .and_then(|doc| features::function_doc_target(&snap.ctx, doc, pos));
        if let (Some(function), Some(root)) = (function, snap.ctx.project_root.clone()) {
            let fallback = cogitator_lsp::Location {
                uri: uri.clone(),
                range: cogitator_lsp::Range::zero_width(pos),
            };
            self.offload_blocking(id, "textDocument/definition", move || {
                match super::super::function_docs::materialize(&root, &function) {
                    Some(path) => super::super::reply::Reply::result(&cogitator_lsp::Location {
                        uri: cogitator_lsp::fs_path_to_uri(&path),
                        range: cogitator_lsp::Range::default(),
                    }),
                    None => super::super::reply::Reply::result(&fallback),
                }
            });
            return;
        }
        let location = match snap.doc.as_deref() {
            Some(doc) => features::definition(&snap.ctx, doc, &uri, pos),
            None => cogitator_lsp::Location {
                uri: uri.clone(),
                range: cogitator_lsp::Range::zero_width(pos),
            },
        };
        self.send(&ResultResponse::new(id, location));
    }

    pub(super) fn completion(&mut self, content: &[u8]) {
        // selector methods/values and tag names: the selector engine
        if self.selectors_completion(content) {
            return;
        }
        // properties YAML: schema-driven, offloaded (lineage + test files)
        if let Some((id, uri, pos)) = Self::position_request(content) {
            if cogitator_analysis::yaml_properties::is_properties_file(&uri) {
                self.properties_completion(id, uri, pos);
                return;
            }
        }
        self.respond_position(content, "textDocument/completion", |ctx, doc, uri, pos| {
            match doc {
                // the server's own config file: schema-table completion
                Some(d) if cogitator_analysis::yaml_cogitator::is_cogitator_config(uri) => {
                    cogitator_analysis::yaml_cogitator::cogitator_completion(&d.text, pos)
                }
                // dbt YAML files get schema-key completion (08 §2.6 v1)
                Some(d) if crate::server::diagnostics::is_yaml(uri) => {
                    yaml_features::yaml_completion(uri, &d.text, pos)
                }
                Some(d) => features::completion(ctx, d, pos),
                None => Vec::new(),
            }
        });
    }

    pub(super) fn resolve_completion(&mut self, content: &[u8]) {
        let Ok(req) = serde_json::from_slice::<MaybeId>(content) else {
            self.invalid_params(content, "completionItem/resolve");
            return;
        };
        let Some(id) = req.id else { return };
        let Ok(item) = serde_json::from_slice::<serde_json::Value>(content) else {
            self.invalid_params(content, "completionItem/resolve");
            return;
        };
        let mut item = item.get("params").cloned().unwrap_or(item);
        let ctx = self.snapshot();
        if let Some(docs) = item
            .get("data")
            .and_then(|data| features::resolve_completion_documentation(&ctx, data))
        {
            if let Some(map) = item.as_object_mut() {
                map.insert("documentation".into(), serde_json::Value::String(docs));
            }
        }
        self.send(&ResultResponse::new(id, item));
    }

    pub(super) fn document_symbol(&mut self, content: &[u8]) {
        self.respond_position(content, "textDocument/documentSymbol", |_, doc, uri, _| {
            doc.map(|d| features::document_symbols(d, uri))
                .unwrap_or_default()
        });
    }

    pub(super) fn document_highlight(&mut self, content: &[u8]) {
        self.respond_position(
            content,
            "textDocument/documentHighlight",
            |ctx, doc, uri, pos| {
                features::local_highlights(ctx, doc, uri, pos)
                    .or_else(|| doc.map(|d| features::document_highlights(d, pos)))
                    .unwrap_or_default()
            },
        );
    }

    pub(super) fn folding_range(&mut self, content: &[u8]) {
        self.respond_position(content, "textDocument/foldingRange", |_, doc, _, _| {
            doc.map(features::folding_ranges).unwrap_or_default()
        });
    }

    pub(super) fn selection_range(&mut self, content: &[u8]) {
        let Ok(req) = serde_json::from_slice::<Envelope<SelectionRangeParams>>(content) else {
            self.invalid_params(content, "textDocument/selectionRange");
            return;
        };
        let Some(params) = req.params else {
            self.invalid_params(content, "textDocument/selectionRange");
            return;
        };
        let uri = params.text_document.uri.into_owned();
        let ranges = self
            .documents
            .get(&uri)
            .map(|doc| features::selection_ranges(doc, &params.positions))
            .unwrap_or_default();
        self.send(&ResultResponse::new(req.id, ranges));
    }

    pub(super) fn call_hierarchy_incoming(&mut self, content: &[u8]) {
        let Ok(req) =
            serde_json::from_slice::<Envelope<cogitator_lsp::CallHierarchyCallsParams>>(content)
        else {
            self.invalid_params(content, "callHierarchy/incomingCalls");
            return;
        };
        let Some(params) = req.params else {
            self.invalid_params(content, "callHierarchy/incomingCalls");
            return;
        };
        let ctx = self.ctx_for(&params.item.uri);
        // offloaded: every caller file is re-read from disk
        self.offload_blocking(req.id, "callHierarchy/incomingCalls", move || {
            let calls = features::macro_incoming_calls(&ctx, &params.item);
            crate::server::reply::Reply::result(&calls)
        });
    }

    pub(super) fn code_lens(&mut self, content: &[u8]) {
        // offloaded since the artifact batch: the run-status lens stats
        // target/ files (05 §6.1 keeps fs off this task)
        let Some((id, uri, _)) = Self::position_request(content) else {
            self.invalid_params(content, "textDocument/codeLens");
            return;
        };
        let snap = self.snapshot_for(&uri);
        let derived = self.derived.clone();
        let cap = self.limits.artifact_cap_bytes.load(Ordering::Relaxed);
        let open = features::selectors_yml::is_selectors_file(&uri).then(|| self.selectors_open());
        self.offload_blocking(id, "textDocument/codeLens", move || {
            let mut lenses = features::code_lenses(&snap.ctx, &uri);
            if let Some(doc) = snap.doc.as_deref() {
                lenses.extend(features::cte_preview_lenses(&snap.ctx, doc, &uri));
            }
            if let (Some(open), Some(doc)) = (open, snap.doc.as_deref()) {
                lenses.extend(crate::server::selectors::lenses(
                    &snap.ctx, open, &uri, &doc.text,
                ));
            }
            lenses.extend(quality_lenses(&snap, &derived, &uri, cap));
            // the last open/save report, never computed here
            lenses.extend(
                derived
                    .breaking_report(&uri)
                    .and_then(|r| features::breaking::lens(&r, &uri)),
            );
            if let Some(root) = snap.ctx.project_root.as_deref() {
                let set = derived.artifacts(root, cap);
                lenses.extend(features::run_lens(&snap.ctx, &uri, &set));
                lenses.extend(features::freshness_lenses(&snap.ctx, &uri, &set));
                lenses.extend(crate::server::handlers::incremental::lens(
                    &snap, &set, &uri,
                ));
            }
            crate::server::reply::Reply::result(&lenses)
        });
    }

    pub(super) fn inlay_hint(&mut self, content: &[u8]) {
        let Ok(req) = serde_json::from_slice::<Envelope<cogitator_lsp::InlayHintParams>>(content)
        else {
            self.invalid_params(content, "textDocument/inlayHint");
            return;
        };
        let Some(params) = req.params else {
            self.invalid_params(content, "textDocument/inlayHint");
            return;
        };
        let uri = params.text_document.uri.into_owned();
        let range = params.range;
        let snap = self.snapshot_for(&uri);
        let derived = self.derived.clone();
        let options = features::InlayOptions {
            vars: !self
                .toggles
                .inlay_vars_off
                .load(std::sync::atomic::Ordering::Relaxed),
            env_vars: !self
                .toggles
                .inlay_env_vars_off
                .load(std::sync::atomic::Ordering::Relaxed),
        };
        let jinja = self
            .toggles
            .inlay_jinja
            .load(std::sync::atomic::Ordering::Relaxed)
            && uri.ends_with(".sql");
        let perf = self.perf_settings();
        let cap = self.limits.artifact_cap_bytes.load(Ordering::Relaxed);
        // offloaded: the relation labels read the profiles inventory
        self.offload_blocking(req.id, "textDocument/inlayHint", move || {
            let target = crate::server::handlers::profiles::active_target_relation(&snap, &derived);
            let hints = match snap.doc.as_deref() {
                Some(doc) => {
                    let mut hints = features::inlay_hints_with(
                        &snap.ctx,
                        doc,
                        &uri,
                        range,
                        target.as_ref(),
                        options,
                    );
                    if jinja {
                        let inputs =
                            crate::server::handlers::commands::preview_inputs(&snap, &derived);
                        let renderer = features::InlineRenderer::new(&inputs);
                        hints.extend(features::jinja_inlay_hints(doc, range, &renderer));
                    }
                    hints
                }
                None => Vec::new(),
            };
            let mut hints: Vec<cogitator_lsp::AnyInlayHint> = hints
                .into_iter()
                .map(cogitator_lsp::AnyInlayHint::Plain)
                .collect();
            hints.extend(crate::server::handlers::plan_hints::plan_hints(
                &snap, &derived, &uri, range, &perf, cap,
            ));
            crate::server::reply::Reply::result(&hints)
        });
    }

    pub(super) fn document_link(&mut self, content: &[u8]) {
        self.respond_position(content, "textDocument/documentLink", |ctx, doc, uri, _| {
            doc.map(|d| features::document_links_at(ctx, d, uri))
                .unwrap_or_default()
        });
    }

    pub(super) fn linked_editing(&mut self, content: &[u8]) {
        self.respond_position(
            content,
            "textDocument/linkedEditingRange",
            |_, doc, _, pos| doc.and_then(|d| features::linked_editing(d, pos)),
        );
    }

    pub(super) fn formatting(&mut self, content: &[u8]) {
        let Ok(req) =
            serde_json::from_slice::<Envelope<cogitator_lsp::DocumentFormattingParams>>(content)
        else {
            self.invalid_params(content, "textDocument/formatting");
            return;
        };
        let Some(params) = req.params else {
            self.invalid_params(content, "textDocument/formatting");
            return;
        };
        let uri = params.text_document.uri.into_owned();
        let configured = self
            .toggles
            .format
            .read()
            .expect("format rules lock")
            .clone();
        let snap = self.snapshot_for(&uri);
        let mut options = features::formatting::FormatOptions {
            tab_size: params.options.tab_size,
            insert_spaces: params.options.insert_spaces,
            rules: configured,
            adapter: snap.ctx.dialect.as_str().to_string(),
        };
        self.offload_blocking(req.id, "textDocument/formatting", move || {
            // cogitator.yml's `format:` first, else the nearest
            // cogitator-format.toml; a broken one formats nothing
            if options.rules.is_none() {
                let path = cogitator_lsp::uri_to_fs_path(&uri).unwrap_or_else(|| uri.clone());
                match features::formatting::rules_near(std::path::Path::new(&path)) {
                    Ok(rules) => options.rules = rules,
                    Err(err) => {
                        tracing::warn!(%err, "formatting: the formatter's config cannot be read");
                        let none: Vec<cogitator_lsp::TextEdit> = Vec::new();
                        return crate::server::reply::Reply::result(&none);
                    }
                }
            }
            // decline-on-doubt answers an empty edit list, never an error
            let edits = snap
                .doc
                .as_deref()
                .filter(|_| !uri.ends_with(".yml") && !uri.ends_with(".yaml"))
                .and_then(|doc| features::formatting::format_document(&doc.text, &options))
                .unwrap_or_default();
            crate::server::reply::Reply::result(&edits)
        });
    }

    pub(super) fn signature_help(&mut self, content: &[u8]) {
        self.respond_position(content, "textDocument/signatureHelp", |ctx, doc, _, pos| {
            doc.and_then(|d| features::signature_help(ctx, d, pos))
        });
    }

    pub(super) fn workspace_symbol(&mut self, content: &[u8]) {
        let Ok(req) = serde_json::from_slice::<Envelope<WorkspaceSymbolParams>>(content) else {
            self.invalid_params(content, "workspace/symbol");
            return;
        };
        let query = req
            .params
            .and_then(|p| p.query.map(|q| q.into_owned()))
            .unwrap_or_default();
        // tag:/selector: go through the selector engine, whose universe
        // reads files: offloaded
        let ctx = self.snapshot();
        let open = self.selectors_open();
        self.offload_blocking(req.id, "workspace/symbol", move || {
            crate::server::selectors::workspace_symbols(&ctx, open, &query)
        });
    }
}

/// The coverage and run-this-test lenses: YAML from the open buffer (or
/// the file), lineage only for a model that has a YAML entry to count
/// against.
fn quality_lenses(
    snap: &crate::server::snapshot::Snapshot,
    derived: &crate::server::derived_cache::DerivedCache,
    uri: &str,
    cap: u64,
) -> Vec<cogitator_lsp::CodeLens> {
    let is_yaml = uri.ends_with(".yml") || uri.ends_with(".yaml");
    let text = match snap.doc.as_deref() {
        Some(doc) => Some(doc.text.clone()),
        None if is_yaml => cogitator_lsp::uri_to_fs_path(uri)
            .and_then(|path| std::fs::read_to_string(path).ok())
            .map(std::sync::Arc::<str>::from),
        None => None,
    };
    let counted = !is_yaml
        && features::model_name_from_uri(uri)
            .and_then(|m| snap.ctx.model_detail_map.get(&m))
            .is_some_and(|d| d.schema_uri.is_some());
    let lineage = counted
        .then(|| crate::server::handlers::lineage::typed_lineage_for(snap, derived, uri, cap));
    features::quality::quality_lenses(
        &snap.ctx,
        uri,
        text.as_deref(),
        lineage.as_ref().and_then(|l| (**l).as_ref()),
    )
}

/// Appends a column-type hover line after whatever the hover says.
fn with_type_line(text: String, line: Option<String>) -> String {
    match line {
        None => text,
        Some(line) if text.is_empty() => line,
        Some(line) => format!("{text}\n\n{line}"),
    }
}
