//! Shared e2e harness: a framed-LSP client over the compiled binary,
//! fixture helpers and the polling utilities every themed suite uses.
#![allow(dead_code)] // each test crate consumes a subset

use std::io::{BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

pub struct Lsp {
    pub child: Child,
    pub stdin: ChildStdin,
    pub stdout: BufReader<ChildStdout>,
    pub next_id: i64,
    /// server→client requests/notifications seen while waiting for replies
    pub stash: Vec<serde_json::Value>,
}

impl Lsp {
    pub fn start() -> Self {
        Self::start_with_env(&[])
    }

    pub fn start_with_env(env: &[(&str, &str)]) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cogitator"));
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        for (key, value) in env {
            command.env(key, value);
        }
        let mut child = command.spawn().expect("binary spawns");
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Lsp {
            child,
            stdin,
            stdout,
            next_id: 0,
            stash: Vec::new(),
        }
    }

    pub fn notify(&mut self, method: &str, params: serde_json::Value) {
        let msg = serde_json::json!({"jsonrpc": "2.0", "method": method, "params": params});
        let body = serde_json::to_vec(&msg).unwrap();
        write!(self.stdin, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
        self.stdin.write_all(&body).unwrap();
        self.stdin.flush().unwrap();
    }

    /// Send a request WITHOUT waiting — concurrency/cancel tests pair it
    /// with `response_for`.
    pub fn request_nowait(&mut self, method: &str, params: serde_json::Value) -> i64 {
        self.next_id += 1;
        let id = self.next_id;
        let msg =
            serde_json::json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        let body = serde_json::to_vec(&msg).unwrap();
        write!(self.stdin, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
        self.stdin.write_all(&body).unwrap();
        self.stdin.flush().unwrap();
        id
    }

    /// Read frames (stashing everything else) until `id` answers.
    pub fn response_for(&mut self, id: i64) -> serde_json::Value {
        loop {
            let value = self.read_message();
            let is_reply = value.get("id").and_then(|i| i.as_i64()) == Some(id)
                && value.get("method").is_none();
            if is_reply {
                return value;
            }
            self.stash.push(value);
        }
    }

    pub fn request(&mut self, method: &str, params: serde_json::Value) -> serde_json::Value {
        self.next_id += 1;
        let id = self.next_id;
        let msg =
            serde_json::json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        let body = serde_json::to_vec(&msg).unwrap();
        write!(self.stdin, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
        self.stdin.write_all(&body).unwrap();
        self.stdin.flush().unwrap();
        loop {
            let value = self.read_message();
            let is_reply = value.get("id").and_then(|i| i.as_i64()) == Some(id)
                && value.get("method").is_none();
            if is_reply {
                return value;
            }
            self.stash.push(value);
        }
    }

    pub fn read_message(&mut self) -> serde_json::Value {
        let mut header = Vec::new();
        let mut last4 = [0u8; 4];
        loop {
            let mut byte = [0u8; 1];
            self.stdout
                .read_exact(&mut byte)
                .expect("server closed stdout");
            header.push(byte[0]);
            last4.rotate_left(1);
            last4[3] = byte[0];
            if &last4 == b"\r\n\r\n" {
                break;
            }
        }
        let header_text = String::from_utf8_lossy(&header);
        // stdout-cleanliness guard: nothing but LSP frames may appear on
        // stdout (05 §6.3), so the very first header line must be a
        // Content-Length — stray print output fails loudly here
        assert!(
            header_text.starts_with("Content-Length:"),
            "non-LSP bytes on stdout: {header_text:?}"
        );
        let len: usize = header_text
            .lines()
            .find_map(|l| l.strip_prefix("Content-Length:"))
            .and_then(|v| v.trim().parse().ok())
            .expect("content length");
        let mut body = vec![0u8; len];
        self.stdout.read_exact(&mut body).unwrap();
        serde_json::from_slice(&body).unwrap()
    }
}

impl Drop for Lsp {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

pub fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/jaffle_shop_duckdb")
}

/// Removes a path on drop — fixtures stay pristine even on panic.
pub struct RemoveOnDrop(pub PathBuf);
impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

/// Copy the fixture (minus target/) so a test can mutate its own root
/// without racing the parallel tests on the shared fixture.
pub fn copy_fixture(src: &std::path::Path, dst: &std::path::Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name() == "target" {
            continue;
        }
        let to = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_fixture(&entry.path(), &to);
        } else {
            std::fs::copy(entry.path(), &to).unwrap();
        }
    }
}

/// Shared setup for the concurrency tests: initialize + open customers
/// and wait for the first refresh (definition answers correctly).
pub fn booted(env: &[(&str, &str)]) -> (Lsp, String) {
    booted_in(&fixture_root().canonicalize().unwrap(), env)
}

/// `booted` against an arbitrary project root (temp fixture copies).
pub fn booted_in(root: &std::path::Path, env: &[(&str, &str)]) -> (Lsp, String) {
    let mut lsp = Lsp::start_with_env(env);
    let model = root.join("models/customers.sql");
    let text = std::fs::read_to_string(&model).unwrap();
    let uri = cogitator_lsp::fs_path_to_uri(&model);
    let _ = lsp.request(
        "initialize",
        serde_json::json!({
            "rootUri": cogitator_lsp::fs_path_to_uri(root),
            "capabilities": {},
            "initializationOptions": {"profilesDir": root.join(".dbt").to_string_lossy()},
        }),
    );
    lsp.notify("initialized", serde_json::json!({}));
    lsp.notify(
        "textDocument/didOpen",
        serde_json::json!({"textDocument": {
            "uri": uri, "languageId": "sql", "version": 1, "text": text}}),
    );
    // wait for the refresh: hover over a ref answers non-empty eventually
    for _ in 0..100 {
        let hover = lsp.request(
            "textDocument/hover",
            serde_json::json!({"textDocument": {"uri": uri},
                               "position": {"line": 2, "character": 24}}),
        );
        if hover["result"]["contents"]
            .as_str()
            .is_some_and(|c| !c.is_empty())
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    (lsp, uri)
}

/// Poll dbt.status until `good` holds (the watcher applies asynchronously).
pub fn status_until(lsp: &mut Lsp, good: impl Fn(&serde_json::Value) -> bool) -> serde_json::Value {
    let mut last = serde_json::Value::Null;
    for _ in 0..100 {
        let reply = lsp.request(
            "workspace/executeCommand",
            serde_json::json!({"command": "dbt.status", "arguments": [{}]}),
        );
        last = reply["result"].clone();
        if good(&last) {
            return last;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("dbt.status never converged: {last}");
}

/// dbt.reloadConfig acknowledges with null and forces a watcher pass.
pub fn reload_config(lsp: &mut Lsp) {
    let reply = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({"command": "dbt.reloadConfig", "arguments": [{}]}),
    );
    assert!(reply.get("result").is_some_and(|r| r.is_null()), "{reply}");
}
