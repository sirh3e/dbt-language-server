//! The guard against a partial rename: every place the document's own
//! token stream writes the name as a relation (after FROM / JOIN) or as a
//! qualifier (`name.`) must be one the binder resolved. One that is not —
//! in a blanked `else` branch, or a qualified column whose name is Jinja
//! (`o.{{ col }}`) — makes the rename a refusal instead of a guess. Bare
//! words of the same name are columns and are not counted.

use cogitator_dialects::Dialect;
use cogitator_ir::Span;
use cogitator_parser::{ParsedDocument, TokenKind};
use sqlparser::keywords::Keyword;

use super::{SqlKind, SqlLocal};
use crate::features::rename_local::occurrences::unquoted;

/// The line of the first written occurrence the binder did not resolve.
pub(crate) fn missed(doc: &ParsedDocument, local: &SqlLocal, dialect: &Dialect) -> Option<u32> {
    let covered = |s: Span| s == local.definition || local.uses.contains(&s);
    let in_reach =
        |s: Span| local.kind == SqlKind::Cte || local.scopes.iter().any(|scope| scope.contains(s));
    let tokens = &doc.tokens;
    tokens.iter().enumerate().find_map(|(i, t)| {
        if !matches!(t.kind, TokenKind::Word(Keyword::NoKeyword)) {
            return None;
        }
        let word = unquoted(doc.literal(t));
        if cogitator_sql_front::fold_schema_name(word, dialect) != local.folded {
            return None;
        }
        let qualifier = tokens
            .get(i + 1)
            .is_some_and(|n| n.kind == TokenKind::Symbol('.') && n.start == t.end);
        let relation = local.kind == SqlKind::Cte
            && i.checked_sub(1)
                .and_then(|p| tokens.get(p))
                .is_some_and(|p| matches!(p.kind, TokenKind::Word(Keyword::FROM | Keyword::JOIN)));
        let span = Span::new(t.start, t.end);
        ((qualifier || relation) && in_reach(span) && !covered(span)).then_some(t.line)
    })
}
