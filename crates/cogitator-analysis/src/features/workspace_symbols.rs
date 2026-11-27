//! workspace/symbol: everything a person may look for by name, one flat
//! list a client's symbol picker filters (Zed's cmd-T, Neovim's
//! `:DbtFind`). Query prefixes pick one kind: `tag:`, `selector:`,
//! `exposure:`, `column:` and `test:`; a plain query lists definitions
//! (selectors, models, sources and their tables, macros, exposures,
//! metrics, semantic models) and, once it is not empty, documented
//! columns and tests too. The symbol kinds are listed in
//! [`cogitator_lsp::symbol_kind`]; the parts that read the YAML are in
//! [`super::symbol_parts`].

use std::path::Path;

use cogitator_lsp::{symbol_kind, Location, Range, SymbolInformation};

use super::symbol_parts::{column_symbols, source_table_symbols, test_symbols};
use super::{file_uri, DbtContext};

/// How many columns and tests one answer carries at most; the
/// definitions are never cut.
pub const MAX_PART_SYMBOLS: usize = 2000;

/// workspace/symbol over a universe built from the context alone; tests
/// are read from disk.
pub fn workspace_symbols(ctx: &DbtContext, query: &str) -> Vec<SymbolInformation> {
    let universe = crate::selectors::universe(ctx, &|_| None);
    workspace_symbols_in(ctx, &universe, query)
}

/// [`workspace_symbols`] over a given selector universe, reading the
/// properties YAML from disk.
pub fn workspace_symbols_in(
    ctx: &DbtContext,
    universe: &crate::selectors::Universe,
    query: &str,
) -> Vec<SymbolInformation> {
    workspace_symbols_with(ctx, universe, query, &|p| std::fs::read_to_string(p).ok())
}

/// [`workspace_symbols_in`] with the file reads given (the server passes
/// the open documents' text before the disk).
pub fn workspace_symbols_with(
    ctx: &DbtContext,
    universe: &crate::selectors::Universe,
    query: &str,
    read: &dyn Fn(&Path) -> Option<String>,
) -> Vec<SymbolInformation> {
    if let Some(tag) = query.strip_prefix("tag:") {
        return tagged(universe, tag.trim());
    }
    let lower = query.to_lowercase();
    let matches = |q: &str| {
        let q = q.trim().to_string();
        move |name: &str| q.is_empty() || name.to_lowercase().contains(&q)
    };
    if let Some(q) = lower.strip_prefix("selector:") {
        let mut out = selectors(ctx, &matches(q));
        out.dedup_by(|a, b| a.name == b.name);
        return out;
    }
    if let Some(q) = lower.strip_prefix("exposure:") {
        return sorted(citizens(ctx, &matches(q)));
    }
    if let Some(q) = lower.strip_prefix("column:") {
        return finished(column_symbols(ctx, &matches(q)), Vec::new());
    }
    if let Some(q) = lower.strip_prefix("test:") {
        return finished(test_symbols(ctx, &matches(q), read), Vec::new());
    }

    let m = matches(&lower);
    let mut defs = selectors(ctx, &m);
    defs.extend(models(ctx, &m));
    defs.extend(sources(ctx, &m));
    defs.extend(source_table_symbols(ctx, &m));
    defs.extend(macros(ctx, &m));
    defs.extend(citizens(ctx, &m));
    // an empty query lists the definitions alone: fast, and what the
    // pickers that ask for everything (`:DbtFind`, `:DbtMacros`) read
    let mut parts = Vec::new();
    if !lower.trim().is_empty() {
        parts = column_symbols(ctx, &m);
        parts.extend(test_symbols(ctx, &m, read));
    }
    finished(parts, defs)
}

/// The definitions whole, the columns and tests cut to
/// [`MAX_PART_SYMBOLS`], all by name and once per file.
fn finished(
    mut parts: Vec<SymbolInformation>,
    defs: Vec<SymbolInformation>,
) -> Vec<SymbolInformation> {
    parts.sort_by(|a, b| a.name.cmp(&b.name));
    parts.truncate(MAX_PART_SYMBOLS);
    let mut out = defs;
    out.extend(parts);
    out.sort_by(|a, b| a.name.cmp(&b.name));
    dedup_same_file(&mut out);
    out
}

