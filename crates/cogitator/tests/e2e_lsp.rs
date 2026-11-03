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
    /// server→client requests/notifications seen while waiting for replies
    stash: Vec<serde_json::Value>,
}

impl Lsp {
    fn start() -> Self {
        Self::start_with_env(&[])
    }

    fn start_with_env(env: &[(&str, &str)]) -> Self {
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

    fn notify(&mut self, method: &str, params: serde_json::Value) {
        let msg = serde_json::json!({"jsonrpc": "2.0", "method": method, "params": params});
        let body = serde_json::to_vec(&msg).unwrap();
        write!(self.stdin, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
        self.stdin.write_all(&body).unwrap();
        self.stdin.flush().unwrap();
    }

    /// Send a request WITHOUT waiting — concurrency/cancel tests pair it
    /// with `response_for`.
    fn request_nowait(&mut self, method: &str, params: serde_json::Value) -> i64 {
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
    fn response_for(&mut self, id: i64) -> serde_json::Value {
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
            let is_reply = value.get("id").and_then(|i| i.as_i64()) == Some(id)
                && value.get("method").is_none();
            if is_reply {
                return value;
            }
            self.stash.push(value);
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

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/jaffle_shop_duckdb")
}

/// Removes a path on drop — fixtures stay pristine even on panic.
struct RemoveOnDrop(PathBuf);
impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

/// Copy the fixture (minus target/) so a test can mutate its own root
/// without racing the parallel tests on the shared fixture.
fn copy_fixture(src: &std::path::Path, dst: &std::path::Path) {
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

#[test]
fn full_session() {
    let root = fixture_root().canonicalize().unwrap();
    let model = root.join("models/customers.sql");
    let text = std::fs::read_to_string(&model).unwrap();
    let uri = cogitator_lsp::fs_path_to_uri(&model);

    let mut lsp = Lsp::start();

    // 1. initialize (01 §3.1)
    let init = lsp.request(
        "initialize",
        serde_json::json!({
            "clientInfo": {"name": "e2e", "version": "0"},
            // modern-client shape: rootUri only, no deprecated rootPath
            "rootUri": cogitator_lsp::fs_path_to_uri(&root),
            "capabilities": {"window": {"workDoneProgress": true}},
            // explicit profiles dir (here: the fixture's own .dbt folder)
            "initializationOptions": {"profilesDir": root.join(".dbt").to_string_lossy()},
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
    assert!(caps["completionProvider"]["triggerCharacters"]
        .as_array()
        .is_some_and(|chars| chars.iter().any(|c| c == "'")));
    assert_eq!(caps["codeActionProvider"], true);
    assert_eq!(init["result"]["serverInfo"]["name"], "cogitator");

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

    // 4b. column lineage command on the open model
    let lineage = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({
            "command": "dbt.columnLineage",
            "arguments": [{"uri": uri}]
        }),
    );
    assert!(
        lineage["result"]["columns"]
            .as_array()
            .is_some_and(|c| !c.is_empty()),
        "column lineage over the wire: {}",
        lineage["result"]
    );

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

    // 6a2. dbt YAML completion is wired through the dispatcher
    lsp.notify(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {"uri": "file:///tmp/schema.yml", "languageId": "yaml",
                              "version": 1, "text": "version: 2\nmodels:\n  - name: a\n    "}
        }),
    );
    let yml = lsp.request(
        "textDocument/completion",
        serde_json::json!({
            "textDocument": {"uri": "file:///tmp/schema.yml"},
            "position": {"line": 3, "character": 4}
        }),
    );
    assert!(
        yml["result"]
            .as_array()
            .is_some_and(|items| items.iter().any(|i| i["label"] == "description")),
        "yaml schema-key completion wired: {}",
        yml["result"]
    );

    // 6a3. engine: member access through a FROM alias resolves YAML columns
    lsp.notify(
        "textDocument/didOpen",
        serde_json::json!({
            "textDocument": {"uri": "file:///tmp/engine.sql", "languageId": "sql", "version": 1,
                              "text": "select c. from {{ ref('customers') }} c"}
        }),
    );
    let member = lsp.request(
        "textDocument/completion",
        serde_json::json!({
            "textDocument": {"uri": "file:///tmp/engine.sql"},
            "position": {"line": 0, "character": 9}
        }),
    );
    assert!(
        member["result"].as_array().is_some_and(|items| items
            .iter()
            .any(|i| i["label"] == "customer_id" && i["detail"] == "Column of customers")),
        "alias member access over the wire: {}",
        member["result"]
    );

    // 6a4. lens batch + openSchema navigation via window/showDocument
    let lenses = lsp.request(
        "textDocument/codeLens",
        serde_json::json!({"textDocument": {"uri": uri}, "position": {"line": 0, "character": 0}}),
    );
    let lens_titles: Vec<String> = lenses["result"]
        .as_array()
        .expect("lenses")
        .iter()
        .map(|l| {
            l["command"]["title"]
                .as_str()
                .unwrap_or_default()
                .to_string()
        })
        .collect();
    assert!(
        lens_titles.iter().any(|t| t.contains("downstream")),
        "{lens_titles:?}"
    );
    assert!(
        lens_titles.contains(&"dbt compile".to_string()),
        "{lens_titles:?}"
    );

    // 6a4b. code actions: the executable lens set at the cursor, as
    // command-only actions
    let actions = lsp.request(
        "textDocument/codeAction",
        serde_json::json!({"textDocument": {"uri": uri},
                           "range": {"start": {"line": 0, "character": 0},
                                     "end": {"line": 0, "character": 0}},
                           "context": {"diagnostics": []}}),
    );
    let action_commands: Vec<String> = actions["result"]
        .as_array()
        .expect("actions")
        .iter()
        .map(|a| {
            a["command"]["command"]
                .as_str()
                .unwrap_or_default()
                .to_string()
        })
        .collect();
    assert!(
        action_commands.contains(&"dbt.run".to_string()),
        "{action_commands:?}"
    );
    assert!(
        action_commands.contains(&"dbt.goToTests".to_string()),
        "{action_commands:?}"
    );

    // 6a4b². in the active profiles.yml, code actions switch targets —
    // the picker-less path that even Zed can use
    let profiles_uri = cogitator_lsp::fs_path_to_uri(&root.join(".dbt/profiles.yml"));
    let profile_actions = lsp.request(
        "textDocument/codeAction",
        serde_json::json!({"textDocument": {"uri": profiles_uri},
                           "range": {"start": {"line": 0, "character": 0},
                                     "end": {"line": 0, "character": 0}},
                           "context": {"diagnostics": []}}),
    );
    let switch_titles: Vec<String> = profile_actions["result"]
        .as_array()
        .expect("profiles actions")
        .iter()
        .map(|a| a["title"].as_str().unwrap_or_default().to_string())
        .collect();
    assert!(
        switch_titles.contains(&"Switch target: jaffle_shop/dev (active)".to_string()),
        "{switch_titles:?}"
    );

    let _ = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({"command": "dbt.openSchema", "arguments": [{"uri": uri}]}),
    );
    let show_doc = lsp
        .stash
        .iter()
        .find(|m| m["method"] == "window/showDocument")
        .cloned()
        .expect("server sent window/showDocument");
    assert!(show_doc["params"]["uri"]
        .as_str()
        .unwrap()
        .ends_with(".yml"));
    // answer the server's request: the response guard must swallow it
    let reply = serde_json::json!({
        "jsonrpc": "2.0", "id": show_doc["id"], "result": {"success": true}
    });
    let body = serde_json::to_vec(&reply).unwrap();
    write!(lsp.stdin, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
    lsp.stdin.write_all(&body).unwrap();
    lsp.stdin.flush().unwrap();

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

    // 6d. unknown request gets MethodNotFound; a known request with bad
    // params gets InvalidParams
    // 6a5. the context refresh reported itself through $/progress
    //      (asserted before the later sections clear the stash)
    assert!(
        lsp.stash
            .iter()
            .any(|m| m["method"] == "window/workDoneProgress/create"),
        "progress token created: {:?}",
        lsp.stash
    );
    assert!(
        lsp.stash.iter().any(|m| m["method"] == "$/progress"
            && m["params"]["value"]["kind"] == "end"
            && m["params"]["value"]["message"]
                .as_str()
                .is_some_and(|s| s.contains("models"))),
        "refresh reported via $/progress"
    );

    // 6a5b. dbt.openModel navigates via window/showDocument
    lsp.stash.clear();
    let _ = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({"command": "dbt.openModel", "arguments": [{"model": "customers"}]}),
    );
    assert!(
        lsp.stash
            .iter()
            .any(|m| m["method"] == "window/showDocument"
                && m["params"]["uri"]
                    .as_str()
                    .is_some_and(|u| u.ends_with("customers.sql"))),
        "dbt.openModel sent showDocument: {:?}",
        lsp.stash
    );

    // 6a6. yaml workflow: tests navigation + extract-to-version
    let tests = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({ "command": "dbt.modelTests", "arguments": [{ "uri": uri }] }),
    );
    assert_eq!(tests["result"]["model"], "customers", "{tests}");
    let list = tests["result"]["tests"].as_array().expect("tests array");
    assert!(
        list.iter()
            .any(|t| t["name"] == "unique" && t["column"] == "customer_id"),
        "{tests}"
    );

    lsp.stash.clear();
    let goto = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({ "command": "dbt.goToTests", "arguments": [{ "uri": uri }] }),
    );
    assert!(goto["result"].is_null());
    assert!(
        lsp.stash
            .iter()
            .any(|m| m["method"] == "window/showDocument"
                && m["params"]["uri"].as_str().unwrap().ends_with("schema.yml")),
        "goToTests must navigate: {:?}",
        lsp.stash
    );

    lsp.stash.clear();
    let extract = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({ "command": "dbt.extractModelYaml", "arguments": [{ "uri": uri }] }),
    );
    assert!(extract["result"].is_null());
    let apply = lsp
        .stash
        .iter()
        .find(|m| m["method"] == "workspace/applyEdit")
        .expect("extract must send workspace/applyEdit");
    let new_text = apply["params"]["edit"]["documentChanges"][0]["edits"][0]["newText"]
        .as_str()
        .unwrap_or_default();
    assert!(new_text.contains("versions:"), "{apply}");

    // 6b. the profiles auth manager over the wire
    let profiles = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({ "command": "dbt.profiles", "arguments": [{}] }),
    );
    let report = &profiles["result"];
    assert_eq!(report["activeProfile"], "jaffle_shop", "{report}");
    assert_eq!(report["activeTarget"], "dev", "{report}");
    let jaffle = &report["profiles"][0];
    assert_eq!(jaffle["name"], "jaffle_shop");
    assert_eq!(jaffle["allowed"], true);
    assert_eq!(jaffle["targets"][0]["type"], "duckdb");
    assert!(jaffle["targets"][0]["auth"]["method"].is_string());

