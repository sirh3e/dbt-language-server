//! `function-paths` of dbt_project.yml (user-defined functions, dbt 1.11+).

use std::path::Path;

use cogitator_schemas::parse_dbt_project;
use cogitator_schemas::project::apply_defaults;

#[test]
fn function_paths_case_table() {
    // (project text, directories under the root, the paths)
    let cases: &[(&str, &[&str], &[&str])] = &[
        ("name: p\n", &[], &[]),
        ("name: p\n", &["functions"], &["functions"]),
        (
            "name: p\nfunction-paths: [udfs]\n",
            &["functions"],
            &["udfs"],
        ),
        ("name: p\nfunction-paths: udfs\n", &[], &["udfs"]),
        // a list of no strings is none, and the default applies
        (
            "name: p\nfunction-paths: [1]\n",
            &["functions"],
            &["functions"],
        ),
    ];
    for (text, dirs, want) in cases {
        let mut file = parse_dbt_project(text, Path::new("dbt_project.yml")).unwrap();
        let dirs: Vec<String> = dirs.iter().map(|d| d.to_string()).collect();
        apply_defaults(&mut file, &dirs);
        assert_eq!(file.function_paths, *want, "{text} {dirs:?}");
    }
}
