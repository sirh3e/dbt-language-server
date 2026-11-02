//! The upgrade subcommand must refuse until this fork publishes releases —
//! the upstream installer would replace cogitator with the old Go binary.

#[test]
fn upgrade_is_defused_with_guidance() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_cogitator"))
        .arg("upgrade")
        .output()
        .expect("binary runs");
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not configured"), "{stderr}");
    assert!(stderr.contains("solution:"), "{stderr}");
}
