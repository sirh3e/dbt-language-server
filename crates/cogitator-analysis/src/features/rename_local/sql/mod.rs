//! The SQL side of local renames: CTE names and table aliases, read off
//! the bound HIR of cogitator-sql-front (its binder resolves every
//! `from x`, `join x` and `x.col`), never off the text. Jinja is masked by
//! the front end; `if` / `for` control flow is flattened first ([`mask`])
//! so a model with control flow still binds, and [`missed`] refuses a
//! rename when a written occurrence is not among the bound ones.

mod collect;
mod complete;
pub(crate) mod mask;

#[cfg(test)]
mod tests;

pub(crate) use collect::{SqlKind, SqlLocal, SqlLocals};
pub(crate) use complete::missed;

use cogitator_dialects::Dialect;
use cogitator_ir::common::ModelConfig;
use cogitator_ir::origin::{SourceOrigin, TemplateVariant};
use cogitator_ir::DegradationKind;
use cogitator_sql_front::{build_hir, HirInput, SchemaEnv};

/// Why the model's SQL could not be bound.
pub(crate) fn unreadable(detail: &str) -> String {
    format!(
        "the SQL of this model could not be read as one query ({detail}); \
         a CTE or alias is renamed only when every use of it is known."
    )
}

/// The CTEs and aliases of a model, or why they cannot be known.
pub(crate) fn sql_locals(text: &str, dialect: &Dialect) -> Result<SqlLocals, String> {
    let masked =
        mask::without_control_tags(text).ok_or_else(|| unreadable("the Jinja does not lex"))?;
    let input = HirInput {
        text: &masked,
        dialect,
        origin: SourceOrigin::Template {
            variant: TemplateVariant::Plain,
        },
        config: ModelConfig::default(),
    };
    let lowered = build_hir(&input, &SchemaEnv::default());
    if let Some(d) = lowered.degraded.iter().find(|d| {
        matches!(
            d.kind,
            DegradationKind::ParseHole
                | DegradationKind::CapExceeded
                | DegradationKind::JinjaControlFlow
        )
    }) {
        return Err(unreadable(&d.detail));
    }
    let hir = lowered
        .value
        .ok_or_else(|| unreadable("the front end declined it"))?;
    Ok(collect::collect(&hir, text))
}

fn touches(span: cogitator_ir::Span, offset: u32) -> bool {
    span.start <= offset && offset <= span.end
}

impl SqlLocals {
    /// The CTE or alias whose definition or use is at `offset`.
    pub fn at(&self, offset: u32) -> Option<usize> {
        self.locals.iter().position(|l| {
            touches(l.definition, offset) || l.uses.iter().any(|s| touches(*s, offset))
        })
    }

    /// Why renaming local `idx` to the folded name `new` would change
    /// what the query reads, if it would. A CTE may not take the name of
    /// another CTE, of an alias or of a table the model reads; an alias
    /// not the name of another FROM item where it is visible, nor a
    /// qualifier already used there.
    pub fn collision(&self, idx: usize, new: &str) -> Option<String> {
        let local = &self.locals[idx];
        let other = |l: &&SqlLocal| !std::ptr::eq(*l, local) && l.folded == new;
        match local.kind {
            SqlKind::Cte => {
                if self
                    .locals
                    .iter()
                    .filter(other)
                    .any(|l| l.kind == SqlKind::Cte)
                {
                    return Some(format!("a CTE named `{new}` already exists in this model."));
                }
                if self
                    .locals
                    .iter()
                    .filter(other)
                    .any(|l| l.kind == SqlKind::Alias)
                {
                    return Some(format!("`{new}` is already a table alias in this model."));
                }
                self.physical
                    .iter()
                    .any(|p| p == new)
                    .then(|| format!("the model already reads a relation named `{new}`."))
            }
            SqlKind::Alias => {
                let shares =
                    |spans: &[cogitator_ir::Span]| spans.iter().any(|s| local.scopes.contains(s));
                let named = self
                    .from_names
                    .iter()
                    .any(|(name, spans)| name == new && shares(spans));
                let qualified = self
                    .qualifiers
                    .iter()
                    .any(|(name, at)| name == new && local.scopes.iter().any(|s| s.contains(*at)));
                (named || qualified).then(|| {
                    format!(
                        "`{new}` already names a relation where `{}` is visible.",
                        local.folded
                    )
                })
            }
        }
    }
}
