//! Parser-pipeline benches (09 §5): the phase-1 minijinja+sqlparser
//! pass over a realistic model, and the jinja block scans. Run with
//! `just bench`; CI only proves they compile (`cargo bench --no-run`).

use criterion::{black_box, criterion_group, criterion_main, Criterion};

use cogitator_dialects::Dialect;
use cogitator_parser::{jinja_scan, DocumentParser, JinjaSqlParser};

const MODEL: &str = r#"
with customers as (
    select * from {{ ref('stg_customers') }}
),
orders as (
    select * from {{ ref('stg_orders') }}
),
payments as (
    select * from {{ source('stripe', 'payments') }}
),
customer_orders as (
    select
        customer_id,
        min(order_date) as first_order,
        max(order_date) as most_recent_order,
        count(order_id) as number_of_orders
    from orders
    group by customer_id
),
final as (
    select
        customers.customer_id,
        {{ full_name('first_name', 'last_name') }} as full_name,
        customer_orders.first_order,
        customer_orders.most_recent_order,
        customer_orders.number_of_orders,
        payments.amount
    from customers
    left join customer_orders using (customer_id)
    left join payments using (customer_id)
    where customers.customer_id > {{ var('min_customer_id', 0) }}
)
select * from final
"#;

fn parse_document(c: &mut Criterion) {
    let dialect = Dialect::new("duckdb");
    c.bench_function("parse_model_pipeline", |b| {
        b.iter(|| JinjaSqlParser.parse(black_box(MODEL), &dialect))
    });
}

fn scan_blocks(c: &mut Criterion) {
    let text = "{% macro m(a, b) %}{{ a }}{% endmacro %}\n\
                {% snapshot s %}select 1{% endsnapshot %}\n"
        .repeat(50);
    c.bench_function("jinja_scan_macros", |b| {
        b.iter(|| jinja_scan::extract_macros(black_box(&text)))
    });
    c.bench_function("jinja_scan_snapshots", |b| {
        b.iter(|| jinja_scan::extract_snapshots(black_box(&text)))
    });
}

criterion_group!(benches, parse_document, scan_blocks);
criterion_main!(benches);
