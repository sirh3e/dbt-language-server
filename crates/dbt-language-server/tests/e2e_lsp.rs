//! End-to-end test (specs/rust-port/05 §7): spawns the compiled binary and
//! speaks framed LSP over stdio against the shared jaffle fixture, checking
//! the acceptance-checklist responses (04 §5).

use std::io::{BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

struct Lsp {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: i64,
}

impl Lsp {
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_dbt-language-server"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("binary spawns");
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Lsp {
            child,
            stdin,
            stdout,
            next_id: 0,
        }
    }

    fn notify(&mut self, method: &str, params: serde_json::Value) {
        let msg = serde_json::json!({"jsonrpc": "2.0", "method": method, "params": params});
        let body = serde_json::to_vec(&msg).unwrap();
        write!(self.stdin, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
        self.stdin.write_all(&body).unwrap();
        self.stdin.flush().unwrap();
    }

    fn request(&mut self, method: &str, params: serde_json::Value) -> serde_json::Value {
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
            if value.get("id").and_then(|i| i.as_i64()) == Some(id) {
                return value;
            }
        }
    }

    fn read_message(&mut self) -> serde_json::Value {
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

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/jaffle_shop_duckdb")
}

#[test]
fn full_session() {
    let root = fixture_root().canonicalize().unwrap();
    let model = root.join("models/customers.sql");
    let text = std::fs::read_to_string(&model).unwrap();
    let uri = format!("file://{}", model.display());

    let mut lsp = Lsp::start();

    // 1. initialize (01 §3.1)
    let init = lsp.request(
        "initialize",
        serde_json::json!({
            "clientInfo": {"name": "e2e", "version": "0"},
            "rootPath": root.to_str().unwrap(),
        }),
    );
    let caps = &init["result"]["capabilities"];
    assert_eq!(caps["textDocumentSync"], 2);
    assert_eq!(caps["hoverProvider"], true);
    assert_eq!(caps["definitionProvider"], true);
    assert_eq!(
        caps["executeCommandProvider"]["commands"][0],
        "dbt.goToSchema"
    );
    assert_eq!(init["result"]["serverInfo"]["name"], "dbt-language-server");

    // 2. didOpen, then give the background refresh a moment
    lsp.notify(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {"uri": uri, "languageId": "sql", "version": 1, "text": text}
        }),
    );

    // locate a ref target in the real model
    let target_off = text
        .find("stg_customers")
        .expect("customers.sql refs stg_customers");
    let line = text[..target_off].matches('\n').count();
    let col = target_off - text[..target_off].rfind('\n').map(|p| p + 1).unwrap_or(0);

    // 3. definition on the ref — retry while the first context refresh runs
    let mut def_uri = String::new();
    for _ in 0..50 {
        let def = lsp.request(
            "textDocument/definition",
            serde_json::json!({
                "textDocument": {"uri": uri},
                "position": {"line": line, "character": col + 1}
            }),
        );
        def_uri = def["result"]["uri"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        if def_uri.ends_with("stg_customers.sql") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert!(
        def_uri.ends_with("stg_customers.sql"),
        "definition resolved: {def_uri}"
    );

    // 4. hover on the same ref returns the model description (may be empty
    //    but must be a string result with contents field)
    let hover = lsp.request(
        "textDocument/hover",
        serde_json::json!({
            "textDocument": {"uri": uri},
            "position": {"line": line, "character": col + 1}
        }),
    );
    assert!(hover["result"]["contents"].is_string());

    // 5. completion inside ref('
    lsp.notify(
        "textDocument/didChange",
        serde_json::json!({
            "textDocument": {"uri": uri, "version": 2},
            "contentChanges": [{
                "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}},
                "rangeLength": 0,
                "text": "select * from {{ ref('\n"
            }]
        }),
    );
    let completion = lsp.request(
        "textDocument/completion",
        serde_json::json!({
            "textDocument": {"uri": uri},
            "position": {"line": 0, "character": 22}
        }),
    );
    let items = completion["result"].as_array().expect("items array");
    assert!(
        items
            .iter()
            .any(|i| i["label"] == "stg_orders" && i["insertText"] == "stg_orders') }}"),
        "ref completion with mirrored suffix"
    );

    // 6. goToSchema via executeCommand on the model file itself
    let schema = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({
            "command": "dbt.goToSchema",
            "arguments": [{"uri": uri, "position": {"line": 0, "character": 0}}]
        }),
    );
    let result = &schema["result"];
    assert!(
        result.is_null() || result["uri"].as_str().unwrap_or_default().ends_with(".yml"),
        "goToSchema returns a yml location or null: {result}"
    );

    // 6b. references on the open model (graph-backed)
    let refs = lsp.request(
        "textDocument/references",
        serde_json::json!({
            "textDocument": {"uri": uri},
            "position": {"line": 0, "character": 0},
            "context": {"includeDeclaration": false}
        }),
    );
    assert!(refs["result"].is_array());

    // 6c. documentSymbol + semanticTokens round-trip
    let symbols = lsp.request(
        "textDocument/documentSymbol",
        serde_json::json!({"textDocument": {"uri": uri}, "position": {"line": 0, "character": 0}}),
    );
    assert!(symbols["result"].is_array());
    let tokens = lsp.request(
        "textDocument/semanticTokens/full",
        serde_json::json!({"textDocument": {"uri": uri}, "position": {"line": 0, "character": 0}}),
    );
    assert!(tokens["result"]["data"]
        .as_array()
        .is_some_and(|d| !d.is_empty()));

    // 6d. unknown request gets MethodNotFound; unknown notification is silent
    let unknown = lsp.request("textDocument/rename", serde_json::json!({}));
    assert_eq!(unknown["error"]["code"], -32601);

    // 7. shutdown: bare response without result (01 §3.3), then exit
    let shutdown = lsp.request("shutdown", serde_json::json!({}));
    assert!(
        shutdown.get("result").is_none(),
        "no result field: {shutdown}"
    );
    lsp.notify("exit", serde_json::json!({}));
    let status = lsp.child.wait().unwrap();
    assert_eq!(status.code(), Some(0));
}
