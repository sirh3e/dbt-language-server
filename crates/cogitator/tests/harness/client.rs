//! The deterministic LSP test client: every receive is deadline-bounded,
//! server stderr is captured and replayed on failure, and server→client
//! requests are answered automatically while staying assertable.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::wire;

/// Default per-wait deadline; `COGITATOR_E2E_DEADLINE_MS` overrides it
/// (slow CI runners raise it, failure-ergonomics checks lower it).
pub fn default_deadline() -> Duration {
    std::env::var("COGITATOR_E2E_DEADLINE_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .map(Duration::from_millis)
        .unwrap_or(Duration::from_secs(30))
}

pub struct ServerBuilder {
    bin: PathBuf,
    args: Vec<String>,
    envs: Vec<(String, String)>,
    cwd: Option<PathBuf>,
    deadline: Duration,
}

impl ServerBuilder {
    pub fn new() -> Self {
        ServerBuilder {
            bin: PathBuf::from(env!("CARGO_BIN_EXE_cogitator")),
            args: Vec::new(),
            envs: Vec::new(),
            cwd: None,
            deadline: default_deadline(),
        }
    }

    pub fn arg(mut self, a: impl Into<String>) -> Self {
        self.args.push(a.into());
        self
    }

    pub fn env(mut self, k: impl Into<String>, v: impl Into<String>) -> Self {
        self.envs.push((k.into(), v.into()));
        self
    }

    pub fn cwd(mut self, dir: &Path) -> Self {
        self.cwd = Some(dir.to_path_buf());
        self
    }

    pub fn deadline(mut self, d: Duration) -> Self {
        self.deadline = d;
        self
    }

    pub fn spawn(self) -> Lsp {
        let mut command = Command::new(&self.bin);
        command
            .args(&self.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(dir) = &self.cwd {
            command.current_dir(dir);
        }
        for (k, v) in &self.envs {
            command.env(k, v);
        }
        let mut child = command.spawn().expect("server binary spawns");
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();

        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let frame = wire::read_frame(&mut reader);
                let done = frame.is_err();
                if tx.send(frame).is_err() || done {
                    return; // test gone, or stream over
                }
            }
        });

        let captured = Arc::new(Mutex::new(Vec::new()));
        let sink = captured.clone();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stderr);
            let mut line = Vec::new();
            while matches!(reader.read_until(b'\n', &mut line), Ok(n) if n > 0) {
                sink.lock().unwrap().extend_from_slice(&line);
                line.clear();
            }
        });

        Lsp {
            child,
            stdin,
            rx,
            stderr: captured,
            buffered: VecDeque::new(),
            server_requests: Vec::new(),
            next_id: 0,
            deadline: self.deadline,
        }
    }
}

impl Default for ServerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Lsp {
    child: Child,
    stdin: ChildStdin,
    rx: Receiver<Result<serde_json::Value, String>>,
    stderr: Arc<Mutex<Vec<u8>>>,
    /// Notifications pulled while waiting for something else.
    buffered: VecDeque<serde_json::Value>,
    /// Server→client requests seen so far (already auto-answered).
    pub server_requests: Vec<serde_json::Value>,
    next_id: i64,
    deadline: Duration,
}

impl Lsp {
    pub fn notify(&mut self, method: &str, params: serde_json::Value) {
        wire::write_frame(
            &mut self.stdin,
            &serde_json::json!({"jsonrpc": "2.0", "method": method, "params": params}),
        );
    }

