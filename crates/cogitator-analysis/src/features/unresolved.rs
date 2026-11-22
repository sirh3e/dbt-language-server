//! `dbt.unresolved` (FEATURE_REQUESTS, editor experience): every
//! unresolved `ref`, `source`, source table, `doc`, macro and `var` of the
//! project in one list. Built from the per-document truth the diagnostics
//! already compute (`dangling_ref_diagnostics`, `dangling_doc_diagnostics`,
//! `citizen_diagnostics` — codes are the contract), plus macro calls that
//! no package defines and dbt does not provide.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cogitator_lsp::{Diagnostic, Range};
use cogitator_parser::{DocumentParser, JinjaSqlParser, TokenKind};
use serde::Serialize;

use super::unresolved_builtins::is_dbt_builtin;
use super::{
    citizen_diagnostics, dangling_doc_diagnostics, dangling_ref_diagnostics, file_uri, token_range,
    DbtContext, Document,
};
use crate::graph::NodeKind;

/// What kind of name does not resolve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnresolvedKind {
    Ref,
    Source,
    SourceTable,
    Doc,
    Macro,
    Var,
}

impl UnresolvedKind {
    /// The kind behind a diagnostic code of the dangling checks.
    fn of_code(code: &str) -> Option<Self> {
        Some(match code {
            "dangling-ref" => Self::Ref,
            "dangling-source" => Self::Source,
            "dangling-source-table" => Self::SourceTable,
            "dangling-doc" => Self::Doc,
            "unknown-var" => Self::Var,
            "dangling-exposure-ref" => Self::Ref,
            _ => return None,
        })
    }
}

/// One unresolved name and where it is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Unresolved {
    pub kind: UnresolvedKind,
    pub name: String,
    pub uri: String,
    pub range: Range,
    pub message: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub suggestions: Vec<String>,
}

/// The literal of the token a diagnostic points at, else the last quoted
/// name of its message (the citizen checks point at the entry).
fn name_at(doc: Option<&Document>, d: &Diagnostic) -> String {
    doc.and_then(|doc| {
        doc.parsed
            .tokens
            .iter()
            .find(|t| token_range(t) == d.range)
            .map(|t| doc.parsed.literal(t).to_string())
    })
    .or_else(|| {
        let first_line = d.message.lines().next().unwrap_or_default();
        let end = first_line.rfind('\'')?;
        let start = first_line[..end].rfind('\'')?;
        Some(first_line[start + 1..end].to_string())
    })
    .unwrap_or_default()
}

fn from_diagnostics(doc: Option<&Document>, uri: &str, found: Vec<Diagnostic>) -> Vec<Unresolved> {
    found
        .into_iter()
        .filter_map(|d| {
            let kind = UnresolvedKind::of_code(&d.code)?;
            Some(Unresolved {
                kind,
                name: name_at(doc, &d),
                uri: uri.to_string(),
                range: d.range,
                message: d.message.lines().next().unwrap_or_default().to_string(),
                suggestions: d.data.map(|data| data.suggestions).unwrap_or_default(),
            })
        })
        .collect()
}

/// `{{ name(...) }}` calls no package defines and dbt does not provide.
/// A call qualified with a package the project does not have (`dbt.`,
/// `adapter.`) is not judged; nothing is while no macro is known yet.
fn unresolved_macros(ctx: &DbtContext, doc: &Document, uri: &str) -> Vec<Unresolved> {
    if ctx.macro_detail_map.is_empty() {
        return Vec::new();
    }
    let parsed = &doc.parsed;
    parsed
        .tokens
        .iter()
        .enumerate()
        .filter(|(_, t)| t.kind == TokenKind::Macro)
        .filter_map(|(idx, t)| {
            let name = parsed.literal(t);
            let missing = match parsed.lookback_find(idx, TokenKind::Package, 2) {
                Some(package) => ctx
                    .macro_detail_map
                    .get(parsed.literal(package))
                    .is_some_and(|m| !m.contains_key(name)),
                None => {
                    !is_dbt_builtin(name)
                        && !ctx.macro_detail_map.values().any(|m| m.contains_key(name))
                }
            };
            missing.then(|| Unresolved {
                kind: UnresolvedKind::Macro,
                name: name.to_string(),
                uri: uri.to_string(),
                range: token_range(t),
                message: format!("macro '{name}' is not defined in the project or its packages"),
                suggestions: Vec::new(),
            })
        })
        .collect()
}

