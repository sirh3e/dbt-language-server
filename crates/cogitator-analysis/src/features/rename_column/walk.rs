//! The transitive column walk both references and rename read: from one
//! model's output column through every consumer that keeps the name, at
//! any depth. A consumer model, or snapshot, whose own output carries the
//! column under the same name is renamed too and the walk goes on from
//! it; one that re-aliases it (`old as other`) only changes the reference
//! and the walk stops there. Pure over the context and a file reader.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use cogitator_lsp::Range;
use cogitator_parser::{DocumentParser, JinjaSqlParser, LineTable, ParsedDocument, TokenKind};

use super::super::references::files::{is_yaml, yaml_files, Reader};
use super::super::references::occur::{span, word_uses};
use super::super::DbtContext;
use super::semantic::{semantic_sites, SemanticSite};
use super::snapshots::{snapshot_files, Snapshot};
use super::sql::{analyse, DocWalk, Seed, SqlEdit};
use super::yaml::{entry_sites, relationship_sites, unit_test_sites};
use crate::graph::NodeKind;

/// What a site becomes when renamed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Edit {
    Name {
        quote: Option<char>,
    },
    /// ` as <new>` after the range.
    AliasAfter,
    /// A line `expr: <new>` after the end of the range's line, at this indent.
    ExprLine {
        at: cogitator_lsp::Position,
        indent: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Site {
    pub path: PathBuf,
    pub range: Range,
    pub edit: Edit,
    /// The starting model's own `columns:` entry.
    pub declaration: bool,
}

#[derive(Debug, Default)]
pub(crate) struct ColumnWalk {
    pub sites: Vec<Site>,
    /// File and reason, for every affected file the walk cannot follow.
    pub refusals: Vec<String>,
    /// Word matches of the files it could not follow, for references.
    pub unclear: Vec<Site>,
    /// The renamed models, the starting one first.
    pub models: Vec<String>,
    /// Names the column would meet: (where, lowercased name).
    pub taken: Vec<(String, String)>,
}

/// More files than this is a refusal, not a slow request.
const MAX_FILES: usize = 2000;

pub(crate) fn walk_column(
    ctx: &DbtContext,
    reader: &Reader<'_>,
    model: &str,
    column: &str,
) -> ColumnWalk {
    let mut out = ColumnWalk::default();
    let Some(details) = ctx.model_detail_map.get(model) else {
        out.refusals
            .push(format!("`{model}` is not a model of this project"));
        return out;
    };
    let own_path = details.uri.clone();
    let mut renamed: BTreeSet<String> = BTreeSet::from([model.to_string()]);
    out.models.push(model.to_string());
    match read_parsed(ctx, reader, &own_path) {
        None => out
            .refusals
            .push(format!("{}: cannot be read", own_path.display())),
        Some(parsed) => match analyse(&parsed, &ctx.dialect, Seed::Own, column) {
            Ok(doc) => out.add_doc(model, &own_path, &parsed.text, doc),
            Err(why) => {
                out.refusals.push(format!("{}: {why}", own_path.display()));
                out.unclear_words(&own_path, &parsed, column);
            }
        },
    }
    let snapshots = snapshot_files(ctx, reader);
    let mut docs: BTreeMap<PathBuf, Result<Analysed, String>> = BTreeMap::new();
    loop {
        let before = renamed.len();
        for (path, owner, kind) in consumers(ctx, &renamed, &snapshots, &own_path) {
            if docs.len() > MAX_FILES {
                out.refusals.push("the walk reaches too many files".into());
                return out;
            }
            let parsed = match kind {
                Consumer::Snapshot(text) => JinjaSqlParser.parse(&text, &ctx.dialect).ok(),
                Consumer::Sql => read_parsed(ctx, reader, &path),
            };
            let Some(parsed) = parsed else { continue };
            let result = analyse(&parsed, &ctx.dialect, Seed::Upstream(&renamed), column);
            let exposes = matches!(&result, Ok(d) if d.exposes);
            if exposes {
                if let Some(owner) = &owner {
                    if renamed.insert(owner.clone()) {
                        out.models.push(owner.clone());
                    }
                }
            }
            let entry = match result {
                Ok(doc) => Ok((parsed.text.to_string(), owner, doc)),
                Err(why) if declines_matter(&parsed, column) => {
                    out.unclear_words(&path, &parsed, column);
                    Err(why)
                }
                Err(_) => continue,
            };
            docs.insert(path, entry);
        }
        if renamed.len() == before {
            break;
        }
    }
    for (path, entry) in docs {
        match entry {
            Ok((text, owner, doc)) => {
                let label = owner.unwrap_or_else(|| path.display().to_string());
                out.add_doc(&label, &path, &text, doc);
            }
            Err(why) => out.refusals.push(format!("{}: {why}", path.display())),
        }
    }
    out.yaml(ctx, reader, &renamed, column);
    out.unclear
        .sort_by(|a, b| (&a.path, a.range.start.line).cmp(&(&b.path, b.range.start.line)));
    out
}

/// A consumer's text, the model it defines, and its sites.
type Analysed = (String, Option<String>, DocWalk);

enum Consumer {
    Sql,
    /// A snapshot's text with its `{% snapshot %}` tags blanked.
    Snapshot(String),
}

/// Every SQL file that refs a renamed model: (path, the model it defines
/// when its output can carry the column on, kind).
fn consumers(
    ctx: &DbtContext,
    renamed: &BTreeSet<String>,
    snapshots: &[Snapshot],
    own: &PathBuf,
) -> Vec<(PathBuf, Option<String>, Consumer)> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for model in renamed {
        for node in ctx.graph.downstream_of(model) {
            let owner = match node.kind {
                NodeKind::Model => Some(node.name.clone()),
                NodeKind::Test | NodeKind::Analysis => None,
                _ => continue,
            };
            if node.uri.as_os_str().is_empty() || &node.uri == own || is_yaml(&node.uri) {
                continue;
            }
            if seen.insert(node.uri.clone()) {
                out.push((node.uri.clone(), owner, Consumer::Sql));
            }
        }
    }
    for s in snapshots {
        if s.refs.iter().any(|r| renamed.contains(r)) && seen.insert(s.path.clone()) {
            out.push((
                s.path.clone(),
                Some(s.name.clone()),
                Consumer::Snapshot(s.masked.clone()),
            ));
        }
    }
    out
}

