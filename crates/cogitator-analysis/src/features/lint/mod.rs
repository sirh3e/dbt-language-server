//! Opt-in style diagnostics (03 §10): every check defaults OFF and is
//! enabled per project through cogitator.yml `lint:`. Each check is a
//! small pure module over the token stream; nothing here reads files.

mod select_star;
mod trailing_comma;
mod unaliased;
mod undocumented;

use cogitator_lsp::Diagnostic;

use super::{DbtContext, Document};
use crate::config_schema::lint_flags;
use crate::lineage::ModelLineage;

/// Which checks run — mirrors the `lint:` section / the shared bitset.
#[derive(Debug, Clone, Copy, Default)]
pub struct LintOptions {
    pub select_star: bool,
    pub unaliased_columns: bool,
    pub trailing_comma: bool,
    pub undocumented_columns: bool,
}

impl LintOptions {
    pub fn from_flags(flags: u32) -> Self {
        LintOptions {
            select_star: flags & lint_flags::SELECT_STAR != 0,
            unaliased_columns: flags & lint_flags::UNALIASED_COLUMNS != 0,
            trailing_comma: flags & lint_flags::TRAILING_COMMA != 0,
            undocumented_columns: flags & lint_flags::UNDOCUMENTED_COLUMNS != 0,
        }
    }

    pub fn any(&self) -> bool {
        self.select_star
            || self.unaliased_columns
            || self.trailing_comma
            || self.undocumented_columns
    }
}

/// Run the enabled checks over one SQL document. `lineage` powers the
/// undocumented-columns check only (the push path passes None — the
/// pull path computes it through the derived cache).
pub fn lint(
    ctx: &DbtContext,
    doc: &Document,
    uri: &str,
    opts: LintOptions,
    lineage: Option<&ModelLineage>,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if opts.select_star {
        out.extend(select_star::check(ctx, doc, uri));
    }
    if opts.unaliased_columns {
        out.extend(unaliased::check(doc));
    }
    if opts.trailing_comma {
        out.extend(trailing_comma::check(ctx, doc));
    }
    if opts.undocumented_columns {
        if let Some(lineage) = lineage {
            out.extend(undocumented::check(ctx, doc, uri, lineage));
        }
    }
    out
}

/// Staging models are exempt from `select_star` by convention: a
/// `staging/` path segment or an `stg_` file-name prefix.
pub(super) fn is_staging(uri: &str) -> bool {
    let path = cogitator_lsp::uri_to_fs_path(uri).unwrap_or_else(|| uri.to_string());
    let path = std::path::Path::new(&path);
    let in_staging_dir = path
        .components()
        .any(|c| c.as_os_str().to_str() == Some("staging"));
    let stg_prefix = path
        .file_stem()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with("stg_"));
    in_staging_dir || stg_prefix
}
