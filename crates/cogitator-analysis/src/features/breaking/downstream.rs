//! What a removed or retyped column breaks: its own tests, the
//! downstream columns whose lineage reads it (dbt.impact's column
//! closure), and whether those are tested, contracted or public.

use std::collections::HashMap;

use cogitator_lsp::{Position, Range, TextEdit};
use serde::Serialize;

use super::anchor::Anchors;
use super::SchemaDiff;
use crate::features::column_reach::{self, Corpus, MetricRead, SemanticRead};
use crate::features::DbtContext;
use crate::lineage::ModelLineage;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ChangeKind {
    Removed,
    Retyped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    Error,
    Warning,
    Information,
}

/// A downstream column fed by the changed column.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownstreamUse {
    pub model: String,
    pub column: String,
    /// Column tests on the downstream column.
    pub tests: Vec<String>,
    /// `contract.enforced` on the downstream model.
    pub contracted: bool,
    /// `access: public` on the downstream model.
    pub public: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnImpact {
    pub column: String,
    pub change: ChangeKind,
    pub severity: Severity,
    /// Column tests on the changed column itself.
    pub tests: Vec<String>,
    pub downstream: Vec<DownstreamUse>,
    /// Semantic-model items over the model or a downstream model that
    /// read the column (empty without a properties corpus).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub semantic: Vec<SemanticRead>,
    /// The metrics over those items' measures.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub metrics: Vec<MetricRead>,
    /// Where in the model's SQL the change shows (`anchor.rs`); absent
    /// when the SQL gives no exact place.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<Range>,
    /// "Keep '<old>' as an alias": re-add the removed name beside its
    /// likely replacement, when that is a bare column or literal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_alias: Option<TextEdit>,
    /// What reads it, each at its place: the diagnostic's related
    /// information. Not on the wire (the fields above say it).
    #[serde(skip)]
    pub related: Vec<Related>,
}

/// One place that relies on a changed column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Related {
    pub uri: String,
    pub range: Range,
    pub message: String,
}

/// What `impacts` reads besides the lineages.
#[derive(Default)]
pub struct Extras<'a> {
    pub anchors: Anchors,
    pub corpus: Option<&'a Corpus>,
}

pub(super) fn impacts(
    ctx: &DbtContext,
    model: &str,
    diff: &SchemaDiff,
    lineages: &HashMap<String, ModelLineage>,
    extras: &Extras<'_>,
) -> Vec<ColumnImpact> {
    let public = |name: &str| {
        ctx.model_detail_map
            .get(name)
            .is_some_and(|d| d.access.as_deref() == Some("public"))
    };
    let model_public = public(model);
    let graph = crate::features::impact_graph(lineages);
    let closure = crate::features::impact_over(ctx, model, None, lineages, &graph);
    let has_exposures = !closure.exposures.is_empty();
    let changed = diff
        .removed
        .iter()
        .map(|r| {
            (
                r.name.as_str(),
                ChangeKind::Removed,
                r.possibly_renamed_to.as_deref(),
            )
        })
        .chain(
            diff.retyped
                .iter()
                .map(|t| (t.name.as_str(), ChangeKind::Retyped, None)),
        );
    changed
        .map(|(column, change, renamed_to)| {
            let report = crate::features::impact_over(ctx, model, Some(column), lineages, &graph);
            let reach = extras
                .corpus
                .and_then(|corpus| column_reach::reach(ctx, &report, corpus, None))
                .unwrap_or_default();
            let downstream: Vec<DownstreamUse> = report
                .column_hits
                .into_iter()
                .map(|hit| DownstreamUse {
                    tests: column_tests(ctx, &hit.model, &hit.column),
                    contracted: ctx
                        .model_detail_map
                        .get(&hit.model)
                        .is_some_and(|d| d.contract_enforced),
                    public: public(&hit.model),
                    model: hit.model,
                    column: hit.column,
                })
                .collect();
            let tests = column_tests(ctx, model, column);
            let read_outside = model_public || has_exposures || !reach.semantic_items.is_empty();
            let (range, keep_alias) = place(&extras.anchors, change, column, renamed_to);
            let mut impact = ColumnImpact {
                severity: severity(change, &tests, &downstream, read_outside),
                column: column.to_string(),
                change,
                tests,
                downstream,
                semantic: reach.semantic_items,
                metrics: reach.metrics,
                range,
                keep_alias,
                related: Vec::new(),
            };
            impact.related = related(ctx, model, &impact, &closure.exposures);
            impact
        })
        .collect()
}

