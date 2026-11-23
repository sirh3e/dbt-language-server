use super::*;

/// The text with every occurrence of the local at the `@` marker
/// replaced by `Z` (the marker itself is removed first).
fn renamed(marked: &str) -> Option<String> {
    let at = marked.find('@').expect("a marker") as u32;
    let text = marked.replacen('@', "", 1);
    let analysis = analyze(&text)?;
    let local = &analysis.locals[analysis.at(at)?];
    let mut spans: Vec<Span> = local.bindings.iter().chain(&local.uses).copied().collect();
    spans.sort();
    let mut out = text.clone();
    for s in spans.iter().rev() {
        out.replace_range(s.start as usize..s.end as usize, "Z");
    }
    Some(out)
}

#[test]
fn locals_rename_as_jinja_scopes_them() {
    for (marked, want) in [
        // set and its uses, a later set of the same name is the same var
        (
            "{% set @x = 1 %}{{ x }}{% set x = x + 1 %}{{ x }}",
            Some("{% set Z = 1 %}{{ Z }}{% set Z = Z + 1 %}{{ Z }}"),
        ),
        // a use before the set is someone else's (a context name)
        (
            "{{ x }}{% set @x = 1 %}{{ x }}",
            Some("{{ x }}{% set Z = 1 %}{{ Z }}"),
        ),
        // block set
        (
            "{% set @body %}a{% endset %}{{ body }}",
            Some("{% set Z %}a{% endset %}{{ Z }}"),
        ),
        // loop variable: scoped to the body
        (
            "{% for @c in cols %}{{ c }}{% endfor %}{{ c }}",
            Some("{% for Z in cols %}{{ Z }}{% endfor %}{{ c }}"),
        ),
        // two loop targets
        (
            "{% for k, @v in d.items() %}{{ k }}={{ v }}{% endfor %}",
            Some("{% for k, Z in d.items() %}{{ k }}={{ Z }}{% endfor %}"),
        ),
        // a set inside a for does not leak out
        (
            "{% set @n = 0 %}{% for i in r %}{% set n = i %}{{ n }}{% endfor %}{{ n }}",
            Some("{% set Z = 0 %}{% for i in r %}{% set n = i %}{{ n }}{% endfor %}{{ Z }}"),
        ),
        // shadowing: the inner loop's c is another variable
        (
            "{% for @c in a %}{{ c }}{% for c in b %}{{ c }}{% endfor %}{{ c }}{% endfor %}",
            Some("{% for Z in a %}{{ Z }}{% for c in b %}{{ c }}{% endfor %}{{ Z }}{% endfor %}"),
        ),
        // macro parameters, from the header and from a use
        (
            "{% macro m(@a, b=1) %}{{ a }}{{ b }}{% endmacro %}{{ a }}",
            Some("{% macro m(Z, b=1) %}{{ Z }}{{ b }}{% endmacro %}{{ a }}"),
        ),
        (
            "{% macro m(a, b=1) %}{{ a ~ @b }}{% endmacro %}",
            Some("{% macro m(a, Z=1) %}{{ a ~ Z }}{% endmacro %}"),
        ),
        // a default is evaluated outside the macro
        (
            "{% set @d = 1 %}{% macro m(a=d) %}{{ a }}{% endmacro %}",
            Some("{% set Z = 1 %}{% macro m(a=Z) %}{{ a }}{% endmacro %}"),
        ),
        // caller arguments of a call block
        (
            "{% call(@row) grid(x) %}{{ row.id }}{% endcall %}{{ row }}",
            Some("{% call(Z) grid(x) %}{{ Z.id }}{% endcall %}{{ row }}"),
        ),
        // inside if, filters, calls and attribute bases
        (
            "{% set @t = 'a' %}{% if t %}{{ t | upper }}{{ f(t, k=t) }}{{ t.x }}{% endif %}",
            Some("{% set Z = 'a' %}{% if Z %}{{ Z | upper }}{{ f(Z, k=Z) }}{{ Z.x }}{% endif %}"),
        ),
        // a generic test's parameters, through the macro rewrite
        (
            "{% test not_neg(model, @column_name) %}{{ column_name }}{% endtest %}",
            Some("{% test not_neg(model, Z) %}{{ Z }}{% endtest %}"),
        ),
        // not a local: a context name, a callee, an attribute
        ("{{ @target.name }}", None),
        ("{{ @ref('x') }}", None),
        ("{% set t = 1 %}{{ t.@name }}", None),
    ] {
        assert_eq!(renamed(marked).as_deref(), want, "{marked}");
    }
}

#[test]
fn an_unparsable_template_has_no_locals() {
    assert!(analyze("{% set x = %}").is_none());
    assert!(analyze("{% for x in y %}").is_none());
}

#[test]
fn collisions_are_found_where_the_new_name_is_bound_or_used() {
    for (marked, new, collides) in [
        ("{% set @x = 1 %}{{ x }}", "y", false),
        ("{% set @x = 1 %}{% set y = 2 %}", "y", true),
        ("{% set @x = 1 %}{{ y }}", "y", true),
        // an inner binding of the new name would shadow the renamed one
        (
            "{% set @x = 1 %}{% for y in r %}{{ x }}{% endfor %}",
            "y",
            true,
        ),
        // a loop var may take a name bound only outside where unused inside
        (
            "{% set y = 1 %}{% for @c in r %}{{ c }}{% endfor %}",
            "y",
            false,
        ),
        // … but not one read inside the loop (it would be captured)
        (
            "{% set y = 1 %}{% for @c in r %}{{ c }}{{ y }}{% endfor %}",
            "y",
            true,
        ),
        ("{% macro m(@a, b) %}{{ a }}{% endmacro %}", "b", true),
        ("{% macro m(@a, b) %}{{ a }}{% endmacro %}", "c", false),
    ] {
        let at = marked.find('@').unwrap() as u32;
        let text = marked.replacen('@', "", 1);
        let a = analyze(&text).unwrap();
        let idx = a.at(at).unwrap();
        assert_eq!(
            a.collision(idx, new).is_some(),
            collides,
            "{marked} → {new}"
        );
    }
}

#[test]
fn calls_remember_where_their_arguments_start() {
    let text = "{{ m(b=1) }}{% call pkg.m(b=2) %}x{% endcall %}{% set v = m(3) %}";
    let a = analyze(text).unwrap();
    let starts: Vec<&str> = a
        .calls
        .iter()
        .filter(|c| c.name == "m")
        .map(|c| &text[c.after_callee as usize..c.after_callee as usize + 1])
        .collect();
    assert_eq!(starts, vec!["(", "(", "("]);
}
