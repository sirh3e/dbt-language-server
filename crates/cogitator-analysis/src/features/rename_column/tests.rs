//! Case tables for one document's part of a column rename: the text it
//! becomes, or why it is refused.

use std::collections::BTreeSet;

use cogitator_dialects::Dialect;
use cogitator_parser::{DocumentParser, JinjaSqlParser};

use super::sql::{analyse, Seed, SqlEdit};
use super::valid_column_name;

/// The document with its sites rewritten to `new`, and whether its
/// output still carries the column on.
fn renamed(
    sql: &str,
    upstream: Option<&[&str]>,
    column: &str,
    new: &str,
) -> Result<(String, bool), String> {
    let parsed = JinjaSqlParser.parse(sql, &Dialect::default()).unwrap();
    let models: BTreeSet<String> = upstream
        .unwrap_or(&[])
        .iter()
        .map(|s| s.to_string())
        .collect();
    let seed = match upstream {
        Some(_) => Seed::Upstream(&models),
        None => Seed::Own,
    };
    let doc = analyse(&parsed, &Dialect::default(), seed, column)?;
    let mut sites = doc.sites.clone();
    sites.sort_by_key(|s| std::cmp::Reverse(s.span.start));
    let mut text = sql.to_string();
    for site in sites {
        let (start, end) = (site.span.start as usize, site.span.end as usize);
        match site.edit {
            SqlEdit::Name => {
                let name = match site.quote {
                    Some(q) => format!("{q}{new}{q}"),
                    None => new.to_string(),
                };
                text.replace_range(start..end, &name);
            }
            SqlEdit::AliasAfter => text.insert_str(end, &format!(" as {new}")),
        }
    }
    Ok((text, doc.exposes))
}

#[test]
fn the_model_itself() {
    for (sql, expected) in [
        // an alias at the root
        ("select id as a, b from {{ ref('x') }}", "select id as z, b from {{ ref('x') }}"),
        // a bare column of an upstream relation gets an alias
        ("select a, b from {{ ref('x') }}", "select a as z, b from {{ ref('x') }}"),
        (
            "select t.a from {{ ref('x') }} t where t.a > 0",
            "select t.a as z from {{ ref('x') }} t where t.a > 0",
        ),
        // the alias in a CTE, carried up through select *
        (
            "with r as (select id as a from {{ ref('x') }}) select * from r",
            "with r as (select id as z from {{ ref('x') }}) select * from r",
        ),
        // through a bare pass-through and a join that reads it
        (
            "with r as (select id as a from {{ ref('x') }}),\nf as (select r.a from r join {{ ref('y') }} y on r.a = y.k)\nselect * from f",
            "with r as (select id as z from {{ ref('x') }}),\nf as (select r.z from r join {{ ref('y') }} y on r.z = y.k)\nselect * from f",
        ),
        // a CTE over upstream *: the item that names it gets the alias
        (
            "with s as (select * from {{ ref('x') }})\nselect s.a from s",
            "with s as (select * from {{ ref('x') }})\nselect s.a as z from s",
        ),
        // re-aliased in a later CTE: the CTE above it is left alone
        (
            "with r as (select id as a from {{ ref('x') }}), f as (select a as a2, a from r) select f.a from f",
            "with r as (select id as z from {{ ref('x') }}), f as (select z as a2, z from r) select f.z from f",
        ),
        // order by the alias
        ("select id as a from {{ ref('x') }} order by a", "select id as z from {{ ref('x') }} order by z"),
    ] {
        let got = renamed(sql, None, "a", "z").map(|(t, _)| t);
        assert_eq!(got, Ok(expected.to_string()), "{sql}");
    }
}

#[test]
fn consumers_follow_while_the_name_is_kept() {
    for (sql, expected, exposes) in [
        // bare pass-through: renamed, and the walk goes on
        ("select a, b from {{ ref('m') }}", "select z, b from {{ ref('m') }}", true),
        // qualified, in where, group by and a join
        (
            "select m.a, count(*) as n from {{ ref('m') }} m join {{ ref('o') }} o on m.a = o.k where m.a > 1 group by m.a",
            "select m.z, count(*) as n from {{ ref('m') }} m join {{ ref('o') }} o on m.z = o.k where m.z > 1 group by m.z",
            true,
        ),
        // re-aliased: only the reference changes, the walk stops
        ("select a as other from {{ ref('m') }}", "select z as other from {{ ref('m') }}", false),
        ("select sum(a) as total from {{ ref('m') }}", "select sum(z) as total from {{ ref('m') }}", false),
        // through a CTE star, then exposed by star
        (
            "with c as (select * from {{ ref('m') }}) select * from c",
            "with c as (select * from {{ ref('m') }}) select * from c",
            true,
        ),
        (
            "with c as (select * from {{ ref('m') }}) select c.a as kept from c",
            "with c as (select * from {{ ref('m') }}) select c.z as kept from c",
            false,
        ),
        // a model it does not read
        ("select a from {{ ref('other') }}", "select a from {{ ref('other') }}", false),
    ] {
        let got = renamed(sql, Some(&["m"]), "a", "z");
        assert_eq!(got, Ok((expected.to_string(), exposes)), "{sql}");
    }
}

#[test]
fn what_cannot_be_followed_is_refused() {
    for (sql, upstream, why) in [
        (
            "{% if x %}select a from {{ ref('m') }}{% endif %}",
            Some(&["m"][..]),
            "control blocks",
        ),
        (
            "select a from {{ ref('m') }} m join {{ ref('o') }} o on m.k = o.k",
            Some(&["m"][..]),
            "more than one relation",
        ),
        (
            "select * from {{ ref('m') }} join {{ ref('o') }} using (a)",
            Some(&["m"][..]),
            "using",
        ),
        (
            "select {{ f('a') }} as b from {{ ref('m') }}",
            Some(&["m"][..]),
            "Jinja",
        ),
        ("select * from {{ ref('x') }}", None, "select *"),
        (
            "select a from {{ ref('x') }} union all select a from {{ ref('y') }}",
            None,
            "set operation",
        ),
        (
            "select b as a, a from {{ ref('x') }}",
            None,
            "more than once",
        ),
        ("select b from {{ ref('x') }}", None, "no column named"),
    ] {
        let err = renamed(sql, upstream, "a", "z").unwrap_err();
        assert!(err.contains(why), "{sql}: {err}");
    }
}

#[test]
fn new_names() {
    for (name, ok) in [
        ("total", true),
        ("_x1", true),
        ("1x", false),
        ("a b", false),
        ("", false),
        ("é", false),
    ] {
        assert_eq!(valid_column_name(name), ok, "{name}");
    }
}
