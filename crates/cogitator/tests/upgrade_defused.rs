//! The upgrade subcommand never self-mutates: it points at this project's
//! own install script (the old installer belonged to the upstream Go
//! releases and would install the previous binary).

#[test]
fn upgrade_points_at_this_projects_installer() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_cogitator"))
        .arg("upgrade")
        .output()
        .expect("binary runs");
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("sirh3e/dbt-language-server/main/install.sh"),
        "{stderr}"
    );
    assert!(stderr.contains("solution:"), "{stderr}");
}
