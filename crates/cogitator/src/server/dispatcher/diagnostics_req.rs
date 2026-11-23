//! The diagnostics-adjacent request handlers: codeAction (quickfix
//! assembly from context diagnostics), pull diagnostics with lint +
//! artifact merge, and the workspace-wide report with revision ids.

use std::sync::atomic::Ordering;

use cogitator_analysis::features;

use cogitator_lsp::{Envelope, FullDocumentDiagnosticReport};

use super::Dispatcher;

/// The model's columns against its properties YAML: the undocumented
/// (or, under a contract, undeclared) ones to add, and the contracted
/// ones it never produces to remove. Lineage is the typed one the
/// undocumented-columns lint and the contract check read, so a finding
/// and its fix name the same columns.
fn column_actions(
    snap: &crate::server::snapshot::Snapshot,
    derived: &crate::server::derived_cache::DerivedCache,
    uri: &str,
    artifact_cap: u64,
) -> Vec<cogitator_lsp::CodeAction> {
    let Some(model) = features::model_name_from_uri(uri) else {
        return Vec::new();
    };
    let Some(details) = snap.ctx.model_detail_map.get(&model) else {
        return Vec::new();
    };
    let Some(yaml) = details
        .schema_uri
        .as_deref()
        .and_then(|path| std::fs::read_to_string(path).ok())
    else {
        return Vec::new();
    };
    let lineage =
        crate::server::handlers::lineage::typed_lineage_for(snap, derived, uri, artifact_cap);
    let Some(lineage) = (*lineage).as_ref() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    out.extend(features::add_missing_columns_action(
        &snap.ctx, &model, details, lineage, &yaml,
    ));
    out.extend(features::remove_unproduced_columns_action(
        &model, details, lineage, &yaml,
    ));
    out
}

/// `dbt.extractModelYaml` as an edit, on the lineage the command uses.
fn extract_yaml_action(
    snap: &crate::server::snapshot::Snapshot,
    derived: &crate::server::derived_cache::DerivedCache,
    uri: &str,
) -> Option<cogitator_lsp::CodeAction> {
    let model = features::model_name_from_uri(uri)?;
    let lineage = crate::server::handlers::commands::lineage_for(
        derived,
        &snap.ctx,
        snap.doc.as_deref(),
        uri,
    );
    let extract = features::extract_model_yaml(&snap.ctx, uri, (*lineage).as_ref())?;
    Some(features::extract_yaml_action(&model, &extract))
}

/// "Add missing columns" from the model's YAML entry: the same action
/// the model's SQL offers, on the model's typed lineage and the YAML
/// document's live text.
fn entry_column_action(
    snap: &crate::server::snapshot::Snapshot,
    derived: &crate::server::derived_cache::DerivedCache,
    models: &[super::yaml_props::EntryModel],
    model: &str,
    yaml_text: &str,
    artifact_cap: u64,
) -> Option<cogitator_lsp::CodeAction> {
    let entry = models.iter().find(|m| m.name == model)?;
    let details = snap.ctx.model_detail_map.get(model)?;
    let lineage = super::yaml_props::lineage_of(entry, derived, artifact_cap);
    features::add_missing_columns_action(&snap.ctx, model, details, (*lineage).as_ref()?, yaml_text)
}