pub(super) fn read_parsed(
    ctx: &DbtContext,
    reader: &Reader<'_>,
    path: &std::path::Path,
) -> Option<ParsedDocument> {
    let text = reader.read(path)?;
    JinjaSqlParser.parse(&text, &ctx.dialect).ok()
}

/// A document the HIR cannot read only matters when it could read the
/// column: it spells the name, or a `*` could pass it on.
fn declines_matter(parsed: &ParsedDocument, column: &str) -> bool {
    !word_uses(parsed, column).is_empty()
        || parsed
            .tokens
            .iter()
            .any(|t| t.kind == TokenKind::Symbol('*'))
}

impl ColumnWalk {
    fn add_doc(&mut self, label: &str, path: &Path, text: &str, doc: DocWalk) {
        let table = LineTable::new(text);
        for site in doc.sites {
            let (line, start) = table.line_col(site.span.start);
            let (_, end) = table.line_col(site.span.end);
            self.sites.push(Site {
                path: path.to_path_buf(),
                range: span(line, start, end.saturating_sub(start)),
                edit: match site.edit {
                    SqlEdit::Name => Edit::Name { quote: site.quote },
                    SqlEdit::AliasAfter => Edit::AliasAfter,
                },
                declaration: false,
            });
        }
        if doc.exposes || !doc.taken.is_empty() {
            for name in doc.taken {
                self.taken.push((label.to_string(), name));
            }
        }
    }

    fn unclear_words(&mut self, path: &Path, parsed: &ParsedDocument, column: &str) {
        for range in word_uses(parsed, column) {
            self.unclear.push(Site {
                path: path.to_path_buf(),
                range,
                edit: Edit::Name { quote: None },
                declaration: false,
            });
        }
    }

    fn yaml(
        &mut self,
        ctx: &DbtContext,
        reader: &Reader<'_>,
        renamed: &BTreeSet<String>,
        column: &str,
    ) {
        let start = self.models.first().cloned();
        for model in self.models.clone() {
            let model = &model;
            let Some(details) = ctx.model_detail_map.get(model) else {
                continue;
            };
            for c in &details.columns {
                self.taken
                    .push((model.clone(), c.name.to_ascii_lowercase()));
            }
            let Some(schema) = details.schema_uri.clone() else {
                continue;
            };
            let Some(text) = reader.read(&schema) else {
                continue;
            };
            let (name, lists) = entry_sites(ctx, &text, model, column);
            let first = start.as_deref() == Some(model.as_str());
            if let Some(range) = name {
                self.push_name(&schema, range, first);
            }
            for range in lists {
                self.push_name(&schema, range, false);
            }
        }
        let is_renamed = |m: &str| renamed.contains(m);
        for path in yaml_files(ctx) {
            let Some(text) = reader.read(&path) else {
                continue;
            };
            for model in renamed {
                for range in relationship_sites(&text, model, column) {
                    self.push_name(&path, range, false);
                }
            }
            match unit_test_sites(&text, column, &is_renamed, &path) {
                Ok(ranges) => ranges
                    .into_iter()
                    .for_each(|r| self.push_name(&path, r, false)),
                Err(why) => self.refusals.push(why),
            }
            for site in semantic_sites(&text, &ctx.dialect, &is_renamed, column) {
                match site {
                    SemanticSite::Word(range) => self.push_name(&path, range, false),
                    SemanticSite::AddExpr { name, at, indent } => self.sites.push(Site {
                        path: path.to_path_buf(),
                        range: name,
                        edit: Edit::ExprLine { at, indent },
                        declaration: false,
                    }),
                }
            }
        }
        self.sites.sort_by(|a, b| {
            (&a.path, a.range.start.line, a.range.start.character).cmp(&(
                &b.path,
                b.range.start.line,
                b.range.start.character,
            ))
        });
        self.sites
            .dedup_by(|a, b| a.path == b.path && a.range == b.range);
    }

    fn push_name(&mut self, path: &Path, range: Range, declaration: bool) {
        self.sites.push(Site {
            path: path.to_path_buf(),
            range,
            edit: Edit::Name { quote: None },
            declaration,
        });
    }
}
