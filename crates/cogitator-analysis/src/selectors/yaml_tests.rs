use super::*;

/// The expression as a compact string, for case tables.
fn show(e: &Expr) -> String {
    match e {
        Expr::Criterion {
            criterion: c,
            indirect,
            ..
        } => {
            let mut s = format!("{}:{}", c.method.name(), c.value);
            if c.childrens_parents {
                s = format!("@{s}");
            }
            if let Some(d) = c.parents {
                s = format!("{}+{s}", d.map(|d| d.to_string()).unwrap_or_default());
            }
            if let Some(d) = c.children {
                s = format!("{s}+{}", d.map(|d| d.to_string()).unwrap_or_default());
            }
            if let Some(i) = indirect {
                s = format!("{s}[{i:?}]");
            }
            s
        }
        Expr::Union(parts) if parts.len() == 1 => show(&parts[0]),
        Expr::Intersection(parts) if parts.len() == 1 => show(&parts[0]),
        Expr::Union(parts) => format!(
            "({})",
            parts.iter().map(show).collect::<Vec<_>>().join(" | ")
        ),
        Expr::Intersection(parts) => {
            format!(
                "({})",
                parts.iter().map(show).collect::<Vec<_>>().join(" & ")
            )
        }
        Expr::Difference(a, b) => format!("({} - {})", show(a), show(b)),
        Expr::Invalid => "!".into(),
    }
}

fn definition(body: &str) -> (String, Vec<String>) {
    let text = format!("selectors:\n  - name: s\n    definition:\n{body}");
    let file = parse(&text);
    (
        show(&file.definitions[0].expr),
        file.problems.into_iter().map(|p| p.message).collect(),
    )
}

#[test]
fn definition_forms() {
    let cases: &[(&str, &str)] = &[
        ("      'tag:nightly'\n", "tag:nightly"),
        ("      '+customers stg_orders,tag:x'\n", "(+fqn:customers | (fqn:stg_orders & tag:x))"),
        ("      method: tag\n      value: nightly\n", "tag:nightly"),
        ("      tag: nightly\n", "tag:nightly"),
        (
            "      method: fqn\n      value: orders\n      parents: true\n      children_depth: 2\n",
            "+fqn:orders+2",
        ),
        ("      method: fqn\n      value: orders\n      childrens_parents: true\n", "@fqn:orders"),
        (
            "      method: tag\n      value: x\n      indirect_selection: cautious\n",
            "tag:x[Cautious]",
        ),
        (
            "      union:\n        - tag:a\n        - method: path\n          value: models/staging\n        - exclude:\n            - fqn: stg_payments\n",
            "((tag:a | path:models/staging) - fqn:stg_payments)",
        ),
        (
            "      intersection:\n        - 'tag:a'\n        - 'config.materialized:table'\n",
            "(tag:a & config.materialized:table)",
        ),
        (
            "      method: tag\n      value: a\n      exclude:\n        - method: fqn\n          value: b\n",
            "(tag:a - fqn:b)",
        ),
        ("      method: selector\n      value: other\n", "selector:other"),
        ("      method: config.materialized\n      value: table\n", "config.materialized:table"),
    ];
    for (body, expected) in cases {
        let (got, problems) = definition(body);
        assert_eq!(&got, expected, "{body}");
        assert!(problems.is_empty(), "{body}: {problems:?}");
    }
}

#[test]
fn problems_are_reported() {
    let cases: &[(&str, &str)] = &[
        ("      method: tag\n", "method `tag` has no `value:`"),
        (
            "      method: tag\n      value: x\n      kids: true\n",
            "unknown key `kids` in a method mapping",
        ),
        (
            "      'a @+b'\n",
            "`@+b`: `@` and a parents `+` cannot be combined",
        ),
        (
            "      union:\n        - a\n        - exclude: [b]\n        - exclude: [c]\n",
            "only one `exclude:` per union or intersection",
        ),
        (
            "      method: tag\n      value: x\n      indirect_selection: sometimes\n",
            "indirect_selection is eager, cautious, buildable or empty",
        ),
    ];
    for (body, expected) in cases {
        let (_, problems) = definition(body);
        assert_eq!(problems, vec![expected.to_string()], "{body}");
    }
}

#[test]
fn names_defaults_and_ranges() {
    let text = "selectors:\n  - name: a\n    default: true\n    definition: 'tag:x  b'\n  - name: a\n    default: true\n    definition: b\n";
    let file = parse(text);
    assert_eq!(file.definitions.len(), 2);
    assert!(file.definitions[0].default);
    let messages: Vec<_> = file.problems.iter().map(|p| p.message.as_str()).collect();
    assert_eq!(
        messages,
        vec![
            "selector `a` is defined twice",
            "only one selector may set `default: true`"
        ]
    );
    // the string's criteria point at their own text, past the quote
    let Expr::Union(parts) = &file.definitions[0].expr else {
        panic!()
    };
    let Expr::Intersection(first) = &parts[1] else {
        panic!()
    };
    let Expr::Criterion { value_range, .. } = &first[0] else {
        panic!()
    };
    assert_eq!(
        (value_range.start.line, value_range.start.character),
        (3, 24)
    );
    assert_eq!(value_range.end.character, 25);
}
