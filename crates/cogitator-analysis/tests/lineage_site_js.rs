//! The lineage site's pure JavaScript (layout, the column walk, search)
//! under `node --test`. Without Node the test says so and passes locally;
//! in CI (`CI` set) a missing Node is a failure, not a skip.

use std::path::PathBuf;
use std::process::Command;

#[test]
fn the_page_core_passes_its_node_tests() {
    let test = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src/features/lineage_site/assets/core.test.js");
    let output = match Command::new("node").arg("--test").arg(&test).output() {
        Ok(output) => output,
        Err(err) => {
            assert!(
                std::env::var_os("CI").is_none(),
                "node is needed for {}: {err}",
                test.display()
            );
            eprintln!(
                "skipped: node is not on PATH ({err}); run `node --test {}`",
                test.display()
            );
            return;
        }
    };
    assert!(
        output.status.success(),
        "node --test failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
