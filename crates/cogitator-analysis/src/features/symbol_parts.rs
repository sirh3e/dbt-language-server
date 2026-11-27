//! The parts of nodes workspace/symbol lists beside the definitions:
//! source tables, documented columns and tests, each where its YAML
//! names it, with what it belongs to as its container.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use cogitator_lsp::{symbol_kind, Position, Range, SymbolInformation};
use cogitator_schemas::ColumnEntry;
use saphyr::{LoadableYamlNode, MarkedYaml};

use super::tests_nav::{collect_model_tests, collect_unit_tests, model_entry, ModelTest};
use super::workspace_symbols::symbol;
use super::DbtContext;

/// Every source's tables as `source.table` (INTERFACE), contained by the
/// source.
pub(super) fn source_table_symbols(
    ctx: &DbtContext,
    m: &dyn Fn(&str) -> bool,
) -> Vec<SymbolInformation> {
    ctx.source_detail_map
        .iter()
        .flat_map(|(source, s)| {
            s.tables.iter().filter_map(move |(table, t)| {
                let name = format!("{source}.{table}");
                m(&name).then(|| {
                    symbol(
                        name,
                        symbol_kind::INTERFACE,
                        &t.uri,
                        t.range,
                        Some(source.clone()),
                    )
                })
            })
        })
        .collect()
}

/// Documented columns (FIELD): `model.column` in the model's properties
/// YAML, `source.table.column` in the sources YAML.
pub(super) fn column_symbols(ctx: &DbtContext, m: &dyn Fn(&str) -> bool) -> Vec<SymbolInformation> {
    let models = ctx.model_detail_map.iter().filter_map(|(model, d)| {
        let path = d.schema_uri.as_ref()?;
        Some((model.clone(), path.as_path(), d.columns.as_slice()))
    });
    let tables = ctx.source_detail_map.iter().flat_map(|(source, s)| {
        s.tables.iter().map(move |(table, t)| {
            (
                format!("{source}.{table}"),
                t.uri.as_path(),
                t.columns.as_slice(),
            )
        })
    });
    models
        .chain(tables)
        .flat_map(|(owner, path, columns)| columns_of(owner, path, columns, m))
        .collect()
}

fn columns_of(
    owner: String,
    path: &Path,
    columns: &[ColumnEntry],
    m: &dyn Fn(&str) -> bool,
) -> Vec<SymbolInformation> {
    columns
        .iter()
        .filter_map(|c| {
            let name = format!("{owner}.{}", c.name);
            m(&name).then(|| {
                symbol(
                    name,
                    symbol_kind::FIELD,
                    path,
                    name_range(c),
                    Some(owner.clone()),
                )
            })
        })
        .collect()
}

/// The column's `name:` value, as wide as the name.
fn name_range(c: &ColumnEntry) -> Range {
    let width = c.name.encode_utf16().count() as u32;
    Range {
        start: c.name_pos,
        end: Position {
            line: c.name_pos.line,
            character: c.name_pos.character + width,
        },
    }
}

/// Every model's data tests (`not_null on orders.order_id`) and unit
/// tests (by their name), METHOD, contained by the model. Each
/// properties file is read and parsed once.
pub(super) fn test_symbols(
    ctx: &DbtContext,
    m: &dyn Fn(&str) -> bool,
    read: &dyn Fn(&Path) -> Option<String>,
) -> Vec<SymbolInformation> {
    let mut by_file: BTreeMap<&PathBuf, Vec<&str>> = BTreeMap::new();
    for (model, d) in &ctx.model_detail_map {
        if let Some(path) = &d.schema_uri {
            by_file.entry(path).or_default().push(model);
        }
    }
    let mut out = Vec::new();
    for (path, models) in by_file {
        let Some(text) = read(path) else { continue };
        let Ok(docs) = MarkedYaml::load_from_str(&text) else {
            continue;
        };
        let Some(root) = docs.first() else { continue };
        let uri = super::file_uri(path);
        for model in models {
            let mut tests = model_entry(root, model)
                .map(|e| collect_model_tests(e, &uri))
                .unwrap_or_default();
            tests.extend(collect_unit_tests(root, model, &uri));
            out.extend(tests.into_iter().filter_map(|t| {
                let name = test_name(model, &t);
                m(&name).then(|| SymbolInformation {
                    name,
                    kind: symbol_kind::METHOD,
                    location: cogitator_lsp::Location {
                        uri: t.uri,
                        range: t.range,
                    },
                    container_name: Some(model.to_string()),
                })
            }));
        }
    }
    out
}

/// `not_null on orders.order_id`, `unique on orders (v2)`; a unit test
/// by its own name.
pub(super) fn test_name(model: &str, t: &ModelTest) -> String {
    if t.unit {
        return t.name.clone();
    }
    let on = match &t.column {
        Some(column) => format!("{model}.{column}"),
        None => model.to_string(),
    };
    match t.version {
        Some(v) => format!("{} on {on} (v{v})", t.name),
        None => format!("{} on {on}", t.name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test(name: &str, column: Option<&str>, version: Option<i64>, unit: bool) -> ModelTest {
        ModelTest {
            name: name.into(),
            column: column.map(Into::into),
            version,
            unit,
            uri: String::new(),
            range: Range::default(),
        }
    }

    #[test]
    fn test_names() {
        let cases = [
            (
                test("not_null", Some("id"), None, false),
                "not_null on orders.id",
            ),
            (test("unique", None, None, false), "unique on orders"),
            (
                test("unique", Some("id"), Some(2), false),
                "unique on orders.id (v2)",
            ),
            (test("orders_math", None, None, true), "orders_math"),
        ];
        for (t, want) in cases {
            assert_eq!(test_name("orders", &t), want);
        }
    }
}
