//! "Scaffold staging model stg_<source>__<table>" (`refactor`): on a
//! `source('a', 'b')` or `ref('x')` in SQL, or on a source table's entry
//! in a properties file, a new staging model over that relation in the
//! shape of dbt's style guide ([`super::staging_render`]) and its
//! properties entry, descriptions inherited from the relation's columns.
//!
//! Names: `stg_<source>__<table>` for a source, `stg_<model>` for a ref.
//! Where: beside the source's properties file (where a source is
//! declared is where its staging lives), or beside the file the ref is
//! in; the entry is a new `<name>.yml` beside the SQL, as
//! `dbt.extractModelYaml` writes one. Refused when the model or either
//! file exists already.
//!
//! Columns, first that knows any: for a source, `catalog.json`'s columns
//! (the warehouse's) else its YAML columns, a YAML `data_type:` winning
//! over the catalog's; for a model, the inferred output types
//! (`column_types`), else the catalog, else its YAML columns. Without
//! any the body is `select *` and the title says so.

use std::path::{Path, PathBuf};

use cogitator_lsp::{
    CodeAction, CreateFile, DocumentChange, Position, Range, TextDocumentEdit, TextEdit,
    WorkspaceEdit,
};
use cogitator_parser::TokenKind;

use super::super::column_types::ModelColumnTypes;
use super::super::{file_uri, DbtContext, Document};
use super::raw_description::raw_description;
use super::staging_render::{self, InputColumn};
use super::yaml_text::{block_end, indent, plain_name};
use crate::artifacts::catalog::{CatalogIndex, CatalogTable};
use cogitator_schemas::ColumnEntry;

/// What the scaffold reads besides the context, through the shell.
pub struct ScaffoldInputs<'a> {
    pub catalog: Option<&'a CatalogIndex>,
    pub types: &'a dyn Fn(&str) -> Option<ModelColumnTypes>,
    pub read: &'a dyn Fn(&Path) -> Option<String>,
}

/// The relation a staging model is scaffolded over.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Relation {
    Source { source: String, table: String },
    Model(String),
}

pub fn staging_scaffold_action(
    ctx: &DbtContext,
    doc: &Document,
    uri: &str,
    range: Range,
    inputs: &ScaffoldInputs<'_>,
) -> Option<CodeAction> {
    let here = PathBuf::from(cogitator_lsp::uri_to_fs_path(uri)?);
    let (relation, dir) = if uri.ends_with(".sql") {
        let relation = relation_in_sql(ctx, doc, range)?;
        let dir = match &relation {
            Relation::Source { source, table } => {
                source_dir(ctx, source, table).or_else(|| here.parent().map(Path::to_path_buf))?
            }
            Relation::Model(_) => here.parent()?.to_path_buf(),
        };
        (relation, dir)
    } else {
        let relation = source_entry_at(ctx, &doc.text, &here, range)?;
        (relation, here.parent()?.to_path_buf())
    };
    let name = match &relation {
        Relation::Source { source, table } => format!("stg_{source}__{table}"),
        Relation::Model(model) => format!("stg_{model}"),
    };
    let name = plain_name(&name)?.to_string();
    if ctx.model_detail_map.contains_key(&name) {
        return None;
    }
    let sql_path = dir.join(format!("{name}.sql"));
    let yml_path = dir.join(format!("{name}.yml"));
    if (inputs.read)(&sql_path).is_some() || (inputs.read)(&yml_path).is_some() {
        return None;
    }
    let (call, object, description, columns) = match &relation {
        Relation::Source { source, table } => {
            let details = ctx.source_detail_map.get(source)?.tables.get(table)?;
            (
                format!("{{{{ source('{source}', '{table}') }}}}"),
                table.clone(),
                details.description.clone(),
                source_columns(ctx, source, table, inputs.catalog, inputs.read),
            )
        }
        Relation::Model(model) => (
            format!("{{{{ ref('{model}') }}}}"),
            model.clone(),
            ctx.model_detail_map.get(model)?.description.clone(),
            model_columns(ctx, model, inputs),
        ),
    };
    let dialect = ctx.dialect.as_str();
    let staged = staging_render::staged(&columns, &object, dialect);
    let sql = staging_render::sql(&call, &staged, dialect);
    let yaml = staging_render::yaml(&name, description.trim(), &staged);
    let title = if staged.is_empty() {
        format!("Scaffold staging model {name} (columns unknown: select *)")
    } else {
        format!("Scaffold staging model {name}")
    };
    Some(CodeAction {
        title,
        kind: Some("refactor"),
        diagnostics: None,
        edit: Some(WorkspaceEdit {
            document_changes: [(sql_path, sql), (yml_path, yaml)]
                .into_iter()
                .flat_map(|(path, text)| new_file(&path, text))
                .collect(),
        }),
        command: None,
        is_preferred: None,
        data: None,
    })
}

fn new_file(path: &Path, text: String) -> [DocumentChange; 2] {
    let uri = file_uri(path);
    [
        DocumentChange::Create(CreateFile::new(uri.clone())),
        DocumentChange::Edit(TextDocumentEdit::new(
            uri,
            vec![TextEdit {
                range: Range::zero_width(Position::default()),
                new_text: text,
            }],
        )),
    ]
}

