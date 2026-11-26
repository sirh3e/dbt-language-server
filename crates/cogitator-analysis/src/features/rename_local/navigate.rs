//! Go to definition, references and highlights on a name local to one
//! file: the targets of [`super::target_at`] (Jinja locals, CTEs, table
//! aliases) and, after them, SQL column aliases — a select-list `… as
//! col` or a CTE column name, with every place the binder resolved to it.
//! Column aliases are navigation only: rename leaves them to
//! `rename_column`, and so do references where the alias is a column of
//! the model's output (its walk follows it into every consumer), and
//! definition on the alias itself (which goes on to its YAML entry).

use cogitator_ir::Span;
use cogitator_lsp::{DocumentHighlight, Location, Position};

use super::occurrences::{self, Occurrences, Role};
use super::sql::{sql_locals, SqlLocals};
use super::{
    is_macro_file, is_sql_file, jinja, names, range_of, DbtContext, Document, Reader, Target,
};

/// What the cursor names, for navigation.
enum Nav {
    Local(Target),
    Column(SqlLocals, usize),
}

fn nav_at(ctx: &DbtContext, doc: &Document, uri: &str, pos: Position) -> Option<Nav> {
    if !is_sql_file(uri) {
        return None;
    }
    let offset = names::offset_of(&doc.text, pos)?;
    if let Some(analysis) = jinja::analyze(&doc.text) {
        if let Some(idx) = analysis.at(offset) {
            return Some(Nav::Local(Target::Jinja(analysis, idx)));
        }
    }
    if is_macro_file(ctx, uri) {
        return None;
    }
    // SQL the binder cannot read names nothing here: the caller falls
    // through to the token-based features
    let locals = sql_locals(&doc.text, &ctx.dialect).ok()?;
    if let Some(idx) = locals.at(offset) {
        return Some(Nav::Local(Target::Sql(locals, idx)));
    }
    let idx = locals.column_at(offset)?;
    Some(Nav::Column(locals, idx))
}

/// The occurrences in the file, definitions first by position.
fn spans(nav: &Nav) -> Vec<(Span, Role)> {
    match nav {
        Nav::Local(target) => occurrences::here(target),
        Nav::Column(locals, idx) => {
            let c = &locals.columns[*idx];
            let mut out: Vec<(Span, Role)> = std::iter::once((c.definition, Role::Definition))
                .chain(c.uses.iter().map(|s| (*s, Role::Use)))
                .collect();
            out.sort_by_key(|(s, _)| *s);
            out
        }
    }
}

/// Whether the cursor is in `span`.
fn on(doc: &Document, span: Span, pos: Position) -> bool {
    names::offset_of(&doc.text, pos).is_some_and(|o| span.start <= o && o <= span.end)
}

/// Whether the cursor names a column of the model's output, which the
/// project-wide column references follow.
fn is_model_column(ctx: &DbtContext, doc: &Document, uri: &str, pos: Position) -> bool {
    crate::features::rename_column::column_at(ctx, doc, uri, pos).is_some()
}

/// textDocument/definition on a SQL local: where the CTE, the table alias
/// or the column alias under the cursor is declared, in its own scope.
/// `None` off such a name, and on a Jinja local (whose binding is the
/// Jinja features' to answer).
pub fn local_definition(
    ctx: &DbtContext,
    doc: &Document,
    uri: &str,
    pos: Position,
) -> Option<Location> {
    let nav = nav_at(ctx, doc, uri, pos)?;
    let (span, _) = spans(&nav)
        .into_iter()
        .find(|(_, role)| *role == Role::Definition)?;
    match &nav {
        Nav::Local(Target::Jinja(..)) => return None,
        Nav::Column(..) if on(doc, span, pos) => return None,
        _ => {}
    }
    Some(Location {
        uri: uri.to_string(),
        range: range_of(&doc.text, span),
    })
}

/// textDocument/references on a local name.
pub fn local_references(
    ctx: &DbtContext,
    doc: Option<&Document>,
    uri: &str,
    pos: Position,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    let doc = doc?;
    let nav = nav_at(ctx, doc, uri, pos)?;
    let (here, elsewhere) = match &nav {
        Nav::Local(target) => {
            let reader = Reader::disk(Some((uri, &doc.text)));
            let Occurrences { here, elsewhere } =
                occurrences::collect(ctx, &reader, doc, uri, target, None).ok()?;
            (here, elsewhere)
        }
        Nav::Column(..) if is_model_column(ctx, doc, uri, pos) => return None,
        Nav::Column(..) => (spans(&nav), Default::default()),
    };
    let mut out: Vec<Location> = here
        .into_iter()
        .filter(|(_, role)| include_declaration || *role == Role::Use)
        .map(|(span, _)| Location {
            uri: uri.to_string(),
            range: range_of(&doc.text, span),
        })
        .collect();
    for (path, ranges) in elsewhere {
        let uri = crate::features::file_uri(&path);
        out.extend(ranges.into_iter().map(|range| Location {
            uri: uri.clone(),
            range,
        }));
    }
    Some(out)
}

/// textDocument/documentHighlight on a local name: bindings are writes,
/// uses reads.
pub fn local_highlights(
    ctx: &DbtContext,
    doc: Option<&Document>,
    uri: &str,
    pos: Position,
) -> Option<Vec<DocumentHighlight>> {
    let doc = doc?;
    let nav = nav_at(ctx, doc, uri, pos)?;
    Some(
        spans(&nav)
            .into_iter()
            .map(|(span, role)| DocumentHighlight {
                range: range_of(&doc.text, span),
                kind: match role {
                    Role::Definition => 3,
                    Role::Use => 2,
                },
            })
            .collect(),
    )
}
