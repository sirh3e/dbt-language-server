//! What navigation finds of a SQL name: the definition and every use,
//! scoped as the binder scopes them.

use cogitator_dialects::Dialect;
use cogitator_ir::Span;

use super::*;

/// `marked` with `@` at the cursor; the answer is the text with the
/// definition written `[name]` and each use `<name>`, or `None` when the
/// cursor names no local.
fn navigated(marked: &str) -> Option<String> {
    let at = marked.find('@').expect("a marker") as u32;
    let text = marked.replacen('@', "", 1);
    let locals = sql_locals(&text, &Dialect::new("duckdb")).expect("it binds");
    let (definition, uses): (Span, Vec<Span>) = match locals.at(at) {
        Some(i) => (locals.locals[i].definition, locals.locals[i].uses.clone()),
        None => {
            let c = &locals.columns[locals.column_at(at)?];
            (c.definition, c.uses.clone())
        }
    };
    let mut spans: Vec<(Span, bool)> = uses.into_iter().map(|s| (s, false)).collect();
    spans.push((definition, true));
    spans.sort();
    let mut out = text.clone();
    for (s, def) in spans.iter().rev() {
        let word = &text[s.start as usize..s.end as usize];
        let wrapped = if *def {
            format!("[{word}]")
        } else {
            format!("<{word}>")
        };
        out.replace_range(s.start as usize..s.end as usize, &wrapped);
    }
    Some(out)
}

#[test]
fn aliases_and_ctes_are_found_in_their_scope() {
    for (marked, want) in [
        // a table alias, from a qualified column
        (
            "select @o.amount from orders o where o.id > 0",
            Some("select <o>.amount from orders [o] where <o>.id > 0"),
        ),
        // `join … as x`
        (
            "select x.id from a join b as @x on true",
            Some("select <x>.id from a join b as [x] on true"),
        ),
        // a CTE used in FROM and JOIN
        (
            "with c as (select 1 as k) select c.k from @c join c as d on true",
            Some("with [c] as (select 1 as k) select <c>.k from <c> join <c> as d on true"),
        ),
        // the same alias in two scopes: the subquery's own
        (
            "select o.id from t o where exists (select 1 from u o where @o.k = 1)",
            Some("select o.id from t o where exists (select 1 from u [o] where <o>.k = 1)"),
        ),
        // … and the outer one, not shadowed by the subquery's
        (
            "select @o.id from t o where exists (select 1 from u o where o.k = 1)",
            Some("select <o>.id from t [o] where exists (select 1 from u o where o.k = 1)"),
        ),
        // a nested CTE shadows the outer one inside its query
        (
            "with c as (select 1 as k) select * from (with c as (select 2 as k) select * from @c) s",
            Some("with c as (select 1 as k) select * from (with [c] as (select 2 as k) select * from <c>) s"),
        ),
        // quoted identifiers
        (
            r#"select @"O".id from t "O""#,
            Some(r#"select <"O">.id from t ["O"]"#),
        ),
        // Jinja in between
        (
            "select @s.id from {{ ref('stg') }} s join {{ source('a', 'b') }} t on s.id = t.id",
            Some("select <s>.id from {{ ref('stg') }} [s] join {{ source('a', 'b') }} t on <s>.id = t.id"),
        ),
    ] {
        assert_eq!(navigated(marked).as_deref(), want, "{marked}");
    }
}

#[test]
fn column_aliases_are_found_where_they_are_read() {
    for (marked, want) in [
        // ORDER BY and GROUP BY read the select alias
        (
            "select a + 1 as @n from t group by n order by n",
            Some("select a + 1 as [n] from t group by <n> order by <n>"),
        ),
        // QUALIFY
        (
            "select row_number() over (order by a) as rn from t qualify @rn = 1",
            Some("select row_number() over (order by a) as [rn] from t qualify <rn> = 1"),
        ),
        // a CTE's column, read qualified and bare outside it
        (
            "with c as (select id as k from t) select c.@k, k from c",
            Some("with c as (select id as [k] from t) select c.<k>, <k> from c"),
        ),
        // a CTE's column list names its columns
        (
            "with c (k) as (select 1) select c.@k from c",
            Some("with c ([k]) as (select 1) select c.<k> from c"),
        ),
        // a derived table's alias
        (
            "select d.@x from (select 1 as x) d",
            Some("select d.<x> from (select 1 as [x]) d"),
        ),
        // a set operation names its outputs by its first SELECT
        (
            "with c as (select 1 as v union all select 2 as w) select @v from c",
            Some("with c as (select 1 as [v] union all select 2 as w) select <v> from c"),
        ),
        // the same alias in two CTEs: each its own
        (
            "with a as (select 1 as k), b as (select 2 as k) select a.k, b.@k from a, b",
            Some("with a as (select 1 as k), b as (select 2 as [k]) select a.k, b.<k> from a, b"),
        ),
        // Jinja around the column
        (
            "with c as (select {{ dbt.hash('x') }} as @h from {{ ref('m') }}) select h from c",
            Some("with c as (select {{ dbt.hash('x') }} as [h] from {{ ref('m') }}) select <h> from c"),
        ),
        // an outer alias, not the derived table's alias of the same name
        (
            "select x as k from (select 1 as k) d order by @k",
            Some("select x as [k] from (select 1 as k) d order by <k>"),
        ),
        // beside a table of unknown columns, the CTE that names it
        (
            "with c as (select 1 as k) select k from c, t order by @k",
            Some("with c as (select 1 as [k]) select <k> from c, t order by <k>"),
        ),
        // WHERE over a table of unknown columns reads the table: no guess
        ("select lower(name) as nm from t where @nm = 'x'", None),
        // a column with no alias is none of ours
        ("select @id from t", None),
    ] {
        assert_eq!(navigated(marked).as_deref(), want, "{marked}");
    }
}