/// The source or model the call under the cursor names, when the
/// project knows it.
fn relation_in_sql(ctx: &DbtContext, doc: &Document, range: Range) -> Option<Relation> {
    let parsed = &doc.parsed;
    let idx = parsed.find_token_at_cursor(range.start.line, range.start.character)?;
    let token = &parsed.tokens[idx];
    let relation = match token.kind {
        TokenKind::Ref => Relation::Model(parsed.literal(token).to_string()),
        TokenKind::SourceName => {
            let table = parsed.tokens[idx + 1..]
                .iter()
                .take(6)
                .find(|t| t.kind == TokenKind::SourceTable)?;
            Relation::Source {
                source: parsed.literal(token).to_string(),
                table: parsed.literal(table).to_string(),
            }
        }
        TokenKind::SourceTable => {
            let source = parsed.lookback_find(idx, TokenKind::SourceName, 6)?;
            Relation::Source {
                source: parsed.literal(source).to_string(),
                table: parsed.literal(token).to_string(),
            }
        }
        _ => return None,
    };
    let known = match &relation {
        Relation::Source { source, table } => ctx
            .source_detail_map
            .get(source)
            .is_some_and(|s| s.tables.contains_key(table)),
        Relation::Model(model) => ctx.model_detail_map.contains_key(model),
    };
    known.then_some(relation)
}

/// The source table whose entry (its `- name:` item, columns and all)
/// holds the cursor in the properties file `text` at `path`.
fn source_entry_at(ctx: &DbtContext, text: &str, path: &Path, range: Range) -> Option<Relation> {
    let lines: Vec<&str> = text.lines().collect();
    let lines = lines.as_slice();
    let cursor = range.start.line as usize;
    ctx.source_detail_map
        .iter()
        .flat_map(|(source, details)| {
            details.tables.iter().filter_map(move |(table, t)| {
                let line = t.range.start.line as usize;
                let dash = indent(lines.get(line)?);
                let holds =
                    t.uri == path && line <= cursor && cursor <= block_end(lines, line, dash);
                holds.then_some((line, source, table))
            })
        })
        .max()
        .map(|(_, source, table)| Relation::Source {
            source: source.clone(),
            table: table.clone(),
        })
}

fn source_dir(ctx: &DbtContext, source: &str, table: &str) -> Option<PathBuf> {
    let details = ctx.source_detail_map.get(source)?;
    let t = details.tables.get(table)?;
    let file = if t.uri.as_os_str().is_empty() {
        &details.uri
    } else {
        &t.uri
    };
    file.parent().map(Path::to_path_buf)
}

fn catalog_columns(table: &CatalogTable) -> Vec<InputColumn> {
    table
        .columns
        .iter()
        .map(|c| InputColumn {
            name: c.name.clone(),
            data_type: Some(c.data_type.to_ascii_lowercase()).filter(|t| !t.is_empty()),
            description: String::new(),
        })
        .collect()
}

/// A YAML column's description as written in `file` (its properties
/// file's text), so a doc() call is carried as one.
fn written(entry: &ColumnEntry, file: Option<&str>) -> String {
    file.and_then(|text| raw_description(text, entry.name_pos))
        .unwrap_or_else(|| entry.description.clone())
        .trim()
        .to_string()
}

fn yaml_columns(columns: &[ColumnEntry], file: Option<&str>) -> Vec<InputColumn> {
    columns
        .iter()
        .map(|c| InputColumn {
            name: c.name.clone(),
            data_type: c.data_type.clone(),
            description: written(c, file),
        })
        .collect()
}

/// `base`'s columns with the YAML's descriptions and data types.
fn with_yaml(
    mut base: Vec<InputColumn>,
    yaml: &[ColumnEntry],
    file: Option<&str>,
) -> Vec<InputColumn> {
    for column in &mut base {
        if let Some(entry) = yaml
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(&column.name))
        {
            column.description = written(entry, file);
            if entry.data_type.is_some() {
                column.data_type = entry.data_type.clone();
            }
        }
    }
    base
}

fn source_columns(
    ctx: &DbtContext,
    source: &str,
    table: &str,
    catalog: Option<&CatalogIndex>,
    read: &dyn Fn(&Path) -> Option<String>,
) -> Vec<InputColumn> {
    let details = ctx
        .source_detail_map
        .get(source)
        .and_then(|s| s.tables.get(table));
    let yaml = details.map(|t| t.columns.as_slice()).unwrap_or_default();
    let file = details.and_then(|t| read(&t.uri));
    let file = file.as_deref();
    let warehouse = catalog
        .and_then(|c| c.sources.get(&(source.to_string(), table.to_string())))
        .map(catalog_columns)
        .filter(|c| !c.is_empty());
    match warehouse {
        Some(columns) => with_yaml(columns, yaml, file),
        None => yaml_columns(yaml, file),
    }
}

fn model_columns(ctx: &DbtContext, model: &str, inputs: &ScaffoldInputs<'_>) -> Vec<InputColumn> {
    let details = ctx.model_detail_map.get(model);
    let yaml = details.map(|d| d.columns.as_slice()).unwrap_or_default();
    let file = details
        .and_then(|d| d.schema_uri.as_deref())
        .and_then(|p| (inputs.read)(p));
    let file = file.as_deref();
    let inferred = (inputs.types)(model)
        .map(|types| {
            types
                .columns
                .into_iter()
                .filter(|c| !c.name.contains('*'))
                .map(|c| InputColumn {
                    name: c.name,
                    data_type: c.data_type,
                    description: String::new(),
                })
                .collect::<Vec<_>>()
        })
        .filter(|c| !c.is_empty());
    let warehouse = || {
        inputs
            .catalog
            .and_then(|c| c.models.get(model))
            .map(catalog_columns)
            .filter(|c| !c.is_empty())
    };
    match inferred.or_else(warehouse) {
        Some(columns) => with_yaml(columns, yaml, file),
        None => yaml_columns(yaml, file),
    }
}