    /// Send a request WITHOUT waiting — concurrency/cancel tests pair it
    /// with `response_for`.
    pub fn request_nowait(&mut self, method: &str, params: serde_json::Value) -> i64 {
        self.next_id += 1;
        let id = self.next_id;
        wire::write_frame(
            &mut self.stdin,
            &serde_json::json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}),
        );
        id
    }

    pub fn request(&mut self, method: &str, params: serde_json::Value) -> serde_json::Value {
        let id = self.request_nowait(method, params);
        self.response_for(id)
    }

    /// Pull frames (buffering notifications, auto-answering server
    /// requests) until `id` answers, within the deadline.
    pub fn response_for(&mut self, id: i64) -> serde_json::Value {
        // another waiter may have buffered the reply already
        let is_reply = |frame: &serde_json::Value| {
            frame.get("id").and_then(|i| i.as_i64()) == Some(id) && frame.get("method").is_none()
        };
        if let Some(i) = self.buffered.iter().position(is_reply) {
            return self.buffered.remove(i).unwrap();
        }
        let due = Instant::now() + self.deadline;
        loop {
            let frame = self.pull(due, &format!("reply to request {id}"));
            let is_reply = frame.get("id").and_then(|i| i.as_i64()) == Some(id)
                && frame.get("method").is_none();
            if is_reply {
                return frame;
            }
            self.buffered.push_back(frame);
        }
    }

    /// The first buffered-or-future notification of `method` matching
    /// `pred`, within the deadline. Consumes the matched frame.
    pub fn wait_notification(
        &mut self,
        method: &str,
        pred: impl Fn(&serde_json::Value) -> bool,
    ) -> serde_json::Value {
        let matches = |frame: &serde_json::Value| {
            frame.get("id").is_none()
                && frame.get("method").and_then(|m| m.as_str()) == Some(method)
                && pred(frame)
        };
        if let Some(i) = self.buffered.iter().position(&matches) {
            return self.buffered.remove(i).unwrap();
        }
        let due = Instant::now() + self.deadline;
        loop {
            let frame = self.pull(due, &format!("notification {method}"));
            if matches(&frame) {
                return frame;
            }
            self.buffered.push_back(frame);
        }
    }

    /// The first seen-or-future server→client REQUEST of `method`
    /// matching `pred` (it was already auto-answered). Consumes it.
    pub fn wait_server_request(
        &mut self,
        method: &str,
        pred: impl Fn(&serde_json::Value) -> bool,
    ) -> serde_json::Value {
        let matches = |frame: &serde_json::Value| {
            frame.get("method").and_then(|m| m.as_str()) == Some(method) && pred(frame)
        };
        let due = Instant::now() + self.deadline;
        loop {
            if let Some(i) = self.server_requests.iter().position(matches) {
                return self.server_requests.remove(i);
            }
            let frame = self.pull(due, &format!("server request {method}"));
            self.buffered.push_back(frame); // pull() records server requests
        }
    }

    /// Drain everything already received, then hand back the buffered
    /// notifications of `method`. Non-blocking.
    pub fn take_notifications(&mut self, method: &str) -> Vec<serde_json::Value> {
        while let Ok(frame) = self.rx.try_recv() {
            if let Some(frame) = self.admit(frame, "drain") {
                self.buffered.push_back(frame);
            }
        }
        let mut out = Vec::new();
        self.buffered.retain(|frame| {
            let hit = frame.get("id").is_none()
                && frame.get("method").and_then(|m| m.as_str()) == Some(method);
            if hit {
                out.push(frame.clone());
            }
            !hit
        });
        out
    }

    /// Poll dbt.status until `good` holds, within the deadline.
    pub fn wait_status(&mut self, good: impl Fn(&serde_json::Value) -> bool) -> serde_json::Value {
        let due = Instant::now() + self.deadline;
        let mut last;
        loop {
            let reply = self.request(
                "workspace/executeCommand",
                serde_json::json!({"command": "dbt.status", "arguments": [{}]}),
            );
            last = reply["result"].clone();
            if good(&last) {
                return last;
            }
            if Instant::now() >= due {
                self.fail(&format!("dbt.status never converged: {last}"));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// The refresh barrier: dbt.status refresh.count is monotonic, so
    /// "count >= n" marks the n-th project refresh as COMPLETE.
    pub fn wait_refresh(&mut self, at_least: u64) -> serde_json::Value {
        self.wait_status(|s| s["refresh"]["count"].as_u64().unwrap_or(0) >= at_least)
    }

    /// dbt.reloadConfig acknowledges with null and forces a watcher pass.
    pub fn reload_config(&mut self) {
        let reply = self.request(
            "workspace/executeCommand",
            serde_json::json!({"command": "dbt.reloadConfig", "arguments": [{}]}),
        );
        assert!(reply.get("result").is_some_and(|r| r.is_null()), "{reply}");
    }

    /// shutdown → exit → reaped exit status, bounded by the deadline.
    pub fn shutdown(mut self) -> ExitStatus {
        let _ = self.request("shutdown", serde_json::Value::Null);
        self.notify("exit", serde_json::json!({}));
        let due = Instant::now() + self.deadline;
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => return status,
                Ok(None) if Instant::now() < due => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Ok(None) => self.fail("server ignored exit within the deadline"),
                Err(err) => self.fail(&format!("waiting on the server failed: {err}")),
            }
        }
    }

    /// One frame off the reader, auto-answering server→client requests.
    fn pull(&mut self, due: Instant, waiting_for: &str) -> serde_json::Value {
        loop {
            let left = due.saturating_duration_since(Instant::now());
            let frame = match self.rx.recv_timeout(left) {
                Ok(frame) => self.admit(frame, waiting_for),
                Err(RecvTimeoutError::Timeout) => {
                    self.fail(&format!("deadline waiting for {waiting_for}"))
                }
                Err(RecvTimeoutError::Disconnected) => {
                    self.fail(&format!("server stream ended waiting for {waiting_for}"))
                }
            };
            // a server→client request was answered + recorded by admit();
            // hand everything else to the caller
            match frame {
                Some(frame) => return frame,
                None => continue,
            }
        }
    }

    /// Classify one raw frame: server→client requests are answered and
    /// recorded (returns None); replies/notifications pass through.
    fn admit(
        &mut self,
        frame: Result<serde_json::Value, String>,
        waiting_for: &str,
    ) -> Option<serde_json::Value> {
        let frame = match frame {
            Ok(frame) => frame,
            Err(err) => self.fail(&format!("{err} (waiting for {waiting_for})")),
        };
        let is_server_request = frame.get("id").is_some() && frame.get("method").is_some();
        if !is_server_request {
            return Some(frame);
        }
        let id = frame["id"].clone();
        let result = match frame["method"].as_str().unwrap_or_default() {
            "window/showDocument" => serde_json::json!({"success": true}),
            "workspace/applyEdit" => serde_json::json!({"applied": true}),
            // window/workDoneProgress/create, client/registerCapability, …
            _ => serde_json::Value::Null,
        };
        wire::write_frame(
            &mut self.stdin,
            &serde_json::json!({"jsonrpc": "2.0", "id": id, "result": result}),
        );
        self.server_requests.push(frame);
        None
    }

    /// Panic with the server's stderr and the undrained frames attached.
    fn fail(&mut self, reason: &str) -> ! {
        let stderr = String::from_utf8_lossy(&self.stderr.lock().unwrap()).into_owned();
        let pending: Vec<String> = self.buffered.iter().map(|f| f.to_string()).collect();
        panic!(
            "{reason}\n--- buffered frames ({}) ---\n{}\n--- server stderr ---\n{stderr}",
            pending.len(),
            pending.join("\n"),
        );
    }
}

impl Drop for Lsp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait(); // no zombies
        if std::thread::panicking() {
            let stderr = String::from_utf8_lossy(&self.stderr.lock().unwrap()).into_owned();
            if !stderr.is_empty() {
                eprintln!("--- server stderr (replayed on panic) ---\n{stderr}");
            }
        }
    }
}

impl Lsp {
    /// Raw channel receive for tests that need frame-at-a-time control.
    pub fn try_next_frame(&mut self) -> Option<serde_json::Value> {
        match self.rx.try_recv() {
            Ok(frame) => self.admit(frame, "try_next_frame"),
            Err(_) => None,
        }
    }
}
