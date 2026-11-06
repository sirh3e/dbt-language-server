//! The committed schemas/cogitator.schema.json must equal what the
//! config-schema table generates — `just gen-config-schema` after any
//! COGITATOR_SCHEMA change.

use cogitator_analysis::config_schema::json::config_json_schema;

#[test]
fn committed_json_schema_matches_the_table() {
    let committed: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../schemas/cogitator.schema.json"),
        )
        .expect("schemas/cogitator.schema.json exists"),
    )
    .expect("committed schema is valid JSON");
    let generated = serde_json::to_value(config_json_schema()).unwrap();
    assert_eq!(
        committed, generated,
        "schemas/cogitator.schema.json drifted — run `just gen-config-schema`"
    );
}

#[test]
fn every_section_and_key_is_in_the_schema() {
    let schema = serde_json::to_value(config_json_schema()).unwrap();
    for section in cogitator_analysis::config_schema::COGITATOR_SCHEMA {
        let props = &schema["properties"][section.name]["properties"];
        for key in section.keys {
            assert!(
                !props[key.key].is_null(),
                "{}.{} missing from the JSON Schema",
                section.name,
                key.key
            );
        }
    }
    // the enum hint renders as a closed enum
    let level = &schema["properties"]["log"]["properties"]["client_level"];
    assert_eq!(level["enum"].as_array().map(|a| a.len()), Some(5));
}
