//! dbt.impact's column hits through a model that reads a sibling: both are
//! one hop downstream of `base`, and the reader's column comes from the
//! sibling, which sorts after it by name. The old taint pass walked models
//! in (depth, name) order and missed the reader's column.

use std::collections::HashMap;

use cogitator_analysis::lineage::column_lineage;
use cogitator_analysis::{build_context, features};
use pretty_assertions::assert_eq;

fn project() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("cogitator-impact-order-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("models")).unwrap();
    std::fs::write(
        dir.join("dbt_project.yml"),
        "name: order\nversion: '1.0'\nprofile: order\nmodel-paths: ['models']\n",
    )
    .unwrap();
    for (name, sql) in [
        ("base", "select 1 as x"),
        ("z_sibling", "select b.x from {{ ref('base') }} as b"),
        (
            "a_reader",
            "select s.x from {{ ref('z_sibling') }} as s join {{ ref('base') }} as b on s.x = b.x",
        ),
    ] {
        std::fs::write(dir.join(format!("models/{name}.sql")), sql).unwrap();
    }
    dir
}

#[test]
fn a_column_read_through_a_sibling_is_hit() {
    let dir = project();
    let ctx = build_context(dir.to_str().unwrap());
    let lineages: HashMap<String, _> = features::impact_targets(&ctx, "base")
        .into_iter()
        .filter_map(|(name, path)| {
            let text = std::fs::read_to_string(path).ok()?;
            let parsed = cogitator_parser::pipeline::parse_document(&text, &ctx.dialect).ok()?;
            Some((name, column_lineage(&parsed, &ctx.dialect)?))
        })
        .collect();
    let report = features::impact(&ctx, "base", Some("x"), &lineages);
    let depths: Vec<(String, usize)> = report
        .models
        .iter()
        .map(|m| (m.name.clone(), m.depth))
        .collect();
    assert_eq!(
        depths,
        [("a_reader".to_string(), 1), ("z_sibling".to_string(), 1)]
    );
    let hits: Vec<String> = report
        .column_hits
        .iter()
        .map(|h| format!("{}.{}", h.model, h.column))
        .collect();
    assert_eq!(hits, ["a_reader.x", "z_sibling.x"]);
    let _ = std::fs::remove_dir_all(&dir);
}