    let switched = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({
            "command": "dbt.setTarget",
            "arguments": [{ "target": "dev" }]
        }),
    );
    assert_eq!(switched["result"]["target"], "dev", "{switched}");
    let bogus = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({
            "command": "dbt.setTarget",
            "arguments": [{ "target": "no_such_target" }]
        }),
    );
    assert!(bogus["result"].is_null(), "{bogus}");

    // 6b2. tag/selector search + the project DAG
    let tagged = lsp.request(
        "workspace/symbol",
        serde_json::json!({ "query": "tag:nightly" }),
    );
    assert_eq!(tagged["result"][0]["name"], "customers", "{tagged}");
    let selectors = lsp.request(
        "workspace/symbol",
        serde_json::json!({ "query": "selector:" }),
    );
    assert!(
        selectors["result"]
            .as_array()
            .is_some_and(|s| s.iter().any(|e| e["name"] == "nightly")),
        "{selectors}"
    );
    let dag = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({ "command": "dbt.projectGraph", "arguments": [{}] }),
    );
    assert!(
        dag["result"]["nodes"]
            .as_array()
            .is_some_and(|n| n.len() > 5),
        "{dag}"
    );
    assert!(dag["result"]["edges"]
        .as_array()
        .is_some_and(|e| !e.is_empty()));

    // 6c. modelInfo: neighbors + tests + versions + compiled SQL in one trip.
    // dbt would write target/compiled/ at runtime; fake it (gitignored) and
    // clean up even on assertion failure.
    let compiled_dir = root.join("target/compiled/jaffle_shop/models");
    std::fs::create_dir_all(&compiled_dir).unwrap();
    std::fs::write(compiled_dir.join("customers.sql"), "select 1 as id\n").unwrap();
    let _compiled_cleanup = RemoveOnDrop(root.join("target"));

    let info = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({ "command": "dbt.modelInfo",
                            "arguments": [{ "model": "customers" }] }),
    );
    let i = &info["result"];
    assert!(
        i["upstream"]
            .as_array()
            .is_some_and(|u| u.iter().any(|e| e["name"] == "stg_customers")),
        "{i}"
    );
    assert!(i["tests"].as_array().is_some_and(|t| !t.is_empty()), "{i}");
    assert!(
        i["compiledUri"]
            .as_str()
            .is_some_and(|u| u.ends_with("target/compiled/jaffle_shop/models/customers.sql")),
        "{i}"
    );
    let versioned = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({ "command": "dbt.modelInfo",
                            "arguments": [{ "model": "customer_orders" }] }),
    );
    assert_eq!(
        versioned["result"]["versions"]["latestVersion"], 2,
        "{versioned}"
    );

    // 6c¹. compiledSql: content over the wire while the fake file exists;
    // a model dbt never compiled answers nulls
    let compiled = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({ "command": "dbt.compiledSql",
                            "arguments": [{ "model": "customers" }] }),
    );
    let c = &compiled["result"];
    assert_eq!(c["model"], "customers", "{c}");
    assert_eq!(c["content"], "select 1 as id\n", "{c}");
    assert!(
        c["uri"]
            .as_str()
            .is_some_and(|u| u.ends_with("target/compiled/jaffle_shop/models/customers.sql")),
        "{c}"
    );
    let uncompiled = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({ "command": "dbt.compiledSql",
                            "arguments": [{ "model": "orders" }] }),
    );
    let u = &uncompiled["result"];
    assert_eq!(u["uri"], serde_json::Value::Null, "{u}");
    assert_eq!(u["content"], serde_json::Value::Null, "{u}");

    // 6c². relation: the fixture's duckdb target (path jaffle_shop.duckdb,
    // schema default main) frames the model's relation
    let relation = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({ "command": "dbt.relation",
                            "arguments": [{ "uri": uri }] }),
    );
    let r = &relation["result"];
    assert_eq!(r["kind"], "model", "{r}");
    assert_eq!(r["relation"], "jaffle_shop.main.customers", "{r}");
    assert_eq!(r["source"], serde_json::Value::Null, "{r}");

    // 6d. status: dbt.status knows the project
    let status = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({ "command": "dbt.status", "arguments": [{}] }),
    );
    let s = &status["result"];
    assert_eq!(s["server"]["backend"], "tree-sitter", "{s}");
    assert_eq!(s["project"]["dialect"], "duckdb", "{s}");
    assert!(s["project"]["models"].as_u64().unwrap() > 0, "{s}");
    assert!(s["refresh"]["count"].as_u64().unwrap() >= 1, "{s}");

    // 6d². derived caches: status carries the counters, dbt.cache reports
    // and clears them (dbt.profiles above populated the profiles surface)
    assert!(s["caches"]["profiles"]["entries"].is_u64(), "{s}");
    let cache_stats = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({ "command": "dbt.cache", "arguments": [{ "action": "stats" }] }),
    );
    assert!(
        cache_stats["result"]["profiles"]["entries"]
            .as_u64()
            .is_some_and(|n| n >= 1),
        "{cache_stats}"
    );
    let cleared = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({ "command": "dbt.cache", "arguments": [{ "action": "clear" }] }),
    );
    assert_eq!(cleared["result"]["profiles"]["entries"], 0, "{cleared}");
    assert_eq!(cleared["result"]["lineage"]["entries"], 0, "{cleared}");

    let unknown = lsp.request("textDocument/typeDefinition", serde_json::json!({}));
    assert_eq!(unknown["error"]["code"], -32601);
    let bad = lsp.request("textDocument/rename", serde_json::json!({}));
    assert_eq!(bad["error"]["code"], -32602);

    // 6e. dbt.setProfilesDir: live switch to another profiles directory —
    //     the report follows without a client restart, and the stale
    //     target override resets (last profiles assertions of the session)
    let other_dir =
        std::env::temp_dir().join(format!("cogitator-e2e-profiles-{}", std::process::id()));
    std::fs::create_dir_all(&other_dir).unwrap();
    std::fs::write(
        other_dir.join("profiles.yml"),
        "jaffle_shop:\n  target: dev\n  outputs:\n    dev:\n      type: snowflake\n",
    )
    .unwrap();
    let _profiles_cleanup = RemoveOnDrop(other_dir.clone());
    let switched_dir = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({ "command": "dbt.setProfilesDir",
                            "arguments": [{ "dir": other_dir.to_string_lossy() }] }),
    );
    assert_eq!(
        switched_dir["result"]["dir"],
        other_dir.to_string_lossy().as_ref(),
        "{switched_dir}"
    );
    let report = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({ "command": "dbt.profiles", "arguments": [{}] }),
    );
    let r = &report["result"];
    assert!(
        r["path"]
            .as_str()
            .is_some_and(|p| p.starts_with(other_dir.to_string_lossy().as_ref())),
        "{r}"
    );
    assert_eq!(r["profiles"][0]["targets"][0]["type"], "snowflake", "{r}");
    let empty_dir = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({ "command": "dbt.setProfilesDir", "arguments": [{}] }),
    );
    assert!(empty_dir["result"].is_null(), "{empty_dir}");

    // 7. shutdown must carry "result": null (deliberate deviation from the
    //    Go bare shape — Neovim rejects it as INVALID_SERVER_MESSAGE)
    let shutdown = lsp.request("shutdown", serde_json::json!({}));
    assert!(
        shutdown.get("result").is_some_and(|r| r.is_null()),
        "result: null expected: {shutdown}"
    );
    lsp.notify("exit", serde_json::json!({}));
    let status = lsp.child.wait().unwrap();
    assert_eq!(status.code(), Some(0));
}

