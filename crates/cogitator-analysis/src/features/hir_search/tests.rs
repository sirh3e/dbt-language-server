//! Case tables for the clauses over small made-up models.

use cogitator_dialects::Dialect;
use cogitator_parser::{DocumentParser, JinjaSqlParser};
use cogitator_sql_front::{build_hir, SchemaEnv};

use super::eval::sites;
use super::query::parse;
use crate::lineage::template_input;

/// The texts the query's clauses find in `sql`, per clause.
fn found(sql: &str, query: &str) -> Vec<Vec<String>> {
    let doc = JinjaSqlParser.parse(sql, &Dialect::default()).unwrap();
    let dialect = Dialect::default();
    let lowered = build_hir(&template_input(&doc, &dialect), &SchemaEnv::default());
    let hir = lowered.value.unwrap();
    parse(query)
        .unwrap()
        .clauses
        .iter()
        .map(|c| {
            sites(&hir, c)
                .into_iter()
                .map(|s| sql[s.start as usize..s.end as usize].to_string())
                .collect()
        })
        .collect()
}

const JOIN: &str = "select o.id, c.name from {{ ref('stg_orders') }} o \
     join {{ ref('stg_customers') }} c on o.customer_id = c.id";

#[test]
fn joins() {
    assert_eq!(
        found(JOIN, "joins stg_customers"),
        [vec!["{{ ref('stg_customers') }}"]]
    );
    assert_eq!(found(JOIN, "joins stg_orders on customer_id")[0].len(), 1);
    assert!(found(JOIN, "joins stg_orders on status")[0].is_empty());
    assert!(found(JOIN, "joins elsewhere")[0].is_empty());
    let using = "select * from {{ ref('a') }} join {{ ref('b') }} using (k)";
    assert_eq!(found(using, "joins b on k")[0].len(), 1);
}

#[test]
fn stars_reads_and_columns() {
    let sql = "with s as (select * from {{ source('shop', 'orders') }}),\n\
               m as (select * from {{ ref('customers') }})\n\
               select s.amount, m.name from s join m on s.id = m.id";
    assert_eq!(found(sql, "select-star")[0].len(), 2);
    assert_eq!(found(sql, "select-star from source")[0], ["*"]);
    assert_eq!(found(sql, "select-star from customers")[0], ["*"]);
    assert_eq!(
        found(sql, "reads source:shop.orders")[0],
        ["{{ source('shop', 'orders') }}"]
    );
    assert!(found(sql, "reads source:shop.payments")[0].is_empty());
    assert_eq!(found(sql, "reads model")[0], ["{{ ref('customers') }}"]);
    let direct = "select o.amount, o.id from {{ ref('orders') }} o where o.amount > 0";
    assert_eq!(
        found(direct, "uses column orders.amount")[0],
        ["o.amount", "o.amount"]
    );
    assert!(found(direct, "uses column orders.status")[0].is_empty());
}

#[test]
fn every_clause_reports_its_sites() {
    let got = found(JOIN, "reads stg_orders and joins stg_customers on id");
    assert_eq!(got.len(), 2);
    assert!(got.iter().all(|sites| !sites.is_empty()), "{got:?}");
}