fn is_yaml(path: &str) -> bool {
    path.ends_with(".yml") || path.ends_with(".yaml")
}

/// The unresolved names of one document: the SQL checks for SQL, the doc
/// and citizen checks for property YAML, as the diagnostics split them.
pub fn document_unresolved(ctx: &DbtContext, uri: &str, doc: &Document) -> Vec<Unresolved> {
    let path = cogitator_lsp::uri_to_fs_path(uri).unwrap_or_else(|| uri.to_string());
    let mut out = if is_yaml(&path) {
        let mut found = dangling_doc_diagnostics(ctx, doc);
        found.extend(citizen_diagnostics(ctx, uri));
        from_diagnostics(Some(doc), uri, found)
    } else {
        let mut out = from_diagnostics(Some(doc), uri, dangling_ref_diagnostics(ctx, doc));
        out.extend(unresolved_macros(ctx, doc, uri));
        out
    };
    out.sort_by_key(|u| (u.range.start.line, u.range.start.character, u.kind));
    out
}

/// The root project's files: its models, snapshots, macros, tests and
/// analyses, and the YAML that describes them, path-sorted.
pub fn project_files(ctx: &DbtContext) -> Vec<PathBuf> {
    let own = ctx.project.name.as_str();
    let mut files: BTreeSet<PathBuf> = BTreeSet::new();
    for model in ctx.model_detail_map.values() {
        if model.project_name == own {
            files.insert(model.uri.clone());
            files.extend(model.schema_uri.clone());
        }
    }
    for m in ctx
        .macro_detail_map
        .get(own)
        .into_iter()
        .flat_map(|m| m.values())
    {
        files.insert(m.uri.clone());
    }
    files.extend(
        ctx.graph
            .nodes()
            .filter(|n| matches!(n.kind, NodeKind::Test | NodeKind::Analysis))
            .map(|n| n.uri.clone()),
    );
    files.extend(ctx.source_detail_map.values().map(|s| s.uri.clone()));
    files.extend(
        ctx.exposure_detail_map
            .values()
            .chain(ctx.metric_detail_map.values())
            .chain(ctx.semantic_model_detail_map.values())
            .map(|c| c.uri.clone()),
    );
    files.retain(|p| !p.as_os_str().is_empty() && !is_package_path(ctx, p));
    files.into_iter().collect()
}

/// A file under the project's installed packages.
fn is_package_path(ctx: &DbtContext, path: &Path) -> bool {
    let Some(root) = ctx.project_root.as_deref() else {
        return false;
    };
    path.strip_prefix(root)
        .ok()
        .and_then(|rel| rel.components().next())
        .is_some_and(|c| c.as_os_str() == "dbt_packages" || c.as_os_str() == "dbt_modules")
}

/// Every unresolved name of the project, by file then position. `text` is
/// the shell's seam: an open document's text, else the file on disk.
pub fn project_unresolved(
    ctx: &DbtContext,
    text: &dyn Fn(&Path) -> Option<Arc<str>>,
) -> Vec<Unresolved> {
    let mut out = Vec::new();
    for path in project_files(ctx) {
        let Some(text) = text(&path) else { continue };
        let Ok(parsed) = JinjaSqlParser.parse(&text, &ctx.dialect) else {
            continue;
        };
        let uri = file_uri(&path);
        let doc = Document {
            uri: std::sync::Arc::from(uri.as_str()),
            text,
            parsed,
            generation: 0,
        };
        out.extend(document_unresolved(ctx, &uri, &doc));
    }
    out
}
