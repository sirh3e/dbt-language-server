//! dbt.select over a project in a real one's shape (testdata/vault_lake)
//! and jaffle_shop: every method, graph and set operator, globs,
//! selectors.yml, --exclude, eager test inclusion, the refusals, and
//! parse errors with their spans. Expected sets are what dbt answers by
//! dbt-core's rules (graph/selector.py, selector_methods.py).

use std::path::PathBuf;

use cogitator_analysis::features::selection::{self, build_universe};
use cogitator_analysis::{build_context, DbtContext};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata")
        .join(name)
}

fn project(name: &str) -> (DbtContext, selection::Universe) {
    let root = fixture(name);
    let ctx = build_context(root.to_str().unwrap());
    let manifest = std::fs::read_to_string(root.join("target/manifest.json"))
        .ok()
        .and_then(|t| {
            cogitator_analysis::artifacts::manifest::parse(&t, &root.join("target/manifest.json"))
                .ok()
        });
    let universe = build_universe(&ctx, manifest.as_ref());
    (ctx, universe)
}

/// `type:name` of what `text` selects, in the answer's order.
fn picked(ctx: &DbtContext, u: &selection::Universe, text: &str) -> Vec<String> {
    let result = selection::select(ctx, u, text, None);
    if let Some(e) = result.error {
        panic!("{text}: {} at {}..{}", e.message, e.start, e.end);
    }
    result
        .nodes
        .iter()
        .map(|n| format!("{}:{}", n.resource_type, n.name))
        .collect()
}

/// dim_items' tests: eager indirect selection takes them in with it.
const DIM_TESTS: &[&str] = &[
    "test:assert_items_have_one_current_state",
    "test:assert_no_state_goes_back_to_held",
    "test:dbt_utils_unique_combination_of_columns_dim_items_item_id__item_version_id",
    "test:not_null_dim_items_item_version_id",
    "test:unique_dim_items_item_version_id",
    "unit_test:test_dim_items_keeps_the_previous_state",
];

fn with_dim_tests(nodes: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = nodes.iter().map(|s| s.to_string()).collect();
    out.extend(DIM_TESTS.iter().map(|s| s.to_string()));
    out
}

fn v(names: &[&str]) -> Vec<String> {
    names.iter().map(|s| s.to_string()).collect()
}

fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

