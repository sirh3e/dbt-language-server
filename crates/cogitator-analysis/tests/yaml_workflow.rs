//! Extract-model-to-YAML and test navigation (dbt.extractModelYaml,
//! dbt.modelTests): scaffold content, version edits that stay valid YAML,
//! and version-aware test lookup.

use std::path::PathBuf;

use cogitator_analysis::features::{extract_model_yaml, model_tests, ExtractEdit};
use cogitator_analysis::{build_context, features};
use cogitator_lsp::Range;
use pretty_assertions::assert_eq;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/jaffle_shop_duckdb")
}

fn fixture_uri(rel: &str) -> String {
    cogitator_lsp::fs_path_to_uri(&fixture_root().join(rel))
}

/// Apply (Range, text) edits to `text` — inserts at line starts and
/// single-line replacements, applied bottom-up.
fn apply_edits(text: &str, edits: &[(Range, String)]) -> String {
    let mut lines: Vec<String> = text.split('\n').map(str::to_string).collect();
    let mut sorted: Vec<&(Range, String)> = edits.iter().collect();
    sorted.sort_by_key(|(r, _)| {
        (
            std::cmp::Reverse(r.start.line),
            std::cmp::Reverse(r.start.character),
        )
    });
    for (range, new_text) in sorted {
        let line = range.start.line as usize;
        if range.start == range.end && range.start.character == 0 {
            for (offset, inserted) in new_text.trim_end_matches('\n').split('\n').enumerate() {
                lines.insert(line + offset, inserted.to_string());
            }
        } else {
            let target = &mut lines[line];
            let (start, end) = (range.start.character as usize, range.end.character as usize);
            target.replace_range(start..end.min(target.len()), new_text);
        }
    }
    lines.join("\n")
}

#[test]
fn tests_navigation_finds_column_tests() {
    let ctx = build_context(fixture_root().to_str().unwrap());
    let (model, tests) =
        model_tests(&ctx, &fixture_uri("models/customers.sql")).expect("tests for customers");
    assert_eq!(model, "customers");
    let names: Vec<(&str, Option<&str>)> = tests
        .iter()
        .map(|t| (t.name.as_str(), t.column.as_deref()))
        .collect();
    assert!(
        names.contains(&("unique", Some("customer_id"))),
        "{names:?}"
    );
    assert!(
        names.contains(&("not_null", Some("customer_id"))),
        "{names:?}"
    );
    assert!(tests.iter().all(|t| t.uri.ends_with("schema.yml")));
    assert!(tests.iter().all(|t| t.range.start.line > 0));
}

#[test]
fn versioned_file_resolves_to_its_base_model() {
    let ctx = build_context(fixture_root().to_str().unwrap());
    let (model, tests) =
        model_tests(&ctx, &fixture_uri("models/customers_v2.sql")).expect("base model tests");
    assert_eq!(model, "customers");
    assert!(!tests.is_empty());
}

#[test]
fn documented_model_gets_a_first_version() {
    let ctx = build_context(fixture_root().to_str().unwrap());
    let edit = extract_model_yaml(&ctx, &fixture_uri("models/customers.sql"), None)
        .expect("extract on documented model");
    let ExtractEdit::AddVersion {
        schema_path,
        edits,
        next,
    } = edit
    else {
        panic!("expected AddVersion for a documented model");
    };
    assert_eq!(next, 1);
    let text = std::fs::read_to_string(&schema_path).unwrap();
    let applied = apply_edits(&text, &edits);
    assert!(applied.contains("latest_version: 1"), "{applied}");
    assert!(applied.contains("- v: 1"), "{applied}");
    // the result is still valid YAML and the entry carries the version
    let docs = saphyr_check(&applied);
    assert!(docs, "edited schema must stay parseable yaml");
}

fn saphyr_check(text: &str) -> bool {
    cogitator_schemas::yaml::syntax_error(text).is_none()
}

/// A throwaway dbt project with one documented+versioned model and one
/// undocumented model.
fn temp_project() -> PathBuf {
    let root = std::env::temp_dir().join(format!("cogitator-extract-{}", std::process::id()));
    let models = root.join("models");
    std::fs::create_dir_all(&models).unwrap();
    std::fs::write(
        root.join("dbt_project.yml"),
        "name: t\nprofile: t\nmodel-paths: [\"models\"]\n",
    )
    .unwrap();
    std::fs::write(models.join("m.sql"), "select 1 as id, 'x' as label\n").unwrap();
    std::fs::write(
        models.join("m.yml"),
        "version: 2\n\nmodels:\n  - name: m\n    latest_version: 1\n    versions:\n      - v: 1\n",
    )
    .unwrap();
    std::fs::write(models.join("fresh.sql"), "select 1 as id, 'x' as label\n").unwrap();
    root
}

#[test]
fn existing_versions_get_the_next_one() {
    let root = temp_project();
    let ctx = build_context(root.to_str().unwrap());
    let uri = cogitator_lsp::fs_path_to_uri(&root.join("models/m.sql"));
    let ExtractEdit::AddVersion {
        schema_path,
        edits,
        next,
    } = extract_model_yaml(&ctx, &uri, None).expect("extract")
    else {
        panic!("expected AddVersion");
    };
    assert_eq!(next, 2);
    let applied = apply_edits(&std::fs::read_to_string(&schema_path).unwrap(), &edits);
    assert!(applied.contains("latest_version: 2"), "{applied}");
    assert!(applied.contains("- v: 1"), "{applied}");
    assert!(applied.contains("- v: 2"), "{applied}");
    assert!(saphyr_check(&applied), "{applied}");
}

#[test]
fn undocumented_model_gets_a_scaffold_with_its_columns() {
    let root = temp_project();
    let ctx = build_context(root.to_str().unwrap());
    let uri = cogitator_lsp::fs_path_to_uri(&root.join("models/fresh.sql"));
    let parsed = cogitator_parser::pipeline::parse_document(
        &std::fs::read_to_string(root.join("models/fresh.sql")).unwrap(),
        &ctx.dialect,
    )
    .unwrap();
    let lineage = cogitator_analysis::lineage::column_lineage(&parsed, &ctx.dialect);
    let ExtractEdit::CreateFile { path, content } =
        extract_model_yaml(&ctx, &uri, lineage.as_ref()).expect("extract")
    else {
        panic!("expected CreateFile for an undocumented model");
    };
    assert!(path.ends_with("fresh.yml"), "{}", path.display());
    assert!(content.contains("- name: fresh"), "{content}");
    assert!(content.contains("- name: id"), "{content}");
    assert!(content.contains("- name: label"), "{content}");
    assert!(saphyr_check(&content), "{content}");

    // lenses: undocumented → scaffold lens; documented → test-count lens
    let lenses = features::code_lenses(&ctx, &uri);
    assert!(
        lenses
            .iter()
            .any(|l| l.command.title == "Extract schema YAML"),
        "{lenses:?}"
    );
}
