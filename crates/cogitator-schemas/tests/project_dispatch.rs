//! `dispatch:` of dbt_project.yml (macro dispatch search order).

use std::path::Path;

use cogitator_schemas::{parse_dbt_project, DispatchConfig};

#[test]
fn dispatch_entries_case_table() {
    let cases: &[(&str, Vec<DispatchConfig>)] = &[
        ("name: p\n", vec![]),
        (
            "name: p\ndispatch:\n  - macro_namespace: dbt_utils\n    search_order: [p, dbt_utils]\n",
            vec![DispatchConfig {
                macro_namespace: "dbt_utils".into(),
                search_order: vec!["p".into(), "dbt_utils".into()],
            }],
        ),
        // an entry without a namespace, or a search order that is no list, is dropped
        (
            "name: p\ndispatch:\n  - search_order: [p]\n  - macro_namespace: x\n    search_order: p\n",
            vec![],
        ),
    ];
    for (text, want) in cases {
        let file = parse_dbt_project(text, Path::new("dbt_project.yml")).unwrap();
        assert_eq!(&file.dispatch, want, "{text}");
    }
}
