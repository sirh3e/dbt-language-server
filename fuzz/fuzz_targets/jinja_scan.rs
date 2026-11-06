//! The block scans run over every macro/docs/snapshot file on disk —
//! arbitrary user text, so no input may panic them.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    let _ = cogitator_parser::jinja_scan::extract_macros(data);
    let _ = cogitator_parser::jinja_scan::extract_docs_blocks(data);
    let _ = cogitator_parser::jinja_scan::extract_snapshots(data);
});
