//! The parsed scope per cursor position: `|` marks the cursor.

use cogitator_dialects::Dialect;
use pretty_assertions::assert_eq;
use rstest::rstest;

use super::{analyze, Clause, Relation};
use crate::features::completion::scope::RelationTarget;

const URI: &str = "file:///tmp/scope.sql";

/// The clause and the `qualifier → relation` pairs at the cursor.
fn scope_at(marked: &str) -> Option<(Clause, Vec<String>)> {
    let offset = marked.find('|').expect("cursor mark");
    let text = marked.replacen('|', "", 1);
    let mut docs = crate::Documents::default();
    docs.open(URI.into(), &text, &Dialect::new("duckdb"));
    let doc = docs.get(URI).unwrap();
    let scope = analyze(doc, &Dialect::new("duckdb"), offset)?;
    let relations = scope
        .relations
        .iter()
        .map(|s| {
            let what = match &s.relation {
                Relation::Target(RelationTarget::Model(m)) => format!("ref {m}"),
                Relation::Target(RelationTarget::SourceTable { source, table }) => {
                    format!("source {source}.{table}")
                }
                Relation::Target(RelationTarget::Cte(c)) => format!("cte {c}"),
                Relation::Target(RelationTarget::Unknown(u)) => format!("table {u}"),
                Relation::Derived(_) => "derived".to_string(),
            };
            format!("{} → {what}", s.qualifier)
        })
        .collect();
    Some((scope.clause, relations))
}

const JOINED: &str = "from {{ ref('orders') }} o join {{ source('shop', 'customers') }} as c on o.customer_id = c.id";

#[rstest]
#[case::select_list("select |, 1 from {{ ref('orders') }} o", Clause::Projection)]
#[case::select_list_typing("select o.cu| from {{ ref('orders') }} o", Clause::Projection)]
#[case::select_list_alias("select a as | from x", Clause::Other)]
#[case::where_("select 1 from x where |", Clause::Where)]
#[case::where_and("select 1 from x where a = 1 and | order by 1", Clause::Where)]
#[case::group_by("select a from x group by |", Clause::GroupBy)]
#[case::order_by("select a from x order by |", Clause::OrderBy)]
#[case::having("select a from x group by a having |", Clause::Having)]
#[case::qualify("select a from x qualify |", Clause::Qualify)]
#[case::join_on("select 1 from x join y on |", Clause::JoinOn)]
#[case::from("select 1 from |", Clause::Relation)]
#[case::from_typing("select 1 from ord|", Clause::Relation)]
#[case::join("select 1 from x join |", Clause::Relation)]
#[case::table_alias("select 1 from x |", Clause::Other)]
fn the_clause_follows_the_cursor(#[case] marked: &str, #[case] clause: Clause) {
    let (got, _) = scope_at(marked).unwrap_or_else(|| panic!("no scope: {marked}"));
    assert_eq!(got, clause, "{marked}");
}

#[rstest]
#[case::aliased_jinja(
    &format!("select | {JOINED}"),
    &["o → ref orders", "c → source shop.customers"],
)]
#[case::unaliased_ref("select | from {{ ref('orders') }}", &["orders → ref orders"])]
#[case::cte(
    "with recent as (select 1 as n) select | from recent r join other",
    &["r → cte recent", "other → table other"],
)]
#[case::inside_a_cte(
    "with recent as (select | from {{ ref('orders') }} o) select * from recent",
    &["o → ref orders"],
)]
#[case::subquery(
    "select * from (select | from y) d join z",
    &["y → table y"],
)]
#[case::derived("select | from (select 1 as n) d", &["d → derived"])]
#[case::control_flow(
    "select | from {% if x %}{{ ref('a') }}{% else %}{{ ref('b') }}{% endif %} t",
    &["t → ref a"],
)]
// incomplete SQL: the clause after the cursor does not parse
#[case::broken_after(
    "select o.| from {{ ref('orders') }} o where",
    &["o → ref orders"],
)]
#[case::broken_join(
    "select | from {{ ref('orders') }} o left join",
    &["o → ref orders"],
)]
#[case::second_statement("select 1 from a; select | from b", &["b → table b"])]
fn the_relations_are_the_cursors_selects(#[case] marked: &str, #[case] relations: &[&str]) {
    let (_, got) = scope_at(marked).unwrap_or_else(|| panic!("no scope: {marked}"));
    assert_eq!(got, relations, "{marked}");
}

#[test]
fn what_does_not_parse_at_all_has_no_scope() {
    assert!(scope_at("select case when | from").is_none());
}