/// The anchor and, for a likely rename, the alias fix (module docs of
/// `anchor.rs` say which place is chosen and why).
fn place(
    anchors: &Anchors,
    change: ChangeKind,
    column: &str,
    renamed_to: Option<&str>,
) -> (Option<Range>, Option<TextEdit>) {
    match change {
        ChangeKind::Retyped => (anchors.item(column).map(|a| a.range), None),
        ChangeKind::Removed => match renamed_to.and_then(|to| anchors.item(to)) {
            Some(item) => {
                let fix = item.expr.as_ref().map(|expr| TextEdit {
                    range: Range {
                        start: item.range.end,
                        end: item.range.end,
                    },
                    new_text: format!(", {expr} as {column}"),
                });
                (Some(item.range), fix)
            }
            None => (anchors.select(), None),
        },
    }
}

/// Every place that relies on the column, for the related information.
fn related(
    ctx: &DbtContext,
    model: &str,
    impact: &ColumnImpact,
    exposures: &[String],
) -> Vec<Related> {
    let at = |path: &std::path::Path, range: Range| (cogitator_lsp::fs_path_to_uri(path), range);
    let column_place = |model: &str, column: &str| {
        let details = ctx.model_detail_map.get(model)?;
        let entry = details
            .columns
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(column));
        match (details.schema_uri.as_deref(), entry) {
            (Some(schema), Some(entry)) => {
                let start = Position {
                    line: entry.name_pos.line,
                    character: entry.name_pos.character,
                };
                Some(at(schema, Range { start, end: start }))
            }
            _ => Some(at(&details.uri, Range::default())),
        }
    };
    let mut out = Vec::new();
    let mut push = |place: Option<(String, Range)>, message: String| {
        if let Some((uri, range)) = place {
            out.push(Related {
                uri,
                range,
                message,
            });
        }
    };
    for test in &impact.tests {
        push(
            column_place(model, &impact.column),
            format!("test {test} on {model}.{}", impact.column),
        );
    }
    for d in &impact.downstream {
        let mut tags = Vec::new();
        if d.contracted {
            tags.push("contracted".to_string());
        }
        if d.public {
            tags.push("public".to_string());
        }
        if !d.tests.is_empty() {
            tags.push(format!("tested: {}", d.tests.join(", ")));
        }
        let tags = match tags.is_empty() {
            true => String::new(),
            false => format!(" ({})", tags.join("; ")),
        };
        push(
            column_place(&d.model, &d.column),
            format!("read by {}.{}{tags}", d.model, d.column),
        );
    }
    for s in &impact.semantic {
        let what = s.name.as_deref().unwrap_or("expr");
        push(
            Some((s.uri.clone(), s.range)),
            format!("semantic model {}: {what}", s.semantic_model),
        );
    }
    for m in &impact.metrics {
        push(
            Some((m.uri.clone(), m.range)),
            format!("metric {} (via {})", m.name, m.via),
        );
    }
    for name in exposures {
        let place = ctx
            .exposure_detail_map
            .get(name)
            .or_else(|| ctx.metric_detail_map.get(name))
            .or_else(|| ctx.semantic_model_detail_map.get(name))
            .map(|c| at(&c.uri, c.range));
        push(place, format!("{name} depends on {model} (node level)"));
    }
    out
}

/// Error only for a removed column a test or a contracted downstream
/// relies on; warning when anything reads it (downstream columns,
/// external consumers of a public model, exposures); information else.
pub(super) fn severity(
    change: ChangeKind,
    tests: &[String],
    downstream: &[DownstreamUse],
    consumed_outside: bool,
) -> Severity {
    let relied_on = !tests.is_empty()
        || downstream
            .iter()
            .any(|d| d.contracted || !d.tests.is_empty());
    if change == ChangeKind::Removed && relied_on {
        return Severity::Error;
    }
    if relied_on || !downstream.is_empty() || consumed_outside {
        return Severity::Warning;
    }
    Severity::Information
}

use crate::features::column_reach::column_tests;