#[rstest::rstest]
// fqn: a name, a folder (fqn without the package), the package, globs
#[case("dim_items", with_dim_tests(&["model:dim_items"]))]
#[case("dim", with_dim_tests(&["model:dim_items"]))]
#[case("vault_lake.dim", with_dim_tests(&["model:dim_items"]))]
#[case("raw_vault", v(&["model:hub_item", "model:sat_item_version"]))]
// graph operators
#[case("+dim_items", with_dim_tests(&["model:bv_item_transitions", "model:dim_items", "model:hub_item", "model:sat_item_version", "source:item_versions", "source:items"]))]
#[case("1+dim_items", with_dim_tests(&["model:bv_item_transitions", "model:dim_items", "model:hub_item"]))]
#[case("sat_item_version+1", v(&["model:bv_item_transitions", "model:sat_item_version", "exposure:item_history_feed"]))]
#[case("@sat_item_version", with_dim_tests(&["model:bv_item_transitions", "model:dim_items", "model:hub_item", "model:sat_item_version", "source:item_versions", "source:items", "exposure:item_history_feed"]))]
// tags: folder +tags, YAML tags, SQL config(tags=), a seed's, a snapshot's
#[case("tag:raw", v(&["model:hub_item", "model:sat_item_version"]))]
#[case("tag:nightly", with_dim_tests(&["model:dim_items"]))]
#[case("tag:hourly", v(&["model:bv_item_transitions"]))]
#[case("tag:reference", v(&["seed:item_states"]))]
#[case("tag:history", v(&["snapshot:items_snapshot"]))]
#[case("tag:hou*", v(&["model:bv_item_transitions"]))]
// resource types: unreferenced seeds and snapshots are nodes too
#[case("resource_type:seed", v(&["seed:item_states"]))]
#[case("resource_type:snapshot", v(&["snapshot:items_snapshot"]))]
#[case("resource_type:exposure", v(&["exposure:item_history_feed"]))]
#[case("resource_type:unit_test", v(&["unit_test:test_dim_items_keeps_the_previous_state"]))]
#[case("resource_type:test", DIM_TESTS[..5].iter().map(|s| s.to_string()).collect())]
// paths and files: a properties file's sources and exposures live under it
#[case("models/raw_vault", v(&["model:hub_item", "model:sat_item_version", "source:item_versions", "source:items", "exposure:item_history_feed"]))]
#[case("path:models/dim/*.sql", with_dim_tests(&["model:dim_items"]))]
#[case("dim_items.sql", with_dim_tests(&["model:dim_items"]))]
#[case("file:hub_item", v(&["model:hub_item"]))]
// configs: the manifest's materialized wins, then config(), then the folder
#[case("config.materialized:table", v(&["model:bv_item_transitions"]))]
#[case("config.materialized:incremental", with_dim_tests(&["model:dim_items", "model:hub_item", "model:sat_item_version"]))]
#[case("config.materialized:seed", v(&["seed:item_states"]))]
// sources, exposures, tests
#[case("source:landing", v(&["source:item_versions", "source:items"]))]
#[case("source:landing.items+", with_dim_tests(&["model:dim_items", "model:hub_item", "source:items"]))]
#[case("+exposure:item_history_feed", v(&["model:sat_item_version", "source:item_versions", "exposure:item_history_feed"]))]
#[case("test_type:singular", DIM_TESTS[..2].iter().map(|s| s.to_string()).collect())]
#[case("test_type:unit", v(&["unit_test:test_dim_items_keeps_the_previous_state"]))]
#[case("test_name:not_null", v(&["test:not_null_dim_items_item_version_id"]))]
#[case("test_name:unique_combination_of_columns", v(&["test:dbt_utils_unique_combination_of_columns_dim_items_item_id__item_version_id"]))]
// selectors.yml: a string, a union with an exclusion, parents
#[case("selector:raw_layer", v(&["model:hub_item", "model:sat_item_version"]))]
#[case("selector:marts_without_tests", v(&["model:dim_items"]))]
#[case("selector:what_dim_items_reads", with_dim_tests(&["model:bv_item_transitions", "model:dim_items", "model:hub_item", "model:sat_item_version", "source:item_versions", "source:items"]))]
// set operators
#[case("tag:raw tag:hourly", v(&["model:bv_item_transitions", "model:hub_item", "model:sat_item_version"]))]
#[case("tag:lake,config.materialized:table", v(&["model:bv_item_transitions"]))]
#[case("tag:lake,tag:mart", with_dim_tests(&["model:dim_items"]))]
#[case("package:this,resource_type:source", v(&["source:item_versions", "source:items"]))]
#[case("tag:lake --exclude resource_type:test", v(&["model:bv_item_transitions", "model:dim_items", "model:hub_item", "model:sat_item_version", "unit_test:test_dim_items_keeps_the_previous_state"]))]
#[case("tag:lake --exclude dim_items", v(&["model:bv_item_transitions", "model:hub_item", "model:sat_item_version"]))]
fn vault_lake_selects_as_dbt_does(#[case] text: &str, #[case] expected: Vec<String>) {
    let (ctx, u) = project("vault_lake");
    assert_eq!(sorted(picked(&ctx, &u, text)), sorted(expected), "{text}");
}

#[test]
fn the_answer_is_ordered_by_resource_type_and_counted() {
    let (ctx, u) = project("vault_lake");
    let result = selection::select(&ctx, &u, "+dim_items", None);
    let types: Vec<&str> = result.nodes.iter().map(|n| n.resource_type).collect();
    let mut expected = types.clone();
    let order = ["model", "seed", "snapshot", "source", "test", "unit_test"];
    expected.sort_by_key(|t| order.iter().position(|o| o == t));
    assert_eq!(types, expected);
    assert_eq!(result.counts.get("model"), Some(&4));
    assert_eq!(result.counts.get("test"), Some(&5));
    let dim = result.nodes.iter().find(|n| n.name == "dim_items").unwrap();
    assert_eq!(dim.unique_id, "model.vault_lake.dim_items");
    assert_eq!(dim.graph_id.as_deref(), Some("model:dim_items"));
    assert!(dim
        .uri
        .as_deref()
        .is_some_and(|u| u.ends_with("models/dim/dim_items.sql")));
    let generic = result
        .nodes
        .iter()
        .find(|n| n.name == "not_null_dim_items_item_version_id")
        .unwrap();
    assert!(
        generic.range.is_some() && generic.graph_id.is_none(),
        "a YAML test opens at its line"
    );
}

