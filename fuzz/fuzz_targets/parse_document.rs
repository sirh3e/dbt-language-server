//! The phase-1 pipeline must never panic, whatever bytes an editor
//! sends: it runs on every keystroke.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    use cogitator_parser::DocumentParser;
    let dialect = cogitator_dialects::Dialect::new("duckdb");
    let _ = cogitator_parser::JinjaSqlParser.parse(data, &dialect);
});
