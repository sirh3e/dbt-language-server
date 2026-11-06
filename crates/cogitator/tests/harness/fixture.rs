//! Per-test project fixtures: every test gets its OWN jaffle_shop copy
//! and a sandboxed home, so nothing races the shared testdata/ tree and
//! nothing leaks from (or into) the developer's real ~/.dbt.

use std::path::{Path, PathBuf};
use std::time::Duration;

use super::client::{Lsp, ServerBuilder};

pub fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/jaffle_shop_duckdb")
}

/// Copy a project tree, skipping `target/` (build output + the server's
/// own persisted cache never travel into a test).
pub fn copy_project(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name() == "target" {
            continue;
        }
        let to = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_project(&entry.path(), &to);
        } else {
            std::fs::copy(entry.path(), &to).unwrap();
        }
    }
}

/// A disposable project root + sandboxed home, deleted on drop (panic
/// included, courtesy of tempfile).
pub struct TestProject {
    pub root: PathBuf,
    pub home: PathBuf,
    _tmp: tempfile::TempDir,
}

/// A fresh jaffle_shop_duckdb copy — THE way every e2e test gets its
/// project. Nothing boots on the shared fixture.
pub fn jaffle() -> TestProject {
    let tmp = tempfile::Builder::new()
        .prefix("cogitator-e2e-")
        .tempdir()
        .expect("temp dir");
    let root = tmp.path().join("project");
    copy_project(&fixture_root(), &root);
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    TestProject {
        root: root.canonicalize().unwrap(),
        home,
        _tmp: tmp,
    }
}

impl TestProject {
    /// A builder preconfigured with this project's sandbox: the server
    /// never sees the developer's HOME, XDG dirs or DBT_PROFILES_DIR.
    pub fn server(&self) -> ServerBuilder {
        let home = self.home.to_string_lossy().into_owned();
        ServerBuilder::new()
            .cwd(&self.root)
            .env("HOME", &home)
            .env("USERPROFILE", &home)
            .env("XDG_CONFIG_HOME", format!("{home}/.config"))
            .env("XDG_DATA_HOME", format!("{home}/.local/share"))
            .env("XDG_STATE_HOME", format!("{home}/.local/state"))
            .env("XDG_CACHE_HOME", format!("{home}/.cache"))
            .env(
                "DBT_PROFILES_DIR",
                self.root.join(".dbt").to_string_lossy().into_owned(),
            )
    }

    /// initialize + initialized + didOpen(customers.sql) + the refresh
    /// barrier. PANICS (with server stderr) if the first refresh never
    /// completes — no silent fallthrough.
    pub fn booted(&self) -> (Lsp, String) {
        self.booted_with(&[], serde_json::json!({}))
    }

    /// `booted` with extra env vars and explicit client capabilities
    /// (e.g. {"window": {"workDoneProgress": true}} for $/progress).
    pub fn booted_with(
        &self,
        env: &[(&str, &str)],
        capabilities: serde_json::Value,
    ) -> (Lsp, String) {
        let mut builder = self.server();
        for (k, v) in env {
            builder = builder.env(*k, *v);
        }
        let mut lsp = builder.spawn();
        let uri = boot(&mut lsp, &self.root, capabilities);
        (lsp, uri)
    }
}

/// The boot sequence against an arbitrary root; returns customers.sql's
/// URI. Shared by TestProject and the few tests that build odd roots.
pub fn boot(lsp: &mut Lsp, root: &Path, capabilities: serde_json::Value) -> String {
    let model = root.join("models/customers.sql");
    let text = std::fs::read_to_string(&model).unwrap();
    let uri = cogitator_lsp::fs_path_to_uri(&model);
    let _ = lsp.request(
        "initialize",
        serde_json::json!({
            "rootUri": cogitator_lsp::fs_path_to_uri(root),
            "capabilities": capabilities,
            "initializationOptions": {"profilesDir": root.join(".dbt").to_string_lossy()},
        }),
    );
    lsp.notify("initialized", serde_json::json!({}));
    lsp.notify(
        "textDocument/didOpen",
        serde_json::json!({"textDocument": {
            "uri": uri, "languageId": "sql", "version": 1, "text": text}}),
    );
    // the refresh barrier: refresh.count is monotonic, >= 1 means the
    // first project scan COMPLETED (hover/definition/graph all ready)
    lsp.wait_refresh(1);
    uri
}

/// Write `cogitator.yml` into the project and force a watcher pass.
pub fn write_config(project: &TestProject, lsp: &mut Lsp, yaml: &str) {
    std::fs::write(project.root.join("cogitator.yml"), yaml).unwrap();
    lsp.reload_config();
}

/// A tiny settle for the rare negative assertion ("nothing arrives") —
/// bounded and named, never a correctness wait.
pub fn settle() -> Duration {
    Duration::from_millis(250)
}
