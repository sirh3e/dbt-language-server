//! `dbt.unresolved`'s core: every unresolved ref, source, source table,
//! doc, macro and var, per document and for the whole project, on the
//! jaffle fixture.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use cogitator_analysis::features::{self, UnresolvedKind as K};
use cogitator_analysis::{build_context, Documents};
use pretty_assertions::assert_eq;
use rstest::rstest;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/jaffle_shop_duckdb")
}

fn found(uri: &str, text: &str) -> Vec<(K, String)> {
    let ctx = build_context(fixture_root().to_str().unwrap());
    let mut docs = Documents::default();
    docs.open(uri.into(), text, &ctx.dialect);
    features::document_unresolved(&ctx, uri, docs.get(uri).unwrap())
        .into_iter()
        .map(|u| (u.kind, u.name))
        .collect()
}

#[rstest]
#[case("select * from {{ ref('customers') }}", vec![])]
#[case("select * from {{ ref('nope') }}", vec![(K::Ref, "nope")])]
#[case("select * from {{ source('nope', 'x') }}", vec![(K::Source, "nope")])]
#[case("select * from {{ source('jaffle_shop', 'nope') }}", vec![(K::SourceTable, "nope")])]
#[case("select {{ var('nope') }}", vec![(K::Var, "nope")])]
#[case("select {{ var('nope', 1) }}", vec![])]
#[case("select {{ var('global_count') }}", vec![])]
#[case("select {{ full_name('a', 'b') }}", vec![])]
#[case("select {{ no_such_macro() }}", vec![(K::Macro, "no_such_macro")])]
#[case("select {{ jaffle_shop.no_such_macro() }}", vec![(K::Macro, "no_such_macro")])]
#[case("select {{ dbt.whatever() }}", vec![])]
#[case("select {{ current_timestamp() }}, {{ is_incremental() }}", vec![])]
#[case(
    "select {{ nope_m() }} from {{ ref('nope') }}",
    vec![(K::Macro, "nope_m"), (K::Ref, "nope")]
)]
fn a_sql_document_reports_what_does_not_resolve(
    #[case] text: &str,
    #[case] expected: Vec<(K, &str)>,
) {
    let expected: Vec<_> = expected
        .into_iter()
        .map(|(k, n)| (k, n.to_string()))
        .collect();
    assert_eq!(found("file:///p/models/x.sql", text), expected);
}

#[test]
fn a_property_file_reports_its_dangling_doc_blocks() {
    let text =
        "version: 2\nmodels:\n  - name: customers\n    description: '{{ doc(\"no_such_doc\") }}'\n";
    assert_eq!(
        found("file:///p/models/x.yml", text),
        vec![(K::Doc, "no_such_doc".to_string())]
    );
}

#[test]
fn the_fixture_project_resolves_everything() {
    let ctx = build_context(fixture_root().to_str().unwrap());
    let read = |p: &Path| std::fs::read_to_string(p).ok().map(Arc::from);
    assert!(features::project_files(&ctx).len() > 5);
    assert_eq!(features::project_unresolved(&ctx, &read), vec![]);
}

#[test]
fn the_project_list_reads_open_documents_through_the_seam() {
    let ctx = build_context(fixture_root().to_str().unwrap());
    let customers = fixture_root()
        .canonicalize()
        .unwrap()
        .join("models/customers.sql");
    let read = |p: &Path| {
        if p.ends_with("models/customers.sql") {
            Some(Arc::from("select * from {{ ref('ghost') }}"))
        } else {
            std::fs::read_to_string(p).ok().map(Arc::from)
        }
    };
    let found = features::project_unresolved(&ctx, &read);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].kind, K::Ref);
    assert_eq!(found[0].name, "ghost");
    assert!(
        found[0].uri.ends_with("models/customers.sql"),
        "{customers:?}"
    );
    assert!(found[0].message.contains("ghost"));
}
