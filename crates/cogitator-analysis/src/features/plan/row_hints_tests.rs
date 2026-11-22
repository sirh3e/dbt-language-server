use super::super::tests::Facts;
use super::*;
use crate::artifacts::catalog::{CatalogIndex, CatalogTable};
use crate::features::plan::query_plan;

fn table(rows: u64) -> CatalogTable {
    let mut table = CatalogTable::default();
    table.stats.row_count = Some(rows);
    table
}

fn stats() -> Facts {
    let mut facts = Facts::new("duckdb");
    let mut catalog = CatalogIndex::default();
    catalog.models.insert("up".to_string(), table(1_000));
    catalog.models.insert("other".to_string(), table(50));
    facts.catalog = Some(catalog);
    facts
}

fn hints(facts: &Facts, text: &str) -> Vec<(String, Rows)> {
    query_plan(&facts.input(text, Some("m")))
        .row_hints
        .into_iter()
        .map(|h| (h.cte, h.rows))
        .collect()
}

fn one(cte: &str, rows: Rows) -> Vec<(String, Rows)> {
    vec![(cte.to_string(), rows)]
}

#[test]
fn row_counts_follow_what_the_structure_proves() {
    let up = "{{ ref('up') }}";
    let other = "{{ ref('other') }}";
    let cases: Vec<(String, Vec<(String, Rows)>)> = vec![
        (
            format!("with a as (select id from {up}) select id from a"),
            one("a", Rows::Exact(1_000)),
        ),
        (
            format!("with a as (select id from {up} where id > 1) select id from a"),
            one("a", Rows::AtMost(1_000)),
        ),
        (
            format!("with a as (select id, count(*) as n from {up} group by id) select id from a"),
            one("a", Rows::AtMost(1_000)),
        ),
        (
            format!("with a as (select count(*) as n from {up}) select n from a"),
            one("a", Rows::Exact(1)),
        ),
        (
            format!("with a as (select id from {up} limit 10) select id from a"),
            one("a", Rows::AtMost(10)),
        ),
        (
            format!("with a as (select distinct id from {up}) select id from a"),
            one("a", Rows::AtMost(1_000)),
        ),
        (
            format!(
                "with a as (select id from {up} union all select id from {other}) select id from a"
            ),
            one("a", Rows::Exact(1_050)),
        ),
        (
            format!("with a as (select id from {up} union select id from {other}) select id from a"),
            one("a", Rows::AtMost(1_050)),
        ),
        // a join's fan-out is never guessed
        (
            format!(
                "with a as (select u.id from {up} u join {other} o on u.id = o.id) select id from a"
            ),
            Vec::new(),
        ),
        // a CTE over a CTE inherits its count, weakened by its own filter
        (
            format!(
                "with a as (select id from {up}),\nb as (select id from a where id > 2)\nselect id from b"
            ),
            vec![
                ("a".to_string(), Rows::Exact(1_000)),
                ("b".to_string(), Rows::AtMost(1_000)),
            ],
        ),
        // a model without statistics gets nothing
        (
            "with a as (select id from {{ ref('unknown') }}) select id from a".to_string(),
            Vec::new(),
        ),
    ];
    let facts = stats();
    for (text, want) in cases {
        assert_eq!(hints(&facts, &text), want, "{text}");
    }
}

#[test]
fn hints_anchor_at_the_cte_name() {
    let facts = stats();
    let text = "with recent as (select id from {{ ref('up') }}) select id from recent";
    let plan = query_plan(&facts.input(text, Some("m")));
    let hint = plan.row_hints.first().expect("a hint");
    assert_eq!(
        &text[hint.span.start as usize..hint.span.end as usize],
        "recent"
    );
}

#[test]
fn no_statistics_no_hints() {
    let text = "with a as (select id from {{ ref('up') }}) select id from a";
    assert!(hints(&Facts::new("duckdb"), text).is_empty());
    // perf.catalog_stats off: the counts are not facts at all
    let facts = stats();
    let mut input = facts.input(text, Some("m"));
    input.catalog_stats = false;
    assert!(query_plan(&input).row_hints.is_empty());
}

#[test]
fn inlay_hints_sit_after_the_name_and_respect_the_range() {
    let facts = stats();
    let text = "with a as (select id from {{ ref('up') }}),\n\
                bé as (select id from a where id > 1)\n\
                select id from bé";
    let plan = query_plan(&facts.input(text, Some("m")));
    let whole = crate::features::plan::row_inlay_hints(text, &plan, Default::default());
    let shown: Vec<(u32, u32, &str)> = whole
        .iter()
        .map(|h| (h.position.line, h.position.character, h.label.as_str()))
        .collect();
    // UTF-16 columns: "bé" is two code units
    assert_eq!(shown, vec![(0, 6, "1,000 rows"), (1, 2, "≤ 1,000 rows")]);
    let second_line = cogitator_lsp::Range {
        start: cogitator_lsp::Position {
            line: 1,
            character: 0,
        },
        end: cogitator_lsp::Position {
            line: 1,
            character: 10,
        },
    };
    let ranged = crate::features::plan::row_inlay_hints(text, &plan, second_line);
    assert_eq!(ranged.len(), 1);
}