impl Dispatcher {
    pub(super) fn code_action(&mut self, content: &[u8]) {
        let Ok(req) = serde_json::from_slice::<Envelope<cogitator_lsp::CodeActionParams>>(content)
        else {
            self.invalid_params(content, "textDocument/codeAction");
            return;
        };
        let Some(params) = req.params else {
            self.invalid_params(content, "textDocument/codeAction");
            return;
        };
        let uri = params.text_document.uri.into_owned();
        let context = params.context.unwrap_or_default();
        let only: Option<Vec<String>> = context
            .only
            .map(|kinds| kinds.into_iter().map(|k| k.into_owned()).collect());
        let diagnostics = context.diagnostics;
        let range = params.range;
        let snap = self.snapshot_for(&uri);
        let derived = self.derived.clone();
        let artifact_cap = self.limits.artifact_cap_bytes.load(Ordering::Relaxed);
        let entry_models = if crate::server::diagnostics::is_yaml(&uri) {
            self.yaml_entry_models(&uri)
        } else {
            Vec::new()
        };
        // offloaded since the editing-actions batch: target choices,
        // the relation catalog and lineage all touch the filesystem
        self.offload_blocking(req.id, "textDocument/codeAction", move || {
            let targets = crate::server::handlers::profiles::target_choices(&snap, &derived, &uri);
            let mut actions = features::code_actions(
                &snap.ctx,
                snap.doc.as_deref(),
                &uri,
                range,
                only.as_deref(),
                targets.as_deref(),
                &diagnostics,
            );
            let mut extra = Vec::new();
            if let Some(doc) = snap.doc.as_deref() {
                let read = |path: &std::path::Path| std::fs::read_to_string(path).ok();
                if uri.ends_with(".yml") || uri.ends_with(".yaml") {
                    // doc('x') lives mostly in descriptions
                    extra.extend(features::declare_missing_actions(
                        &snap.ctx, doc, &uri, range, &read,
                    ));
                    if let Some((model, details)) = features::model_entry_at(&snap.ctx, &uri, range)
                    {
                        let model_uri = cogitator_lsp::fs_path_to_uri(&details.uri);
                        let tests = features::model_tests(&snap.ctx, &model_uri)
                            .map(|(_, tests)| tests)
                            .unwrap_or_default();
                        extra.extend(features::scaffold_tests_action(
                            &doc.text, &uri, model, details, &tests,
                        ));
                        let lineage = crate::server::handlers::commands::lineage_for(
                            &derived, &snap.ctx, None, &model_uri,
                        );
                        extra.extend(read(&details.uri).and_then(|sql| {
                            features::unit_test_scaffold_action(
                                &snap.ctx,
                                model,
                                details,
                                &sql,
                                &doc.text,
                                &uri,
                                (*lineage).as_ref(),
                            )
                        }));
                        if let Some(lineage) = (*lineage).as_ref() {
                            extra.extend(features::inherit_description_actions(
                                &snap.ctx, &doc.text, &uri, details, lineage,
                            ));
                        }
                        extra.extend(entry_column_action(
                            &snap,
                            &derived,
                            &entry_models,
                            model,
                            &doc.text,
                            artifact_cap,
                        ));
                        extra.extend(super::yaml_types::fill_action(
                            &snap.ctx, &derived, &doc.text, &uri, range,
                        ));
                    }
                } else {
                    let target =
                        crate::server::handlers::profiles::active_target_relation(&snap, &derived);
                    let catalog = features::RelationCatalog::build(&snap.ctx, target.as_ref());
                    extra.extend(features::convert_literal_relations(doc, &uri, &catalog));
                    extra.extend(features::define_missing_actions(
                        &snap.ctx, doc, &uri, range, &read,
                    ));
                    extra.extend(column_actions(&snap, &derived, &uri, artifact_cap));
                    extra.extend(features::declare_missing_actions(
                        &snap.ctx, doc, &uri, range, &read,
                    ));
                    extra.extend(features::private_ref_actions(
                        &snap.ctx, doc, &uri, range, &read,
                    ));
                    extra.extend(features::expand_star_action(&snap.ctx, doc, &uri, range));
                    extra.extend(features::extract_cte_action(&snap.ctx, doc, &uri, range));
                    extra.extend(extract_yaml_action(&snap, &derived, &uri));
                    extra.extend(unit_test_action(&snap, &derived, doc, &uri, &read));
                    extra.extend(
                        crate::server::handlers::plan::split_plan_of(
                            &snap,
                            &derived,
                            &uri,
                            artifact_cap,
                        )
                        .and_then(|plan| features::split_action(&plan, &uri)),
                    );
                }
            }
            if let (Some(doc), Some(root)) = (snap.doc.as_deref(), snap.ctx.project_root.as_deref())
            {
                let rules = derived.astgrep_rules(root);
                extra.extend(crate::server::astgrep::actions(
                    &rules, &uri, &doc.text, range,
                ));
            }
            extra.retain(|action| action.allowed_by(only.as_deref()));
            actions.extend(extra);
            crate::server::reply::Reply::result(&actions)
        });
    }

