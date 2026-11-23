//! Column references: the transitive column walk the rename makes
//! (`rename_column::walk`), so both always agree — the model's own item,
//! its YAML entry (the declaration) and the lists that name it, and every
//! consumer at any depth that keeps the name. A file the walk cannot
//! follow (Jinja control blocks, SQL that does not parse) still shows its
//! words spelling the column, where rename would refuse.

use cogitator_lsp::Location;

use super::super::rename_column::walk::walk_column;
use super::super::{file_uri, DbtContext};
use super::files::Reader;

pub(crate) fn column_references(
    ctx: &DbtContext,
    reader: &Reader<'_>,
    model: &str,
    column: &str,
    include_declaration: bool,
) -> Vec<Location> {
    let walk = walk_column(ctx, reader, model, column);
    walk.sites
        .iter()
        .chain(&walk.unclear)
        .filter(|s| include_declaration || !s.declaration)
        .map(|s| Location {
            uri: file_uri(&s.path),
            range: s.range,
        })
        .collect()
}
