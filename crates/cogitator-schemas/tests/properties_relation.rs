//! Relation coordinates survive the properties prune: database/schema/loader
//! per source, identifier/loader per table — raw strings, `env_var()` text
//! kept verbatim (resolution happens in the analysis layer).

use std::path::Path;

use cogitator_schemas::parse_properties;

const YAML: &str = r#"
version: 2
sources:
  - name: warehouse
    database: "{{ env_var('WH_DB', 'raw') }}"
    schema: landing
    loader: fivetran
    tables:
      - name: payments
        identifier: payments_v2
        loader: stripe
      - name: orders
  - name: bare
    tables:
      - name: events
"#;

#[test]
fn source_relation_fields_survive_the_prune() {
    let parsed = parse_properties(YAML, Path::new("schema.yml")).expect("parses");
    let warehouse = &parsed.sources[0];
    assert_eq!(
        warehouse.database.as_deref(),
        Some("{{ env_var('WH_DB', 'raw') }}"),
        "env_var text stays raw at this layer"
    );
    assert_eq!(warehouse.schema.as_deref(), Some("landing"));
    assert_eq!(warehouse.loader.as_deref(), Some("fivetran"));
    let payments = &warehouse.tables[0];
    assert_eq!(payments.identifier.as_deref(), Some("payments_v2"));
    assert_eq!(payments.loader.as_deref(), Some("stripe"));
    assert_eq!(warehouse.tables[1].identifier, None);

    let bare = &parsed.sources[1];
    assert_eq!(bare.database, None);
    assert_eq!(bare.schema, None);
    assert_eq!(bare.loader, None);
    assert_eq!(bare.tables[0].identifier, None);
}
