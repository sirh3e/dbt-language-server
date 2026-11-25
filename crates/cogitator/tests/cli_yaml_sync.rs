//! `cogitator yaml-sync` on a fixture copy: `--check` fails while the
//! YAML is out of sync, a sync writes it, and the next sync is a no-op.

mod harness;
use harness::jaffle;

fn yaml_sync(root: &std::path::Path, args: &[&str]) -> (i32, String) {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_cogitator"))
        .arg("yaml-sync")
        .arg("--project-dir")
        .arg(root)
        .args(args)
        .env("DBT_PROFILES_DIR", root.join(".dbt"))
        .output()
        .expect("binary runs");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    (output.status.code().unwrap_or(-1), stdout)
}

fn yaml_files(root: &std::path::Path) -> Vec<(std::path::PathBuf, String)> {
    let mut out = Vec::new();
    let mut stack = vec![root.join("models")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "yml") {
                let text = std::fs::read_to_string(&path).unwrap();
                out.push((path, text));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn check_fails_until_a_sync_and_a_second_sync_changes_nothing() {
    let project = jaffle();
    let root = &project.root;
    let before = yaml_files(root);

    let (code, out) = yaml_sync(root, &["--check"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("+ column first_name"), "{out}");
    assert!(out.contains("Planned "), "{out}");
    assert_eq!(yaml_files(root), before, "--check wrote files");

    let (code, out) = yaml_sync(root, &[]);
    assert_eq!(code, 0, "{out}");
    let synced = yaml_files(root);
    assert_ne!(synced, before);
    assert!(root.join("models/staging/stg_stripe_payments.yml").exists());

    let (code, out) = yaml_sync(root, &["--check"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("Schema YAML is in sync"), "{out}");

    let (code, _) = yaml_sync(root, &[]);
    assert_eq!(code, 0);
    assert_eq!(yaml_files(root), synced, "a second sync changed files");
}

#[test]
fn named_models_and_json_output() {
    let project = jaffle();
    let (code, out) = yaml_sync(&project.root, &["--check", "--json", "stg_customers"]);
    assert_eq!(code, 1, "{out}");
    let plan: serde_json::Value = serde_json::from_str(&out).expect("json plan");
    assert_eq!(plan["models"].as_array().unwrap().len(), 1, "{plan}");
    assert_eq!(plan["models"][0]["model"], "stg_customers");
    assert_eq!(plan["models"][0]["added"][0], "first_name");
}

#[test]
fn a_directory_without_a_project_says_so() {
    let empty = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_cogitator"))
        .args(["yaml-sync", "--check", "--project-dir"])
        .arg(empty.path())
        .output()
        .expect("binary runs");
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("No dbt project") && stderr.contains("Solution:"),
        "{stderr}"
    );
}
