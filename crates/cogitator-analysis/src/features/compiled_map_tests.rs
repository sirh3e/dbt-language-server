use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rstest::rstest;

use super::*;
use crate::features::{expand_jinja, PreviewInputs};

fn lines(start: u32, end: u32, exact: bool) -> SourceLines {
    SourceLines { start, end, exact }
}

#[rstest]
// copied through: every line exact
#[case("select 1\nfrom t\n", "select 1\nfrom t\n", vec![lines(0, 0, true), lines(1, 1, true)])]
// a ref rendered on its own line, whitespace ignored
#[case(
    "select *\n  from {{ ref('a') }}\nwhere x",
    "select *\nfrom \"db\".\"main\".\"a\"\nwhere x",
    vec![lines(0, 0, true), lines(1, 1, false), lines(2, 2, true)]
)]
// a config line that rendered to nothing: the compiled text is one shorter
#[case(
    "{{ config(materialized='table') }}\nselect 1\nfrom t",
    "\nselect 1\nfrom t",
    vec![lines(0, 0, false), lines(1, 1, true), lines(2, 2, true)]
)]
// a loop that unrolled: every unrolled line maps to the whole loop
#[case(
    "select\n{% for c in cs %}\n  {{ c }},\n{% endfor %}\n1 as one",
    "select\n  a,\n  b,\n1 as one",
    vec![
        lines(0, 0, true),
        lines(1, 3, false),
        lines(1, 3, false),
        lines(4, 4, true),
    ]
)]
// a line only the compiled text has: the source line before it
#[case("select 1\nfrom t", "select 1\n-- added\nfrom t", vec![lines(0, 0, true), lines(0, 0, false), lines(1, 1, true)])]
fn compiled_lines_map_back_to_their_source(
    #[case] source: &str,
    #[case] compiled: &str,
    #[case] expected: Vec<SourceLines>,
) {
    assert_eq!(compiled_line_map(source, compiled), expected);
}

#[rstest]
#[case(
    "/p/target/compiled/jaffle_shop/models/customers.sql",
    Some("/p/models/customers.sql")
)]
#[case(
    "/p/target/run/jaffle_shop/models/staging/stg.sql",
    Some("/p/models/staging/stg.sql")
)]
#[case(
    "/p/target/compiled/jaffle_package/models/pkg.sql",
    Some("/p/dbt_packages/jaffle_package/models/pkg.sql")
)]
#[case("/p/models/customers.sql", None)]
#[case("/p/target/compiled/jaffle_shop", None)]
fn a_compiled_path_names_its_source(#[case] compiled: &str, #[case] expected: Option<&str>) {
    assert_eq!(
        compiled_source_path(Path::new("/p"), "jaffle_shop", Path::new(compiled)),
        expected.map(PathBuf::from)
    );
}

fn fixture(rel: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/jaffle_shop_duckdb");
    std::fs::read_to_string(root.join(rel)).unwrap()
}

/// The jaffle models, rendered by the Jinja preview as the "compiled"
/// text: every line holding a rendered relation maps to the source line
/// holding its ref(), every line copied through maps exactly to itself.
#[rstest]
#[case("models/customers.sql")]
#[case("models/orders.sql")]
#[case("models/staging/stg_orders.sql")]
fn the_rendered_jaffle_models_map_back_to_their_lines(#[case] model: &str) {
    let source = fixture(model);
    let refs = [
        "stg_customers",
        "stg_orders",
        "stg_payments",
        "stg_customer_status",
        "raw_orders",
        "raw_customers",
        "raw_payments",
    ];
    let inputs = PreviewInputs {
        macro_sources: vec![fixture("macros/jaffle_macros.sql")],
        model_relations: refs
            .iter()
            .map(|r| (r.to_string(), format!("\"jaffle\".\"main\".\"{r}\"")))
            .collect(),
        source_relations: HashMap::new(),
        vars: HashMap::new(),
        target_name: "dev".into(),
        is_incremental: false,
        this: None,
    };
    let Ok(compiled) = expand_jinja(&source, &inputs) else {
        return; // a model the preview cannot render is no evidence either way
    };
    let map = compiled_line_map(&source, &compiled.sql);
    let src: Vec<&str> = source.lines().collect();
    for (i, line) in compiled.sql.lines().enumerate() {
        let at = map[i];
        assert!(
            at.start <= at.end && (at.end as usize) < src.len(),
            "{model}:{i} {at:?}"
        );
        if at.exact {
            assert_eq!(line.trim(), src[at.start as usize].trim(), "{model}:{i}");
        }
        for r in refs {
            if line.contains(&format!("\"{r}\"")) {
                let shown = &src[at.start as usize..=at.end as usize];
                assert!(
                    shown.iter().any(|s| s.contains(&format!("ref('{r}')"))),
                    "{model}:{i} {line:?} → {shown:?}"
                );
            }
        }
    }
}

#[test]
fn the_hover_links_the_source_and_shows_its_lines() {
    let hover = compiled_hover(
        lines(1, 1, false),
        "select *\nfrom {{ ref('a') }}\n",
        "file:///p/models/a.sql",
        "models/a.sql",
    );
    assert_eq!(
        hover,
        "**Compiled from** [`models/a.sql` line 2](file:///p/models/a.sql#L2) (rendered)\n\n```jinja\nfrom {{ ref('a') }}\n```"
    );
    let many = compiled_hover(lines(0, 30, false), &"x\n".repeat(40), "u", "m.sql");
    assert!(many.contains("lines 1–31"), "{many}");
    assert!(many.ends_with("x\n…\n```"), "{many}");
    assert_eq!(
        compiled_definition(lines(4, 6, false), "file:///p/m.sql")
            .range
            .start,
        Position {
            line: 4,
            character: 0
        }
    );
}