fn sorted(mut out: Vec<SymbolInformation>) -> Vec<SymbolInformation> {
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

pub(super) fn symbol(
    name: String,
    kind: i32,
    path: &Path,
    range: Range,
    container: Option<String>,
) -> SymbolInformation {
    SymbolInformation {
        name,
        kind,
        location: Location {
            uri: file_uri(path),
            range,
        },
        container_name: container,
    }
}

/// `tag:x` → nodes carrying a tag that contains x (a glob as written);
/// tags compare as dbt does: case-sensitive. A bare prefix lists every
/// tagged node.
fn tagged(universe: &crate::selectors::Universe, tag: &str) -> Vec<SymbolInformation> {
    use crate::selectors::{self, glob, Indirect, ResourceType};
    let pattern = if glob::is_glob(tag) {
        tag.to_string()
    } else {
        format!("*{tag}*")
    };
    let Ok(eval) = selectors::select(universe, &[], &format!("tag:{pattern}"), Indirect::Empty)
    else {
        return Vec::new();
    };
    sorted(
        selectors::sorted(universe, &eval)
            .into_iter()
            .map(|n| {
                let kind = match n.resource_type {
                    ResourceType::Source => symbol_kind::INTERFACE,
                    ResourceType::Test | ResourceType::UnitTest => symbol_kind::FUNCTION,
                    _ => symbol_kind::CLASS,
                };
                symbol(n.display_name(), kind, &n.uri, n.range, None)
            })
            .collect(),
    )
}

/// selectors.yml entries (NAMESPACE), in definition order of names.
fn selectors(ctx: &DbtContext, m: &dyn Fn(&str) -> bool) -> Vec<SymbolInformation> {
    sorted(
        ctx.selectors
            .definitions
            .iter()
            .filter(|d| m(&d.name))
            .filter_map(|d| ctx.selector_detail_map.get(&d.name))
            .map(|s| {
                symbol(
                    s.name.clone(),
                    symbol_kind::NAMESPACE,
                    &s.uri,
                    s.range,
                    None,
                )
            })
            .collect(),
    )
}

/// Models (CLASS), at the top of their SQL file.
fn models(ctx: &DbtContext, m: &dyn Fn(&str) -> bool) -> Vec<SymbolInformation> {
    ctx.model_detail_map
        .iter()
        .filter(|(name, _)| m(name))
        .map(|(name, d)| {
            symbol(
                name.clone(),
                symbol_kind::CLASS,
                &d.uri,
                Range::default(),
                None,
            )
        })
        .collect()
}

/// Sources (INTERFACE), by their source name.
fn sources(ctx: &DbtContext, m: &dyn Fn(&str) -> bool) -> Vec<SymbolInformation> {
    ctx.source_detail_map
        .iter()
        .filter(|(name, _)| m(name))
        .map(|(name, s)| symbol(name.clone(), symbol_kind::INTERFACE, &s.uri, s.range, None))
        .collect()
}

/// Macros of every package (FUNCTION), at their whole block.
fn macros(ctx: &DbtContext, m: &dyn Fn(&str) -> bool) -> Vec<SymbolInformation> {
    ctx.macro_detail_map
        .values()
        .flatten()
        .filter(|(name, _)| m(name))
        .map(|(name, d)| symbol(name.clone(), symbol_kind::FUNCTION, &d.uri, d.range, None))
        .collect()
}

/// Exposures, metrics and semantic models (MODULE).
fn citizens(ctx: &DbtContext, m: &dyn Fn(&str) -> bool) -> Vec<SymbolInformation> {
    ctx.exposure_detail_map
        .iter()
        .chain(&ctx.metric_detail_map)
        .chain(&ctx.semantic_model_detail_map)
        .filter(|(name, _)| m(name))
        .map(|(name, c)| symbol(name.clone(), symbol_kind::MODULE, &c.uri, c.range, None))
        .collect()
}

/// One symbol per name and file: a package installed by a link, or a
/// folder of linked macros, reaches the same file under several paths.
fn dedup_same_file(out: &mut Vec<SymbolInformation>) {
    let mut seen = std::collections::HashSet::new();
    // one canonicalize per file, however many symbols it holds
    let mut paths = std::collections::HashMap::new();
    out.retain(|s| {
        let path = paths
            .entry(s.location.uri.clone())
            .or_insert_with(|| {
                cogitator_lsp::uri_to_fs_path(&s.location.uri)
                    .map(std::path::PathBuf::from)
                    .map(|p| std::fs::canonicalize(&p).unwrap_or(p))
            })
            .clone();
        seen.insert((s.name.clone(), s.kind, path, s.location.range.start.line))
    });
}