/// Shared setup for the concurrency tests: initialize + open customers
/// and wait for the first refresh (definition answers correctly).
fn booted(env: &[(&str, &str)]) -> (Lsp, String) {
    booted_in(&fixture_root().canonicalize().unwrap(), env)
}

/// `booted` against an arbitrary project root (temp fixture copies).
fn booted_in(root: &std::path::Path, env: &[(&str, &str)]) -> (Lsp, String) {
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

/// The concurrency proof: a delayed dbt.modelInfo must NOT block the
/// hover sent right after it — three orders of magnitude of margin
/// (1500 ms floor vs a sub-ms handler), no polling.
#[test]
fn slow_request_does_not_block_fast_ones() {
    let (mut lsp, uri) = booted(&[("COGITATOR_E2E_DELAY", "dbt.modelInfo=1500")]);
    let slow = lsp.request_nowait(
        "workspace/executeCommand",
        serde_json::json!({"command": "dbt.modelInfo",
                           "arguments": [{"model": "customers", "uri": ""}]}),
    );
    let fast = lsp.request_nowait(
        "textDocument/hover",
        serde_json::json!({"textDocument": {"uri": uri},
                           "position": {"line": 0, "character": 0}}),
    );
    // the hover must answer FIRST even though it was sent second
    let first = lsp.response_for(fast);
    assert!(first.get("result").is_some(), "{first}");
    let slow_reply = lsp.response_for(slow);
    assert_eq!(slow_reply["result"]["model"], "customers", "{slow_reply}");
    let _ = lsp.request("shutdown", serde_json::json!({}));
    lsp.notify("exit", serde_json::json!({}));
}

/// Real cancellation: the delayed request answers RequestCancelled
/// (-32800) and the server keeps serving.
#[test]
fn cancel_request_aborts_in_flight_work() {
    let (mut lsp, uri) = booted(&[("COGITATOR_E2E_DELAY", "dbt.modelInfo=1500")]);
    let slow = lsp.request_nowait(
        "workspace/executeCommand",
        serde_json::json!({"command": "dbt.modelInfo",
                           "arguments": [{"model": "customers", "uri": ""}]}),
    );
    lsp.notify("$/cancelRequest", serde_json::json!({"id": slow}));
    let cancelled = lsp.response_for(slow);
    assert_eq!(cancelled["error"]["code"], -32800, "{cancelled}");
    // and the server still answers afterwards
    let hover = lsp.request(
        "textDocument/hover",
        serde_json::json!({"textDocument": {"uri": uri},
                           "position": {"line": 0, "character": 0}}),
    );
    assert!(hover.get("result").is_some(), "{hover}");
    let _ = lsp.request("shutdown", serde_json::json!({}));
    lsp.notify("exit", serde_json::json!({}));
}

/// Silent drops are gone: a command with a non-object argument answers
/// InvalidParams (this request used to hang the client forever).
#[test]
fn malformed_command_argument_answers_invalid_params() {
    let (mut lsp, _uri) = booted(&[]);
    let bad = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({"command": "dbt.modelInfo", "arguments": [42]}),
    );
    assert_eq!(bad["error"]["code"], -32602, "{bad}");
    assert!(
        bad["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("argument OBJECT")),
        "{bad}"
    );
    let _ = lsp.request("shutdown", serde_json::json!({}));
    lsp.notify("exit", serde_json::json!({}));
}

/// Poll dbt.status until `good` holds (the watcher applies asynchronously).
fn status_until(lsp: &mut Lsp, good: impl Fn(&serde_json::Value) -> bool) -> serde_json::Value {
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
fn reload_config(lsp: &mut Lsp) {
    let reply = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({"command": "dbt.reloadConfig", "arguments": [{}]}),
    );
    assert!(reply.get("result").is_some_and(|r| r.is_null()), "{reply}");
}

/// The hot-loadable cogitator.yml end to end: applied at boot, re-tuned
/// live (the compiledSql wire cap), a parse failure keeps the old values
/// and surfaces as Guidance, and deleting the file reverts to launch
/// values — all driven through dbt.reloadConfig, never the 2 s poll.
#[test]
fn config_file_hot_reload() {
    let src = fixture_root().canonicalize().unwrap();
    let root = std::env::temp_dir().join(format!("cogitator-e2e-config-{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    copy_fixture(&src, &root);
    let _cleanup = RemoveOnDrop(root.clone());
    let config = root.join("cogitator.yml");
    std::fs::write(&config, "profiles:\n  target: dev\n").unwrap();

    let (mut lsp, _uri) = booted_in(&root, &[]);

    // 1. the boot pass loaded the file and applied profiles.target
    let status = status_until(&mut lsp, |s| {
        s["config"]["loaded"] == true && s["profiles"]["activeTarget"] == "dev"
    });
    assert!(
        status["config"]["path"]
            .as_str()
            .is_some_and(|p| p.ends_with("cogitator.yml")),
        "{status}"
    );
    assert!(status["config"]["error"].is_null(), "{status}");

    // 2. hot re-tune: a 0 MiB wire cap flips compiledSql to uri-only
    let compiled_dir = root.join("target/compiled/jaffle_shop/models");
    std::fs::create_dir_all(&compiled_dir).unwrap();
    std::fs::write(compiled_dir.join("customers.sql"), "select 1 as id\n").unwrap();
    std::fs::write(
        &config,
        "profiles:\n  target: dev\nlimits:\n  compiled_sql_mib: 0\n",
    )
    .unwrap();
    reload_config(&mut lsp);
    let mut capped = serde_json::Value::Null;
    for _ in 0..100 {
        let reply = lsp.request(
            "workspace/executeCommand",
            serde_json::json!({"command": "dbt.compiledSql",
                               "arguments": [{"model": "customers"}]}),
        );
        if reply["result"]["content"].is_null() && reply["result"]["uri"].is_string() {
            capped = reply;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(
        capped["result"]["uri"]
            .as_str()
            .is_some_and(|u| u.ends_with("customers.sql")),
        "cap never applied: {capped}"
    );
    // the cap explanation reached the client as a showMessage
    assert!(
        lsp.stash.iter().any(|m| {
            m["method"] == "window/showMessage"
                && m["params"]["message"]
                    .as_str()
                    .is_some_and(|t| t.contains("wire cap"))
        }),
        "{:?}",
        lsp.stash
    );

    // 3. a file that stops parsing: error surfaces, old values stay
    std::fs::write(&config, "profiles:\n  dir: [unclosed\n").unwrap();
    reload_config(&mut lsp);
    let broken = status_until(&mut lsp, |s| s["config"]["error"].is_string());
    assert_eq!(broken["profiles"]["activeTarget"], "dev", "{broken}");

    // 4. deleting the file reverts its keys to the launch values
    std::fs::remove_file(&config).unwrap();
    reload_config(&mut lsp);
    let reverted = status_until(&mut lsp, |s| s["config"]["loaded"] == false);
    assert!(reverted["config"]["path"].is_null(), "{reverted}");
    assert_eq!(reverted["profiles"]["activeTarget"], "", "{reverted}");

    let _ = lsp.request("shutdown", serde_json::json!({}));
    lsp.notify("exit", serde_json::json!({}));
}

/// Did-you-mean quickfixes end to end (01 §6.1-6.2): the dangling-ref
/// diagnostic carries typed data; echoed back through the codeAction
/// context it yields an edit-bearing quickfix, and `only: ["quickfix"]`
/// now filters instead of blanking the response.
#[test]
fn dangling_ref_quickfix_carries_an_edit() {
    let (mut lsp, _uri) = booted(&[]);
    let root = fixture_root().canonicalize().unwrap();
    let bad_uri = cogitator_lsp::fs_path_to_uri(&root.join("models/quickfix_probe.sql"));
    lsp.notify(
        "textDocument/didOpen",
        serde_json::json!({"textDocument": {"uri": bad_uri, "languageId": "sql", "version": 1,
            "text": "select * from {{ ref('customer') }}\n"}}),
    );
    let pulled = lsp.request(
        "textDocument/diagnostic",
        serde_json::json!({"textDocument": {"uri": bad_uri}}),
    );
    let items = pulled["result"]["items"].as_array().expect("items").clone();
    let diag = items
        .iter()
        .find(|d| d["code"] == "dangling-ref")
        .expect("dangling-ref diagnostic")
        .clone();
    assert_eq!(diag["data"]["fix"], "replace_token", "{diag}");
    assert_eq!(diag["data"]["suggestions"][0], "customers", "{diag}");

    let actions = lsp.request(
        "textDocument/codeAction",
        serde_json::json!({
            "textDocument": {"uri": bad_uri},
            "range": diag["range"],
            "context": {"diagnostics": [diag], "only": ["quickfix"]}
        }),
    );
    let list = actions["result"].as_array().expect("actions");
    assert!(!list.is_empty(), "{actions}");
    assert!(list.iter().all(|a| a["kind"] == "quickfix"), "{actions}");
    let first = &list[0];
    assert_eq!(first["title"], "Change to 'customers'", "{first}");
    assert_eq!(
        first["edit"]["documentChanges"][0]["edits"][0]["newText"], "customers",
        "{first}"
    );

    let _ = lsp.request("shutdown", serde_json::json!({}));
    lsp.notify("exit", serde_json::json!({}));
}

/// cogitator.yml intelligence (03 §10): an open config file gets its
/// OWN diagnostics pass — unknown-key warnings from the shared schema,
/// never the dbt-properties pass that used to flag every root key.
#[test]
fn cogitator_yml_gets_config_intelligence() {
    let src = fixture_root().canonicalize().unwrap();
    let root = std::env::temp_dir().join(format!("cogitator-e2e-yamlcfg-{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    copy_fixture(&src, &root);
    let _cleanup = RemoveOnDrop(root.clone());
    let config = root.join("cogitator.yml");
    let text = "lint:\n  select_star: true\n  bogus: 1\n";
    std::fs::write(&config, text).unwrap();

    let (mut lsp, _uri) = booted_in(&root, &[]);
    let config_uri = cogitator_lsp::fs_path_to_uri(&config);
    lsp.notify(
        "textDocument/didOpen",
        serde_json::json!({"textDocument": {"uri": config_uri, "languageId": "yaml",
            "version": 1, "text": text}}),
    );
    let pulled = lsp.request(
        "textDocument/diagnostic",
        serde_json::json!({"textDocument": {"uri": config_uri}}),
    );
    let items = pulled["result"]["items"].as_array().expect("items");
    assert_eq!(items.len(), 1, "exactly the unknown-key warning: {pulled}");
    assert!(
        items[0]["message"]
            .as_str()
            .is_some_and(|m| m.contains("'bogus'") && m.contains("under lint:")),
        "{items:?}"
    );
    // completion inside lint: offers the checks, documented
    let completion = lsp.request(
        "textDocument/completion",
        serde_json::json!({"textDocument": {"uri": config_uri},
                           "position": {"line": 1, "character": 2}}),
    );
    let labels: Vec<&str> = completion["result"]
        .as_array()
        .expect("items")
        .iter()
        .filter_map(|i| i["label"].as_str())
        .collect();
    assert!(labels.contains(&"dead_models"), "{labels:?}");
    // hover documents the key from the schema table
    let hover = lsp.request(
        "textDocument/hover",
        serde_json::json!({"textDocument": {"uri": config_uri},
                           "position": {"line": 1, "character": 4}}),
    );
    assert!(
        hover["result"]["contents"]
            .as_str()
            .is_some_and(|c| c.contains("SELECT *")),
        "{hover}"
    );

    let _ = lsp.request("shutdown", serde_json::json!({}));
    lsp.notify("exit", serde_json::json!({}));
}

/// Opt-in lint, hot: no items while `lint:` is absent; writing
/// cogitator.yml with select_star: true and forcing a reload makes the
/// next pull carry the warning (with its column-list quickfix data).
#[test]
fn lint_toggles_hot_through_cogitator_yml() {
    let src = fixture_root().canonicalize().unwrap();
    let root = std::env::temp_dir().join(format!("cogitator-e2e-lint-{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    copy_fixture(&src, &root);
    let _cleanup = RemoveOnDrop(root.clone());

    let (mut lsp, _uri) = booted_in(&root, &[]);
    let probe_uri = cogitator_lsp::fs_path_to_uri(&root.join("models/mart_probe.sql"));
    lsp.notify(
        "textDocument/didOpen",
        serde_json::json!({"textDocument": {"uri": probe_uri, "languageId": "sql", "version": 1,
            "text": "select * from {{ ref('stg_customers') }}\n"}}),
    );
    let quiet = lsp.request(
        "textDocument/diagnostic",
        serde_json::json!({"textDocument": {"uri": probe_uri}}),
    );
    assert!(
        quiet["result"]["items"]
            .as_array()
            .is_some_and(|i| i.iter().all(|d| d["code"] != "lint-select-star")),
        "{quiet}"
    );

    std::fs::write(root.join("cogitator.yml"), "lint:\n  select_star: true\n").unwrap();
    reload_config(&mut lsp);
    let mut flagged = serde_json::Value::Null;
    for _ in 0..100 {
        let pulled = lsp.request(
            "textDocument/diagnostic",
            serde_json::json!({"textDocument": {"uri": probe_uri}}),
        );
        if let Some(item) = pulled["result"]["items"]
            .as_array()
            .and_then(|i| i.iter().find(|d| d["code"] == "lint-select-star"))
        {
            flagged = item.clone();
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(
        flagged["message"]
            .as_str()
            .is_some_and(|m| m.contains("SELECT *")),
        "lint never switched on: {flagged}"
    );
    assert!(
        flagged["data"]["suggestions"][0]
            .as_str()
            .is_some_and(|c| c.contains("customer_id")),
        "{flagged}"
    );

    let _ = lsp.request("shutdown", serde_json::json!({}));
    lsp.notify("exit", serde_json::json!({}));
}

/// dbt artifacts end to end: fake target/ JSONs feed the last-run
/// lens, run/test-failure diagnostics, the drift hint, and the stamp
/// check picks up a rewrite without restart (03 §10).
#[test]
fn artifact_backed_lenses_and_diagnostics() {
    let src = fixture_root().canonicalize().unwrap();
    let root = std::env::temp_dir().join(format!("cogitator-e2e-artifacts-{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    copy_fixture(&src, &root);
    let _cleanup = RemoveOnDrop(root.clone());
    let target = root.join("target");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(
        target.join("run_results.json"),
        r#"{"metadata": {"generated_at": "2026-11-03T10:00:00Z"},
            "results": [
              {"unique_id": "model.jaffle_shop.customers", "status": "success", "execution_time": 3.2},
              {"unique_id": "model.jaffle_shop.orders", "status": "error", "message": "boom"},
              {"unique_id": "test.jaffle_shop.not_null_customers_customer_id.abc",
               "status": "fail", "message": "Got 3 results"}
            ]}"#,
    )
    .unwrap();
    std::fs::write(
        target.join("manifest.json"),
        r#"{"nodes": {
            "model.jaffle_shop.customers": {"resource_type": "model", "name": "customers"},
            "model.jaffle_shop.orders": {"resource_type": "model", "name": "orders"},
            "test.jaffle_shop.not_null_customers_customer_id.abc":
                {"resource_type": "test", "attached_node": "model.jaffle_shop.customers"}
        }}"#,
    )
    .unwrap();

    let (mut lsp, customers_uri) = booted_in(&root, &[]);

    // 1. the last-run lens on a green model
    let lenses = lsp.request(
        "textDocument/codeLens",
        serde_json::json!({"textDocument": {"uri": customers_uri}}),
    );
    let titles: Vec<&str> = lenses["result"]
        .as_array()
        .expect("lenses")
        .iter()
        .filter_map(|l| l["command"]["title"].as_str())
        .collect();
    assert!(
        titles.iter().any(|t| t.starts_with("last run: OK in 3.2")),
        "{titles:?}"
    );

    // 2. failed run → warning with the adapter message
    let orders_uri = cogitator_lsp::fs_path_to_uri(&root.join("models/orders.sql"));
    let pulled = lsp.request(
        "textDocument/diagnostic",
        serde_json::json!({"textDocument": {"uri": orders_uri}}),
    );
    let items = pulled["result"]["items"].as_array().expect("items");
    assert!(
        items.iter().any(|d| d["code"] == "artifact-run-failed"
            && d["message"].as_str().is_some_and(|m| m.contains("boom"))),
        "{items:?}"
    );

    // 3. failed test lands on the model file AND the schema entry
    let pulled = lsp.request(
        "textDocument/diagnostic",
        serde_json::json!({"textDocument": {"uri": customers_uri}}),
    );
    assert!(
        pulled["result"]["items"]
            .as_array()
            .is_some_and(|i| i.iter().any(|d| d["code"] == "artifact-test-failed")),
        "{pulled}"
    );
    let schema_uri = cogitator_lsp::fs_path_to_uri(&root.join("models/schema.yml"));
    let pulled = lsp.request(
        "textDocument/diagnostic",
        serde_json::json!({"textDocument": {"uri": schema_uri}}),
    );
    let schema_item = pulled["result"]["items"]
        .as_array()
        .and_then(|i| {
            i.iter()
                .find(|d| d["code"] == "artifact-test-failed")
                .cloned()
        })
        .expect("schema test-failure diagnostic");
    assert!(
        schema_item["range"]["start"]["line"].as_u64().unwrap() > 0,
        "anchored at the model's entry: {schema_item}"
    );

    // 4. a model the manifest misses gets the drift hint
    let drift_uri = cogitator_lsp::fs_path_to_uri(&root.join("models/customer_orders.sql"));
    let pulled = lsp.request(
        "textDocument/diagnostic",
        serde_json::json!({"textDocument": {"uri": drift_uri}}),
    );
    let drift = pulled["result"]["items"]
        .as_array()
        .and_then(|i| i.iter().find(|d| d["code"] == "artifact-drift").cloned())
        .expect("drift hint");
    assert!(
        drift["relatedInformation"][0]["location"]["uri"]
            .as_str()
            .is_some_and(|u| u.ends_with("manifest.json")),
        "{drift}"
    );

    // 4b. freshness verdicts become lenses on the source YAML entries
    std::fs::write(
        target.join("sources.json"),
        r#"{"results": [{"unique_id": "source.jaffle_shop.stripe.payments",
                         "status": "pass", "snapshotted_at": "2026-11-03T09:00:00Z"}]}"#,
    )
    .unwrap();
    let mut fresh_titles: Vec<String> = Vec::new();
    for _ in 0..100 {
        let lenses = lsp.request(
            "textDocument/codeLens",
            serde_json::json!({"textDocument": {"uri": schema_uri}}),
        );
        fresh_titles = lenses["result"]
            .as_array()
            .map(|l| {
                l.iter()
                    .filter_map(|x| x["command"]["title"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        if fresh_titles
            .iter()
            .any(|t| t.starts_with("freshness: pass"))
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(
        fresh_titles
            .iter()
            .any(|t| t == "freshness: pass · 2026-11-03T09:00:00Z"),
        "{fresh_titles:?}"
    );

    // 5. the stamp check sees a rewrite — no restart, no reload command
    std::fs::write(
        target.join("run_results.json"),
        r#"{"results": [{"unique_id": "model.jaffle_shop.customers", "status": "error",
                         "message": "late failure"}]}"#,
    )
    .unwrap();
    let mut changed = false;
    for _ in 0..100 {
        let lenses = lsp.request(
            "textDocument/codeLens",
            serde_json::json!({"textDocument": {"uri": customers_uri}}),
        );
        if lenses["result"]
            .as_array()
            .is_some_and(|l| l.iter().any(|x| x["command"]["title"] == "last run: ERROR"))
        {
            changed = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(changed, "stamp refresh never surfaced the rewrite");

    let _ = lsp.request("shutdown", serde_json::json!({}));
    lsp.notify("exit", serde_json::json!({}));
}

/// The job registry (05 §9): dbt.runSelection answers a token for a
/// fake long-running dbt on PATH, dbt.cancel kills it, bad selectors
/// and unknown tokens answer InvalidParams Guidance.
#[cfg(unix)]
#[test]
fn dbt_jobs_start_and_cancel_by_token() {
    use std::os::unix::fs::PermissionsExt;
    let bin_dir =
        std::env::temp_dir().join(format!("cogitator-e2e-fakedbt-{}", std::process::id()));
    std::fs::create_dir_all(&bin_dir).unwrap();
    let fake = bin_dir.join("dbt");
    std::fs::write(&fake, "#!/bin/sh\nsleep 30\n").unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    let _cleanup = RemoveOnDrop(bin_dir.clone());
    let path_env = format!(
        "{}:{}",
        bin_dir.display(),
        std::env::var("PATH").unwrap_or_default()
    );

    let (mut lsp, _uri) = booted(&[("PATH", &path_env)]);

    let started = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({"command": "dbt.runSelection",
                           "arguments": [{"selector": "tag:nightly", "kind": "run"}]}),
    );
    let token = started["result"]["token"].as_u64().expect("token");
    assert!(token >= 1, "{started}");

    let cancelled = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({"command": "dbt.cancel", "arguments": [{"token": token}]}),
    );
    assert_eq!(cancelled["result"]["token"], token, "{cancelled}");
    assert!(
        cancelled["result"]["label"]
            .as_str()
            .is_some_and(|l| l.contains("tag:nightly")),
        "{cancelled}"
    );

    // cancelling again: the token is gone
    let missing = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({"command": "dbt.cancel", "arguments": [{"token": token}]}),
    );
    assert_eq!(missing["error"]["code"], -32602, "{missing}");

    // argument-shaped selectors never reach argv
    let bad = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({"command": "dbt.runSelection",
                           "arguments": [{"selector": "--vars oops"}]}),
    );
    assert_eq!(bad["error"]["code"], -32602, "{bad}");
    assert!(
        bad["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("flag")),
        "{bad}"
    );

    let _ = lsp.request("shutdown", serde_json::json!({}));
    lsp.notify("exit", serde_json::json!({}));
}

/// Exposures as graph citizens (03 §10): symbol search, the project
/// DAG, and dangling-target warnings on the YAML entry.
#[test]
fn exposures_join_the_graph() {
    let src = fixture_root().canonicalize().unwrap();
    let root = std::env::temp_dir().join(format!("cogitator-e2e-exposures-{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    copy_fixture(&src, &root);
    let _cleanup = RemoveOnDrop(root.clone());
    let exposures_yml = root.join("models/exposures.yml");
    std::fs::write(
        &exposures_yml,
        "version: 2\nexposures:\n  - name: weekly_kpis\n    type: dashboard\n    depends_on:\n      - ref('customers')\n      - ref('customrs')\n",
    )
    .unwrap();

    let (mut lsp, _uri) = booted_in(&root, &[]);

    // workspace/symbol with the exposure: prefix
    let symbols = lsp.request(
        "workspace/symbol",
        serde_json::json!({"query": "exposure:"}),
    );
    assert!(
        symbols["result"]
            .as_array()
            .is_some_and(|s| s.iter().any(|x| x["name"] == "weekly_kpis")),
        "{symbols}"
    );

    // the DAG carries the exposure node and its edge
    let dag = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({"command": "dbt.projectGraph", "arguments": [{}]}),
    );
    let node = dag["result"]["nodes"]
        .as_array()
        .and_then(|n| n.iter().find(|x| x["name"] == "weekly_kpis").cloned())
        .expect("exposure node");
    assert_eq!(node["kind"], "exposure", "{node}");
    assert!(
        dag["result"]["edges"].as_array().is_some_and(|e| e
            .iter()
            .any(|x| x["from"] == "model:customers" && x["to"] == "exposure:weekly_kpis")),
        "{dag}"
    );

    // the dangling target warns at the exposure's entry, with a hint
    let exposures_uri = cogitator_lsp::fs_path_to_uri(&exposures_yml);
    let text = std::fs::read_to_string(&exposures_yml).unwrap();
    lsp.notify(
        "textDocument/didOpen",
        serde_json::json!({"textDocument": {"uri": exposures_uri, "languageId": "yaml",
            "version": 1, "text": text}}),
    );
    let pulled = lsp.request(
        "textDocument/diagnostic",
        serde_json::json!({"textDocument": {"uri": exposures_uri}}),
    );
    let dangling = pulled["result"]["items"]
        .as_array()
        .and_then(|i| {
            i.iter()
                .find(|d| d["code"] == "dangling-exposure-ref")
                .cloned()
        })
        .expect("dangling exposure warning");
    assert!(
        dangling["message"]
            .as_str()
            .is_some_and(|m| m.contains("'customrs'") && m.contains("customers")),
        "{dangling}"
    );

    // impact: stg_customers feeds customers (and through it the
    // exposure); the column closure traces customer_id into customers
    let impact = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({"command": "dbt.impact",
                           "arguments": [{"model": "stg_customers", "column": "customer_id"}]}),
    );
    let report = &impact["result"];
    assert!(
        report["models"].as_array().is_some_and(|m| m
            .iter()
            .any(|n| n["name"] == "customers" && n["depth"] == 1)),
        "{report}"
    );
    assert!(
        report["exposures"]
            .as_array()
            .is_some_and(|e| e.iter().any(|x| x == "weekly_kpis")),
        "{report}"
    );
    assert!(
        report["summary"]
            .as_str()
            .is_some_and(|t| t.starts_with("stg_customers.customer_id feeds")),
        "{report}"
    );
    assert!(
        report["columnHits"].as_array().is_some_and(|h| h
            .iter()
            .any(|x| x["model"] == "customers" && x["column"] == "customer_id")),
        "{report}"
    );

    // dead models: the exposure keeps 'customers' alive; customer_orders
    // has no dependents and no selector naming it
    let dead = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({"command": "dbt.deadModels", "arguments": [{}]}),
    );
    let names: Vec<&str> = dead["result"]["models"]
        .as_array()
        .expect("models")
        .iter()
        .filter_map(|m| m["name"].as_str())
        .collect();
    assert!(names.contains(&"customer_orders"), "{names:?}");
    assert!(!names.contains(&"customers"), "{names:?}");

    let _ = lsp.request("shutdown", serde_json::json!({}));
    lsp.notify("exit", serde_json::json!({}));
}

/// Package awareness (03 §10): two-arg ref('pkg','model') resolves to
/// the package model (and never false-flags the package name), and a
/// declared-but-not-installed package warns in packages.yml.
#[test]
fn package_awareness_two_arg_ref_and_deps() {
    let src = fixture_root().canonicalize().unwrap();
    let root = std::env::temp_dir().join(format!("cogitator-e2e-packages-{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    copy_fixture(&src, &root);
    let _cleanup = RemoveOnDrop(root.clone());
    let packages_yml = root.join("packages.yml");
    std::fs::write(
        &packages_yml,
        "packages:\n  - package: dbt-labs/dbt_utils\n    version: 1.3.0\n  - local: dbt_packages/jaffle_package\n",
    )
    .unwrap();

    let (mut lsp, _uri) = booted_in(&root, &[]);

    // a probe model using the two-arg form
    let probe_uri = cogitator_lsp::fs_path_to_uri(&root.join("models/pkg_probe.sql"));
    let text = "select * from {{ ref('jaffle_package', 'stg_customer_status') }}\n";
    lsp.notify(
        "textDocument/didOpen",
        serde_json::json!({"textDocument": {"uri": probe_uri, "languageId": "sql",
            "version": 1, "text": text}}),
    );
    // no dangling-ref false positive on the package name
    let pulled = lsp.request(
        "textDocument/diagnostic",
        serde_json::json!({"textDocument": {"uri": probe_uri}}),
    );
    assert!(
        pulled["result"]["items"]
            .as_array()
            .is_some_and(|i| i.iter().all(|d| d["code"] != "dangling-ref")),
        "{pulled}"
    );
    // definition on the MODEL argument jumps into the package
    let character = text.find("stg_customer_status").unwrap() as u32 + 2;
    let definition = lsp.request(
        "textDocument/definition",
        serde_json::json!({"textDocument": {"uri": probe_uri},
                           "position": {"line": 0, "character": character}}),
    );
    assert!(
        definition["result"]["uri"].as_str().is_some_and(
            |u| u.ends_with("dbt_packages/jaffle_package/models/stg_customer_status.sql")
        ),
        "{definition}"
    );

    // packages.yml: the uninstalled hub package warns, the local one not
    let packages_uri = cogitator_lsp::fs_path_to_uri(&packages_yml);
    let text = std::fs::read_to_string(&packages_yml).unwrap();
    lsp.notify(
        "textDocument/didOpen",
        serde_json::json!({"textDocument": {"uri": packages_uri, "languageId": "yaml",
            "version": 1, "text": text}}),
    );
    let pulled = lsp.request(
        "textDocument/diagnostic",
        serde_json::json!({"textDocument": {"uri": packages_uri}}),
    );
    let items = pulled["result"]["items"].as_array().expect("items");
    assert_eq!(items.len(), 1, "{items:?}");
    assert!(
        items[0]["message"]
            .as_str()
            .is_some_and(|m| m.contains("'dbt_utils'") && m.contains("dbt deps")),
        "{items:?}"
    );

    let _ = lsp.request("shutdown", serde_json::json!({}));
    lsp.notify("exit", serde_json::json!({}));
}

/// Inlay hints, document links and linked editing (01 §6.3) over the
/// shared fixture — read-only requests, no fixture mutation.
#[test]
fn inlay_links_and_linked_editing() {
    let (mut lsp, customers_uri) = booted(&[]);

    // document links: every ref in customers.sql targets its model file
    let links = lsp.request(
        "textDocument/documentLink",
        serde_json::json!({"textDocument": {"uri": customers_uri}}),
    );
    let targets: Vec<&str> = links["result"]
        .as_array()
        .expect("links")
        .iter()
        .filter_map(|l| l["target"].as_str())
        .collect();
    assert!(
        targets
            .iter()
            .any(|t| t.ends_with("staging/stg_customers.sql")),
        "{targets:?}"
    );

    // inlay hints: the ref resolves to its warehouse relation, and a
    // macro call site names its positional parameters
    let probe_uri = cogitator_lsp::fs_path_to_uri(
        &fixture_root()
            .canonicalize()
            .unwrap()
            .join("models/inlay_probe.sql"),
    );
    lsp.notify(
        "textDocument/didOpen",
        serde_json::json!({"textDocument": {"uri": probe_uri, "languageId": "sql", "version": 1,
            "text": "select {{ full_name('a', 'b') }} from {{ ref('stg_customers') }}\n"}}),
    );
    let hints = lsp.request(
        "textDocument/inlayHint",
        serde_json::json!({"textDocument": {"uri": probe_uri},
                           "range": {"start": {"line": 0, "character": 0},
                                     "end": {"line": 2, "character": 0}}}),
    );
    let labels: Vec<&str> = hints["result"]
        .as_array()
        .expect("hints")
        .iter()
        .filter_map(|h| h["label"].as_str())
        .collect();
    assert!(
        labels
            .iter()
            .any(|l| l.starts_with("→ jaffle_shop.main.stg_customers")),
        "{labels:?}"
    );
    assert!(labels.contains(&"first_name:"), "{labels:?}");
    assert!(labels.contains(&"last_name:"), "{labels:?}");

    // linked editing: a CTE name edits together with its uses
    let le_uri = cogitator_lsp::fs_path_to_uri(
        &fixture_root()
            .canonicalize()
            .unwrap()
            .join("models/le_probe.sql"),
    );
    lsp.notify(
        "textDocument/didOpen",
        serde_json::json!({"textDocument": {"uri": le_uri, "languageId": "sql", "version": 1,
            "text": "with t as (select 1 as x) select * from t\n"}}),
    );
    let linked = lsp.request(
        "textDocument/linkedEditingRange",
        serde_json::json!({"textDocument": {"uri": le_uri},
                           "position": {"line": 0, "character": 5}}),
    );
    assert_eq!(
        linked["result"]["ranges"].as_array().map(|r| r.len()),
        Some(2),
        "{linked}"
    );
    // a plain column is NOT linked-editable
    let not_linked = lsp.request(
        "textDocument/linkedEditingRange",
        serde_json::json!({"textDocument": {"uri": le_uri},
                           "position": {"line": 0, "character": 23}}),
    );
    assert!(not_linked["result"].is_null(), "{not_linked}");

    let _ = lsp.request("shutdown", serde_json::json!({}));
    lsp.notify("exit", serde_json::json!({}));
}
