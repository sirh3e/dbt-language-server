use cogitator_dialects::Dialect;

use super::*;

fn duckdb() -> Dialect {
    Dialect::new("duckdb")
}

fn renamed(marked: &str) -> Result<Option<String>, String> {
    let at = marked.find('@').expect("a marker") as u32;
    let text = marked.replacen('@', "", 1);
    let locals = sql_locals(&text, &duckdb())?;
    let Some(idx) = locals.at(at) else {
        return Ok(None);
    };
    let local = &locals.locals[idx];
    let mut spans = local.uses.clone();
    spans.push(local.definition);
    spans.sort();
    let mut out = text.clone();
    for s in spans.iter().rev() {
        out.replace_range(s.start as usize..s.end as usize, "Z");
    }
    Ok(Some(out))
}

#[test]
fn ctes_and_aliases_rename_through_the_binder() {
    for (marked, want) in [
        // a CTE: its definition, FROM, JOIN and qualified columns
        (
            "with @a as (select 1 as x) select a.x from a join a as b on a.x = b.x",
            Some("with Z as (select 1 as x) select Z.x from Z join Z as b on Z.x = b.x"),
        ),
        // from a use
        (
            "with a as (select 1 as x), b as (select * from @a) select * from b",
            Some("with Z as (select 1 as x), b as (select * from Z) select * from b"),
        ),
        // an alias, in its query only
        (
            "select @o.id, o.amount from orders o where o.id > 0",
            Some("select Z.id, Z.amount from orders Z where Z.id > 0"),
        ),
        (
            "select x.id from orders as @x join (select o.id from items o) y on x.id = y.id",
            Some("select Z.id from orders as Z join (select o.id from items o) y on Z.id = y.id"),
        ),
        // an alias seen from a correlated subquery
        (
            "select * from t @o where exists (select 1 from u where u.k = o.k)",
            Some("select * from t Z where exists (select 1 from u where u.k = Z.k)"),
        ),
        // Jinja in the SQL does not break it
        (
            "with @src as (select * from {{ ref('stg') }}) select src.id from src",
            Some("with Z as (select * from {{ ref('stg') }}) select Z.id from Z"),
        ),
        // a for loop's body is read once
        (
            "with @p as (select 1 as a) select {% for m in ms %}p.a,{% endfor %} 1 from p",
            Some("with Z as (select 1 as a) select {% for m in ms %}Z.a,{% endfor %} 1 from Z"),
        ),
        // a column made by Jinja is not bound: `missed` refuses the rename
        (
            "with @p as (select 1 as a) select p.{{ m }} from p",
            Some("with Z as (select 1 as a) select p.{{ m }} from Z"),
        ),
        // a physical table without an alias is no local name
        ("select @orders.id from orders", None),
    ] {
        assert_eq!(renamed(marked).unwrap().as_deref(), want, "{marked}");
    }
}

#[test]
fn sql_that_does_not_parse_is_refused() {
    let text = "with a as (select 1), b as (select from where) select * from a";
    assert!(sql_locals(text, &duckdb()).is_err());
}

#[test]
fn collisions() {
    for (marked, new, collides) in [
        ("with @a as (select 1) select * from a", "b", false),
        (
            "with @a as (select 1), b as (select 2) select * from a, b",
            "b",
            true,
        ),
        (
            "with @a as (select 1) select * from a join t on true",
            "t",
            true,
        ),
        (
            "with @a as (select 1) select * from a join t as b on true",
            "b",
            true,
        ),
        ("select @o.x from t o join u p on o.x = p.x", "p", true),
        // the table behind another alias is no name in the query
        ("select @o.x from t o join u p on o.x = p.x", "u", false),
        ("select @o.x from t o join u p on o.x = p.x", "q", false),
        // another query's alias does not collide
        (
            "select @o.x from t o union all select p.x from u p",
            "p",
            false,
        ),
    ] {
        let at = marked.find('@').unwrap() as u32;
        let text = marked.replacen('@', "", 1);
        let locals = sql_locals(&text, &duckdb()).unwrap();
        let idx = locals.at(at).unwrap();
        assert_eq!(
            locals.collision(idx, new).is_some(),
            collides,
            "{marked} → {new}"
        );
    }
}

#[test]
fn what_the_binder_did_not_resolve_is_missed() {
    use cogitator_parser::{DocumentParser, JinjaSqlParser};
    let dialect = duckdb();
    for (text, at, missed_line) in [
        ("with p as (select 1 as a) select p.a from p", " p as", None),
        ("with p as (select 1 as a)\nselect p.{{ m }} from p", " p as", Some(1)),
        (
            "with p as (select 1 as a)\nselect 1 from x {% if c %}{% else %}\njoin p on true{% endif %}",
            " p as",
            Some(2),
        ),
        // a bare column of the alias's name is not an occurrence
        ("select o.id, o from t o", " o", None),
        ("select o.id, o.{{ c }} from t o", " o", Some(0)),
    ] {
        let locals = sql_locals(text, &dialect).unwrap();
        let idx = locals.at(text.rfind(at).unwrap() as u32 + 1).unwrap();
        let doc = JinjaSqlParser.parse(text, &dialect).unwrap();
        assert_eq!(missed(&doc, &locals.locals[idx], &dialect), missed_line, "{text}");
    }
}