#[test]
fn an_exclusion_may_come_on_its_own() {
    let (ctx, u) = project("vault_lake");
    let result = selection::select(&ctx, &u, "tag:raw", Some("hub_item"));
    let names: Vec<&str> = result.nodes.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(names, vec!["sat_item_version"]);
}

#[rstest::rstest]
#[case("tag:", None, 0, 4, "needs a value")]
#[case("tag:raw bogus:x", None, 8, 14, "no selection method")]
#[case("state:modified+", None, 0, 15, "previous dbt run")]
#[case("resource_type:modle", None, 0, 19, "no resource type")]
#[case("config.schema:dim", None, 0, 17, "not known to the editor")]
#[case("selector:nope", None, 0, 13, "has no selector")]
#[case("tag:raw --exclude tag:", None, 18, 22, "needs a value")]
#[case(
    "tag:raw --exclude resource_type:nope",
    None,
    18,
    36,
    "no resource type"
)]
#[case("tag:raw", Some("x,,y"), 2, 3, "empty criterion")]
fn errors_say_what_and_where(
    #[case] text: &str,
    #[case] exclude: Option<&str>,
    #[case] start: usize,
    #[case] end: usize,
    #[case] says: &str,
) {
    let (ctx, u) = project("vault_lake");
    let result = selection::select(&ctx, &u, text, exclude);
    let error = result.error.expect("an error");
    assert!(error.message.contains(says), "{}", error.message);
    assert_eq!((error.start, error.end), (start, end), "{}", error.message);
    assert!(result.nodes.is_empty());
}

#[test]
fn jaffle_shops_selectors_and_tags() {
    let (ctx, u) = project("jaffle_shop_duckdb");
    let nightly = picked(&ctx, &u, "selector:nightly");
    assert!(
        nightly.contains(&"model:customers".to_string()),
        "{nightly:?}"
    );
    assert_eq!(
        picked(&ctx, &u, "resource_type:seed"),
        vec!["seed:raw_customers", "seed:raw_orders", "seed:raw_payments"]
    );
    assert_eq!(
        picked(&ctx, &u, "resource_type:snapshot"),
        vec!["snapshot:orders_snapshot"]
    );
    let staging = picked(&ctx, &u, "staging,resource_type:model");
    assert!(
        staging
            .iter()
            .all(|n| n.starts_with("model:stg_") || n.starts_with("test:")),
        "{staging:?}"
    );
    assert!(
        staging.contains(&"model:stg_orders".to_string()),
        "{staging:?}"
    );
}

#[test]
fn the_vocabulary_offers_what_the_project_has() {
    let (ctx, u) = project("vault_lake");
    let vocab = selection::vocabulary(&ctx, &u);
    assert!(vocab.methods.contains(&"tag") && vocab.methods.contains(&"config.materialized"));
    let tags = &vocab.values["tag"];
    for tag in [
        "history",
        "hourly",
        "lake",
        "mart",
        "nightly",
        "raw",
        "reference",
    ] {
        assert!(tags.contains(&tag.to_string()), "{tag} in {tags:?}");
    }
    assert!(vocab.values["selector"].contains(&"marts_without_tests".to_string()));
    assert!(vocab.values["source"].contains(&"landing.items".to_string()));
    assert!(vocab.values["path"].contains(&"models/dim".to_string()));
}

#[test]
fn no_text_makes_it_panic() {
    // every string of up to 6 characters over the grammar's alphabet,
    // sampled: the parser and evaluator answer or refuse, never panic
    let (ctx, u) = project("vault_lake");
    let alphabet: Vec<char> = "@+1,: .*?[]!-xtag".chars().collect();
    let mut seed: u64 = 0x5eed;
    for _ in 0..20_000 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let len = (seed % 7) as usize;
        let text: String = (0..len)
            .map(|i| alphabet[((seed >> (i * 5)) as usize) % alphabet.len()])
            .collect();
        let _ = selection::select(&ctx, &u, &text, None);
        let _ = selection::select(&ctx, &u, &format!("tag:{text}"), Some(&text));
    }
    for odd in [
        "--exclude",
        "+",
        "@",
        "1+",
        "+1",
        ":",
        "a:",
        "[",
        "a,",
        ",a",
        "--exclude --exclude",
        "é:ü+",
    ] {
        let _ = selection::select(&ctx, &u, odd, None);
    }
}
