//! Artifact-backed surfaces (03 §10): the last-run lens and the
//! run/test/drift diagnostics, pure over the context plus whatever
//! artifact payloads the shell managed to load.

use cogitator_error::guidance::Guidance;
use cogitator_lsp::{diagnostic_severity, CodeLens, Command, Diagnostic, Location, Range};

use super::{model_name_from_uri, DbtContext};

/// Diagnostics render Guidance headlines only, so the adapter message
/// folds into the headline (first line, bounded).
fn with_reason(headline: String, message: &Option<String>) -> String {
    match message.as_deref().and_then(|m| m.lines().next()) {
        Some(reason) if !reason.trim().is_empty() => {
            let mut reason = reason.trim().to_string();
            reason.truncate(160);
            format!("{headline}: {reason}")
        }
        _ => headline,
    }
}
use crate::artifacts::status::{manifest_drift, run_status_by_model, test_failures, DriftKind};
use crate::artifacts::{ArtifactPaths, ArtifactSet};

/// "last run: OK in 3.2 s" on a model file — informational (empty
/// command), like the macro call-site lenses.
pub fn run_lens(ctx: &DbtContext, uri: &str, set: &ArtifactSet) -> Option<CodeLens> {
    let run_results = set.run_results.as_ref()?;
    let model = model_name_from_uri(uri)?;
    if !ctx.model_detail_map.contains_key(&model) {
        return None;
    }
    let run = run_status_by_model(run_results);
    let status = run.get(&model)?;
    let title = match status.status.as_str() {
        "success" | "pass" => format!("last run: OK in {:.1} s", status.execution_time),
        "skipped" => "last run: skipped".to_string(),
        other => format!("last run: {}", other.to_uppercase()),
    };
    Some(CodeLens {
        range: Range::default(),
        command: Command {
            title,
            command: String::new(),
            arguments: Vec::new(),
        },
    })
}

/// Run failures, failed tests and manifest drift for one URI: a model
/// file gets its own run/test/drift findings at the file head; a
/// properties YAML gets each failing model's findings at its entry.
pub fn artifact_diagnostics(ctx: &DbtContext, uri: &str, set: &ArtifactSet) -> Vec<Diagnostic> {
    if uri.ends_with(".yml") || uri.ends_with(".yaml") {
        return schema_file_diagnostics(ctx, uri, set);
    }
    model_file_diagnostics(ctx, uri, set)
}

fn model_file_diagnostics(ctx: &DbtContext, uri: &str, set: &ArtifactSet) -> Vec<Diagnostic> {
    let Some(model) = model_name_from_uri(uri) else {
        return Vec::new();
    };
    if !ctx.model_detail_map.contains_key(&model) {
        return Vec::new();
    }
    let mut out = Vec::new();
    if let Some(run_results) = &set.run_results {
        let run = run_status_by_model(run_results);
        if let Some(status) = run.get(&model) {
            if matches!(status.status.as_str(), "error" | "fail") {
                let guidance = Guidance::new(with_reason(
                    format!("the last dbt run of '{model}' failed"),
                    &status.message,
                ));
                out.push(Diagnostic::plain(
                    Range::default(),
                    guidance
                        .fix(format!("fix and re-run: dbt run --select {model}"))
                        .to_diagnostic(),
                    diagnostic_severity::WARNING,
                    "artifact-run-failed",
                    "cogitator",
                ));
            }
        }
        for failure in test_failures(ctx, run_results, set.manifest.as_deref()) {
            if failure.model != model {
                continue;
            }
            let guidance = Guidance::new(with_reason(
                format!("test '{}' failed", failure.test_name),
                &failure.message,
            ));
            out.push(Diagnostic::plain(
                Range::default(),
                guidance.to_diagnostic(),
                diagnostic_severity::WARNING,
                "artifact-test-failed",
                "cogitator",
            ));
        }
    }
    if let Some(manifest) = &set.manifest {
        let drifted = manifest_drift(ctx, manifest)
            .into_iter()
            .any(|d| d.model == model && d.kind == DriftKind::OnlyInScan);
        if drifted {
            let mut diagnostic = Diagnostic::plain(
                Range::default(),
                Guidance::new(format!(
                    "'{model}' is missing from manifest.json (stale target/?)"
                ))
                .fix("re-run dbt parse (or any dbt command) to refresh it.")
                .to_diagnostic(),
                diagnostic_severity::HINT,
                "artifact-drift",
                "cogitator",
            );
            if let Some(root) = ctx.project_root.as_deref() {
                let manifest_path = ArtifactPaths::under(root).manifest;
                diagnostic.related_information =
                    Some(vec![cogitator_lsp::DiagnosticRelatedInformation {
                        location: Location {
                            uri: cogitator_lsp::fs_path_to_uri(&manifest_path),
                            range: Range::default(),
                        },
                        message: "the stale manifest".to_string(),
                    }]);
            }
            out.push(diagnostic);
        }
    }
    out
}

/// Failed tests shown where the tests are declared: at each model's
/// entry in THIS properties file.
fn schema_file_diagnostics(ctx: &DbtContext, uri: &str, set: &ArtifactSet) -> Vec<Diagnostic> {
    let Some(run_results) = &set.run_results else {
        return Vec::new();
    };
    let path = cogitator_lsp::uri_to_fs_path(uri).unwrap_or_else(|| uri.to_string());
    let path = std::path::Path::new(&path);
    let mut out = Vec::new();
    for failure in test_failures(ctx, run_results, set.manifest.as_deref()) {
        let Some(details) = ctx.model_detail_map.get(&failure.model) else {
            continue;
        };
        if details.schema_uri.as_deref() != Some(path) {
            continue;
        }
        let guidance = Guidance::new(with_reason(
            format!("test '{}' of '{}' failed", failure.test_name, failure.model),
            &failure.message,
        ));
        out.push(Diagnostic::plain(
            details.schema_range,
            guidance.to_diagnostic(),
            diagnostic_severity::WARNING,
            "artifact-test-failed",
            "cogitator",
        ));
    }
    out
}