    /// textDocument/diagnostic (pull): same static set the push pipeline uses.
    pub(super) fn pull_diagnostics(&mut self, content: &[u8]) {
        let Some((id, uri, _)) = Self::position_request(content) else {
            self.invalid_params(content, "textDocument/diagnostic");
            return;
        };
        let snap = self.snapshot_for(&uri);
        let mut flags = self.toggles.lints.load(Ordering::Relaxed);
        if self.fusion_enabled.load(Ordering::Relaxed) {
            // Fusion's compile owns syntax errors (07 §3 precedence)
            flags &= !cogitator_analysis::config_schema::lint_flags::SYNTAX;
        }
        let lints = features::lint::LintOptions::from_flags(flags);
        let derived = self.derived.clone();
        let artifact_cap = self.limits.artifact_cap_bytes.load(Ordering::Relaxed);
        let perf = self.perf_settings();
        let entry_models = if crate::server::diagnostics::is_yaml(&uri) {
            self.yaml_entry_models(&uri)
        } else {
            Vec::new()
        };
        let selectors_open = cogitator_analysis::features::selectors_yml::is_selectors_file(&uri)
            .then(|| self.selectors_open());
        self.offload_blocking(id, "textDocument/diagnostic", move || {
            // the pull path affords lineage (cached per generation), so
            // the undocumented-columns and contract checks run here, not
            // on push; typed, so an expanded star counts as its columns
            let lineage = (lints.undocumented_columns && !uri.ends_with(".yml")).then(|| {
                crate::server::handlers::lineage::typed_lineage_for(
                    &snap,
                    &derived,
                    &uri,
                    artifact_cap,
                )
            });
            let mut items = crate::server::diagnostics::static_diagnostics(
                &snap.ctx,
                snap.doc.as_deref(),
                &uri,
                lints,
                lineage.as_ref().and_then(|l| (**l).as_ref()),
            );
            if let (Some(open), Some(doc)) = (selectors_open, snap.doc.as_deref()) {
                items.extend(crate::server::selectors::unmatched(
                    &snap.ctx, open, &doc.text,
                ));
            }
            if let Some(doc) = snap.doc.as_deref().filter(|_| !entry_models.is_empty()) {
                // tests on columns the model never produces (pull only:
                // it takes each entry's lineage)
                items.extend(super::yaml_props::unproduced_diagnostics(
                    &snap.ctx,
                    &doc.text,
                    &entry_models,
                    &derived,
                    artifact_cap,
                ));
            }
            if let Some(doc) = snap
                .doc
                .as_deref()
                .filter(|_| crate::server::diagnostics::is_yaml(&uri))
            {
                // data_type and unit-test rows against the inferred
                // column types (read off the published snapshot)
                items.extend(super::yaml_types::diagnostics(
                    &snap.ctx, &derived, &doc.text,
                ));
            }
            if let Some(root) = snap.ctx.project_root.as_deref() {
                let set = derived.artifacts(root, artifact_cap);
                items.extend(features::artifact_diagnostics(&snap.ctx, &uri, &set));
                if let Some(doc) = snap.doc.as_deref() {
                    let rules = derived.astgrep_rules(root);
                    items.extend(crate::server::astgrep::diagnostics(&rules, &uri, &doc.text));
                }
            }
            // computed on open/save (dispatcher/breaking.rs), read here
            items.extend(crate::server::diagnostics::breaking_diagnostics(
                &derived, &uri,
            ));
            if perf.active() {
                if let (Some(doc), Some(plan)) = (
                    snap.doc.as_deref(),
                    crate::server::handlers::plan::document_plan(
                        &snap,
                        &derived,
                        &uri,
                        artifact_cap,
                        perf.catalog_stats,
                    ),
                ) {
                    items.extend(crate::server::diagnostics::perf_diagnostics(
                        doc, &plan, &perf,
                    ));
                }
            }
            crate::server::reply::Reply::result(&FullDocumentDiagnosticReport {
                kind: "full",
                items,
            })
        });
    }

    /// workspace/diagnostic (01 §6.4): project-wide dangling findings
    /// under the global context-revision result-id scheme.
    pub(super) fn workspace_diagnostics(&mut self, content: &[u8]) {
        let Ok(req) =
            serde_json::from_slice::<Envelope<cogitator_lsp::WorkspaceDiagnosticParams>>(content)
        else {
            self.invalid_params(content, "workspace/diagnostic");
            return;
        };
        let previous: std::collections::HashMap<String, String> = req
            .params
            .unwrap_or_default()
            .previous_result_ids
            .into_iter()
            .map(|p| (p.uri, p.value))
            .collect();
        let ctx = self.snapshot();
        let revision = self.context_revision.load(Ordering::Relaxed).to_string();
        self.offload_blocking(req.id, "workspace/diagnostic", move || {
            let findings =
                features::project_diagnostics(&ctx, &|path| std::fs::read_to_string(path).ok());
            let mut items = Vec::new();
            let mut seen = std::collections::HashSet::new();
            for (uri, diagnostics) in findings {
                seen.insert(uri.clone());
                if previous.get(&uri) == Some(&revision) {
                    items.push(cogitator_lsp::WorkspaceDocumentReport::Unchanged {
                        uri,
                        result_id: revision.clone(),
                        version: None,
                    });
                } else {
                    items.push(cogitator_lsp::WorkspaceDocumentReport::Full {
                        uri,
                        result_id: revision.clone(),
                        version: None,
                        items: diagnostics,
                    });
                }
            }
            // files the client still tracks that are clean now answer an
            // empty Full so stale findings clear
            for (uri, _) in previous {
                if !seen.contains(&uri) {
                    items.push(cogitator_lsp::WorkspaceDocumentReport::Full {
                        uri,
                        result_id: revision.clone(),
                        version: None,
                        items: Vec::new(),
                    });
                }
            }
            crate::server::reply::Reply::result(&cogitator_lsp::WorkspaceDiagnosticReport { items })
        });
    }
}

/// "Scaffold a unit test" on a model with a properties YAML, its rows
/// keyed by the lineage's columns where the model's lineage is known.
fn unit_test_action(
    snap: &crate::server::snapshot::Snapshot,
    derived: &crate::server::derived_cache::DerivedCache,
    doc: &cogitator_analysis::Document,
    uri: &str,
    read: &dyn Fn(&std::path::Path) -> Option<String>,
) -> Option<cogitator_lsp::CodeAction> {
    let model = features::model_name_from_uri(uri)?;
    let details = snap.ctx.model_detail_map.get(&model)?;
    let schema = details.schema_uri.as_ref()?;
    let yaml = read(schema)?;
    let lineage =
        crate::server::handlers::commands::lineage_for(derived, &snap.ctx, Some(doc), uri);
    features::unit_test_scaffold_action(
        &snap.ctx,
        &model,
        details,
        &doc.text,
        &yaml,
        &cogitator_lsp::fs_path_to_uri(schema),
        (*lineage).as_ref(),
    )
}
