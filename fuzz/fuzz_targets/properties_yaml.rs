//! Properties YAML parsing fronts every schema file in a workspace;
//! malformed YAML must degrade, never crash.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    let _ = cogitator_schemas::properties::parse_properties(
        data,
        std::path::Path::new("fuzz.yml"),
    );
});
