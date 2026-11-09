//! Parity of the HIR lineage against the legacy engine over every model in
//! `testdata/jaffle_shop_duckdb` and every input of the existing lineage
//! tests. Sources compare as sets: the legacy order came from a HashMap.
//!
//! Accepted differences, the complete list:
//! 1. Legacy `__dbt_rel_N__` placeholder names (scalar Jinja, `this`, and
//!    other relations it had no name for) never appear in the new output.
//! 2. Gap closures only add: every other legacy source must still be there.
//! 3. A top-level set operation answers where legacy declined.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use cogitator_dialects::Dialect;
use cogitator_parser::{DocumentParser, JinjaSqlParser, ParsedDocument};

use super::legacy::legacy_column_lineage;
use super::{column_lineage, ModelLineage, SourceRef};

/// (dialect, input) of every test in tests/ that calls `column_lineage`,
/// then the other shapes the legacy engine answered for.
const TEST_INPUTS: &[(&str, &str)] = &[
    (
        "duckdb",
        "select a.x, b.y, a.x + b.y as z\nfrom {{ ref('a') }} as a\n\
         join {{ source('s', 't') }} b on a.id = b.id",
    ),
    ("duckdb", "select x.* from {{ ref('a') }} x"),
    (
        "duckdb",
        "select case when a > 1 then b else c end as v, cast(d as int) as e,\n\
         coalesce(f, g) as h, i between j and k as l, m like n as o,\n\
         p in (q, r) as s\nfrom {{ ref('z') }}",
    ),
    (
        "duckdb",
        "select {{ var('x') }} as v, {{ my_macro() }} from {{ ref('m') }}",
    ),
    ("duckdb", "select * from {{ ref('pkg', 'model') }}"),
    (
        "duckdb",
        "with c as (select a from {{ ref('m') }}) select a, missing from c",
    ),
    (
        "duckdb",
        "with c as (select * from {{ ref('m') }}) select id, c.name from c",
    ),
    ("duckdb", "select a from raw_schema.raw_table"),
    (
        "duckdb",
        "select t.a, o.b from {{ ref('m') }} t, raw.other o where o.b = t.a",
    ),
    (
        "duckdb",
        "select x from {{ ref('a') }} join {{ ref('b') }} on true",
    ),
    (
        "duckdb",
        "select customer_id, amount as total from {{ ref('stg_payments') }}",
    ),
    (
        "duckdb",
        "with orders as (\n    select order_id, customer_id from {{ ref('stg_orders') }}\n),\n\
         payments as (\n    select order_id, amount from {{ source('stripe', 'payment') }}\n)\n\
         select o.customer_id, p.amount\n\
         from orders o join payments p on o.order_id = p.order_id\n",
    ),
    (
        "duckdb",
        "select *, first_name || ' ' || last_name as full_name\n\
         from {{ ref('stg_customers') }}\n",
    ),
    (
        "duckdb",
        "{{ config(materialized='table') }}\n{# c #}\nselect a from {{ ref('b') }}",
    ),
    (
        "duckdb",
        "{# {{ ref('dead') }} #}{% raw %}select a{% endraw %} from {{ ref('b') }}",
    ),
    (
        "duckdb",
        "select 1 {% if flag %}, 2{% endif %} from {{ ref('a') }}",
    ),
    (
        "duckdb",
        "select customer_id, 1 as brand_new_col from {{ ref('stg_customers') }}",
    ),
    (
        "snowflake",
        "select id, amount\nfrom {{ ref('payments') }}\n\
         qualify row_number() over (partition by id order by amount desc) = 1\n",
    ),
];

fn parse(text: &str, dialect: &Dialect) -> ParsedDocument {
    JinjaSqlParser.parse(text, dialect).expect("parse")
}

/// Every `.sql` file under the fixture's `models/`, sorted.
fn fixture_models() -> Vec<PathBuf> {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/jaffle_shop_duckdb/models");
    let mut out = Vec::new();
    let mut dirs = vec![root];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(&dir).expect("read models").flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|e| e == "sql") {
                out.push(path);
            }
        }
    }
    out.sort();
    assert!(out.len() >= 5, "fixture models not found: {out:?}");
    out
}

/// The legacy report with accepted difference 1 applied.
fn legacy_columns(lineage: &ModelLineage) -> Vec<(String, BTreeSet<SourceRef>)> {
    lineage
        .columns
        .iter()
        .filter(|c| !c.name.starts_with("__dbt_rel_"))
        .map(|c| {
            let sources = c
                .sources
                .iter()
                .filter(|s| !is_placeholder(s))
                .cloned()
                .collect();
            (c.name.clone(), sources)
        })
        .collect()
}

fn is_placeholder(source: &SourceRef) -> bool {
    match source {
        SourceRef::Model { column, .. } | SourceRef::Source { column, .. } => {
            column.starts_with("__dbt_rel_")
        }
        SourceRef::Unknown { name, column } => {
            name.starts_with("__dbt_rel_") || column.starts_with("__dbt_rel_")
        }
    }
}

fn assert_parity(label: &str, text: &str, dialect: &Dialect) {
    let doc = parse(text, dialect);
    let legacy = legacy_column_lineage(&doc, dialect);
    let current = column_lineage(&doc, dialect);
    let (legacy, current) = match (legacy, current) {
        (None, None) => return,
        (Some(legacy), Some(current)) => (legacy, current),
        (legacy, current) => {
            panic!(
                "{label}: legacy answered {}, HIR answered {}\n{text}",
                legacy.is_some(),
                current.is_some()
            );
        }
    };
    let legacy = legacy_columns(&legacy);
    let names: Vec<&str> = current.columns.iter().map(|c| c.name.as_str()).collect();
    let legacy_names: Vec<&str> = legacy.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, legacy_names, "{label}: output columns differ");
    for (column, (name, expected)) in current.columns.iter().zip(&legacy) {
        let sources: BTreeSet<&SourceRef> = column.sources.iter().collect();
        let lost: Vec<_> = expected.iter().filter(|s| !sources.contains(s)).collect();
        assert!(
            lost.is_empty(),
            "{label}: column {name} lost {lost:?}, kept {sources:?}"
        );
    }
}

#[test]
fn fixture_models_match_legacy() {
    let dialect = Dialect::new("duckdb");
    for path in fixture_models() {
        let text = std::fs::read_to_string(&path).expect("read model");
        assert_parity(&path.display().to_string(), &text, &dialect);
    }
}

#[test]
fn lineage_test_inputs_match_legacy() {
    for (dialect, text) in TEST_INPUTS {
        assert_parity(text, text, &Dialect::new(*dialect));
    }
}

#[test]
fn control_blocks_still_decline() {
    let dialect = Dialect::new("duckdb");
    for text in [
        "select 1 {% if flag %}, 2{% endif %} from {{ ref('a') }}",
        // HIR can build this as its Incremental variant; lineage still declines
        "select a from {{ ref('b') }}\n{% if is_incremental() %}where a > 1{% endif %}",
    ] {
        assert!(
            column_lineage(&parse(text, &dialect), &dialect).is_none(),
            "{text}"
        );
    }
}

#[test]
fn top_level_set_operations_answer_where_legacy_declined() {
    // Accepted difference 3, pinned so it cannot widen unnoticed.
    let dialect = Dialect::new("duckdb");
    let doc = parse(
        "select a from {{ ref('x') }} union all select b from {{ ref('y') }}",
        &dialect,
    );
    assert!(legacy_column_lineage(&doc, &dialect).is_none());
    assert!(column_lineage(&doc, &dialect).is_some());
}
