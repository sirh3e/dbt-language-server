//! A controllable `dbt` on PATH, cross-platform: the `fake-dbt` bin
//! (src/bin/fake_dbt.rs, behind the `e2e` cargo feature) copied into a
//! temp dir as `dbt`/`dbt.exe`, scripted entirely through env vars.
#![cfg(feature = "e2e")]

use std::path::PathBuf;

/// What the fake dbt should do when the server runs it.
#[derive(Default)]
pub struct FakeDbt {
    pub sleep_ms: Option<u64>,
    pub exit_code: Option<i32>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
}

impl FakeDbt {
    pub fn sleeping(ms: u64) -> Self {
        FakeDbt {
            sleep_ms: Some(ms),
            ..Default::default()
        }
    }

    pub fn printing(stdout: impl Into<String>) -> Self {
        FakeDbt {
            stdout: Some(stdout.into()),
            ..Default::default()
        }
    }

    pub fn failing(code: i32, stderr: impl Into<String>) -> Self {
        FakeDbt {
            exit_code: Some(code),
            stderr: Some(stderr.into()),
            ..Default::default()
        }
    }
}

/// The installed fake: keep it alive for the test's duration; feed
/// `envs()` to the ServerBuilder.
pub struct FakeDbtOnPath {
    pub args_log: PathBuf,
    path_value: String,
    envs: Vec<(String, String)>,
    _dir: tempfile::TempDir,
}

pub fn install(behavior: FakeDbt) -> FakeDbtOnPath {
    let dir = tempfile::Builder::new()
        .prefix("cogitator-fake-dbt-")
        .tempdir()
        .expect("temp dir");
    let name = if cfg!(windows) { "dbt.exe" } else { "dbt" };
    std::fs::copy(env!("CARGO_BIN_EXE_fake-dbt"), dir.path().join(name)).expect("fake dbt copies");

    let mut paths = vec![dir.path().to_path_buf()];
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    let path_value = std::env::join_paths(paths)
        .expect("PATH joins")
        .to_string_lossy()
        .into_owned();

    let args_log = dir.path().join("argv.txt");
    let mut envs = vec![(
        "FAKE_DBT_ARGS_LOG".to_string(),
        args_log.to_string_lossy().into_owned(),
    )];
    if let Some(ms) = behavior.sleep_ms {
        envs.push(("FAKE_DBT_SLEEP_MS".into(), ms.to_string()));
    }
    if let Some(code) = behavior.exit_code {
        envs.push(("FAKE_DBT_EXIT_CODE".into(), code.to_string()));
    }
    if let Some(out) = behavior.stdout {
        envs.push(("FAKE_DBT_STDOUT".into(), out));
    }
    if let Some(err) = behavior.stderr {
        envs.push(("FAKE_DBT_STDERR".into(), err));
    }
    FakeDbtOnPath {
        args_log,
        path_value,
        envs,
        _dir: dir,
    }
}

impl FakeDbtOnPath {
    /// Env pairs for ServerBuilder::env — PATH override + the script.
    pub fn envs(&self) -> Vec<(String, String)> {
        let mut all = vec![("PATH".to_string(), self.path_value.clone())];
        all.extend(self.envs.iter().cloned());
        all
    }

    /// Every argv line the fake dbt has logged so far.
    pub fn logged_args(&self) -> Vec<String> {
        std::fs::read_to_string(&self.args_log)
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    }
}
