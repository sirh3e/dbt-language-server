//! Where an upstream column is defined, for a label part's `location`: a
//! model's column where its SQL produces it (`production_range`), a
//! source's at its YAML `name:`, else the source table's entry. Each
//! upstream file is read and parsed once per [`ProjectLocator`].

use std::collections::HashMap;
use std::path::Path;

use cogitator_lsp::{Location, Range};
use cogitator_parser::{DocumentParser, JinjaSqlParser, ParsedDocument};

use super::super::{file_uri, production_range, DbtContext};
use crate::lineage::SourceRef;

/// Resolves a reference to where it is defined.
pub trait Locate {
    fn locate(&mut self, reference: &SourceRef) -> Option<Location>;
}

/// No locations (tests, or a client that does not resolve parts).
impl Locate for () {
    fn locate(&mut self, _: &SourceRef) -> Option<Location> {
        None
    }
}

/// The project's models and sources; files come through `read`.
pub struct ProjectLocator<'a> {
    ctx: &'a DbtContext,
    read: &'a dyn Fn(&Path) -> Option<String>,
    parsed: HashMap<String, Option<ParsedDocument>>,
}

impl<'a> ProjectLocator<'a> {
    pub fn new(ctx: &'a DbtContext, read: &'a dyn Fn(&Path) -> Option<String>) -> Self {
        ProjectLocator {
            ctx,
            read,
            parsed: HashMap::new(),
        }
    }

    fn model(&mut self, name: &str, column: &str) -> Option<Location> {
        let details = self.ctx.model_detail_map.get(name)?;
        if details.uri.as_os_str().is_empty() {
            return None;
        }
        let (ctx, read) = (self.ctx, self.read);
        let parsed = self
            .parsed
            .entry(name.to_string()) // boundary: the per-request memo owns its keys
            .or_insert_with(|| {
                read(&details.uri).and_then(|text| JinjaSqlParser.parse(&text, &ctx.dialect).ok())
            });
        let range = match (parsed.as_ref(), column) {
            (_, "*") | (None, _) => Range::default(),
            (Some(parsed), column) => production_range(parsed, column).unwrap_or_default(),
        };
        Some(Location {
            uri: file_uri(&details.uri),
            range,
        })
    }

    fn source(&self, source: &str, table: &str, column: &str) -> Option<Location> {
        let table = self.ctx.source_detail_map.get(source)?.tables.get(table)?;
        if table.uri.as_os_str().is_empty() {
            return None;
        }
        let range = table
            .columns
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(column))
            .map_or(table.range, |c| Range {
                start: c.name_pos,
                end: cogitator_lsp::Position {
                    line: c.name_pos.line,
                    character: c.name_pos.character + c.name.encode_utf16().count() as u32,
                },
            });
        Some(Location {
            uri: file_uri(&table.uri),
            range,
        })
    }
}

impl Locate for ProjectLocator<'_> {
    fn locate(&mut self, reference: &SourceRef) -> Option<Location> {
        match reference {
            SourceRef::Model { name, column } => self.model(name, column),
            SourceRef::Source {
                source,
                table,
                column,
            } => self.source(source, table, column),
            SourceRef::Unknown { .. } => None,
        }
    }
}
