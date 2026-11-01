//! dbt schema types generated at compile time with `typify::import_types!`
//! (specs/rust-port/08 §2.2, macro style).
//!
//! The macro reads the **preprocessed** schemas committed under
//! `schemas/dbt/processed/` (produced by `schemas/dbt/preprocess.py` from the
//! pristine vendored dbt-jsonschema files — typify-0.8 workarounds documented
//! there). Paths are relative to this crate's `CARGO_MANIFEST_DIR`.

pub mod dbt_project {
    typify::import_types!(
        schema = "../../schemas/dbt/processed/dbt_project.json",
        unknown_crates = Allow,
    );
}

pub mod dbt_yml_files {
    typify::import_types!(
        schema = "../../schemas/dbt/processed/dbt_yml_files.json",
        unknown_crates = Allow,
    );
}

pub mod packages {
    typify::import_types!(
        schema = "../../schemas/dbt/processed/packages.json",
        unknown_crates = Allow,
    );
}

pub mod selectors {
    typify::import_types!(
        schema = "../../schemas/dbt/processed/selectors.json",
        unknown_crates = Allow,
    );
}
