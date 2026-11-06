//! End-to-end suite (specs/rust-port/05 §7) — this file's theme only;
//! the framed-LSP harness lives in tests/harness/.

mod harness;
#[allow(unused_imports)]
use harness::*;
use std::io::Write;
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

/// The editor-visible log stream (05 §6.3 addendum): a plain boot
/// already surfaces the INFO overview lines as window/logMessage.
#[test]
fn server_logs_reach_the_editor() {
    let (mut lsp, _uri) = booted(&[]);
    // stir once more so any straggling notifications get stashed
    let _ = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({"command": "dbt.status", "arguments": [{}]}),
    );
    let logs: Vec<&str> = lsp
        .stash
        .iter()
        .filter(|m| m["method"] == "window/logMessage")
        .filter_map(|m| m["params"]["message"].as_str())
        .collect();
    assert!(
        logs.iter().any(|m| m.contains("context refreshed")),
        "no overview line in {logs:?}"
    );
    assert!(
        lsp.stash
            .iter()
            .filter(|m| m["method"] == "window/logMessage")
            .all(|m| m["params"]["type"]
                .as_i64()
                .is_some_and(|t| (1..=3).contains(&t))),
        "default level is info — no debug lines: {logs:?}"
    );
    let _ = lsp.request("shutdown", serde_json::json!({}));
    lsp.notify("exit", serde_json::json!({}));
}

/// Verbosity is hot: cogitator.yml log.client_level and $/setTrace
/// both tune the editor-visible stream, last writer wins.
#[test]
fn log_verbosity_tunes_hot() {
    let src = fixture_root().canonicalize().unwrap();
    let root = std::env::temp_dir().join(format!("cogitator-e2e-loglvl-{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    copy_fixture(&src, &root);
    let _cleanup = RemoveOnDrop(root.clone());

    let (mut lsp, uri) = booted_in(&root, &[]);
    let saved = serde_json::json!({"textDocument": {"uri": uri}});
    let debug_logs = |lsp: &Lsp| {
        lsp.stash
            .iter()
            .filter(|m| m["method"] == "window/logMessage" && m["params"]["type"] == 4)
            .count()
    };

    // default info: a didSave produces no debug lines
    lsp.notify("textDocument/didSave", saved.clone());
    let _ = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({"command": "dbt.status", "arguments": [{}]}),
    );
    assert_eq!(debug_logs(&lsp), 0, "info level shows no debug lines");

    // cogitator.yml log.client_level: debug → "handled" lines appear
    std::fs::write(root.join("cogitator.yml"), "log:\n  client_level: debug\n").unwrap();
    let reply = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({"command": "dbt.reloadConfig", "arguments": [{}]}),
    );
    assert!(reply.get("result").is_some(), "{reply}");
    let mut saw_debug = 0;
    for _ in 0..100 {
        lsp.notify("textDocument/didSave", saved.clone());
        let _ = lsp.request(
            "workspace/executeCommand",
            serde_json::json!({"command": "dbt.status", "arguments": [{}]}),
        );
        saw_debug = debug_logs(&lsp);
        if saw_debug > 0 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(saw_debug > 0, "debug level never surfaced");

    // $/setTrace off: only errors from here — no new info/debug lines
    lsp.notify("$/setTrace", serde_json::json!({"value": "off"}));
    let _ = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({"command": "dbt.status", "arguments": [{}]}),
    );
    let baseline: usize = lsp
        .stash
        .iter()
        .filter(|m| m["method"] == "window/logMessage" && m["params"]["type"] != 1)
        .count();
    lsp.notify("textDocument/didSave", saved.clone());
    let _ = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({"command": "dbt.status", "arguments": [{}]}),
    );
    let after: usize = lsp
        .stash
        .iter()
        .filter(|m| m["method"] == "window/logMessage" && m["params"]["type"] != 1)
        .count();
    assert_eq!(after, baseline, "setTrace off silences non-errors");

    let _ = lsp.request("shutdown", serde_json::json!({}));
    lsp.notify("exit", serde_json::json!({}));
}

/// Multi-root (01 §3 addendum): the second workspace folder gets its
/// own context — hover/definition and diagnostics for its files come
/// from THAT project, dbt.status lists it, and a file from a third,
/// unlisted project joins lazily on didOpen.
#[test]
fn multi_root_workspace_folders() {
    let base = std::env::temp_dir().join(format!("cogitator-e2e-multiroot-{}", std::process::id()));
    let _cleanup = RemoveOnDrop(base.clone());
    let make_project = |name: &str| {
        let root = base.join(name);
        let models = root.join("models");
        std::fs::create_dir_all(&models).unwrap();
        std::fs::write(
            root.join("dbt_project.yml"),
            format!("name: {name}\nprofile: {name}\nmodel-paths: [\"models\"]\n"),
        )
        .unwrap();
        std::fs::write(models.join(format!("{name}_base.sql")), "select 1 as id").unwrap();
        std::fs::write(
            models.join(format!("{name}_top.sql")),
            format!("select * from {{{{ ref('{name}_base') }}}}"),
        )
        .unwrap();
        root
    };
    let second = make_project("beta");
    let third = make_project("gamma");
    let primary = fixture_root().canonicalize().unwrap();

    let mut lsp = Lsp::start_with_env(&[]);
    let _ = lsp.request(
        "initialize",
        serde_json::json!({
            "workspaceFolders": [
                {"uri": cogitator_lsp::fs_path_to_uri(&primary), "name": "jaffle"},
                {"uri": cogitator_lsp::fs_path_to_uri(&second), "name": "beta"},
            ],
            "capabilities": {},
            "initializationOptions": {"profilesDir": primary.join(".dbt").to_string_lossy()},
        }),
    );
    lsp.notify("initialized", serde_json::json!({}));

    // the secondary folder's context arrives and dbt.status lists it
    let status = status_until(&mut lsp, |s| {
        s["project"]["extraRoots"]
            .as_array()
            .is_some_and(|roots| !roots.is_empty())
    });
    assert!(
        status["project"]["extraRoots"][0]
            .as_str()
            .is_some_and(|r| r.ends_with("beta")),
        "{status}"
    );

    // definition inside the secondary project resolves within IT
    let beta_top = second.join("models/beta_top.sql");
    let beta_uri = cogitator_lsp::fs_path_to_uri(&beta_top);
    lsp.notify(
        "textDocument/didOpen",
        serde_json::json!({"textDocument": {"uri": beta_uri, "languageId": "sql", "version": 1,
            "text": std::fs::read_to_string(&beta_top).unwrap()}}),
    );
    let def = lsp.request(
        "textDocument/definition",
        serde_json::json!({"textDocument": {"uri": beta_uri},
                           "position": {"line": 0, "character": 25}}),
    );
    assert!(
        def["result"]["uri"]
            .as_str()
            .is_some_and(|u| u.ends_with("beta_base.sql")),
        "{def}"
    );

    // a file from a project NOBODY listed joins lazily on didOpen
    let gamma_top = third.join("models/gamma_top.sql");
    let gamma_uri = cogitator_lsp::fs_path_to_uri(&gamma_top);
    lsp.notify(
        "textDocument/didOpen",
        serde_json::json!({"textDocument": {"uri": gamma_uri, "languageId": "sql", "version": 1,
            "text": std::fs::read_to_string(&gamma_top).unwrap()}}),
    );
    let status = status_until(&mut lsp, |s| {
        s["project"]["extraRoots"].as_array().is_some_and(|roots| {
            roots
                .iter()
                .any(|r| r.as_str().is_some_and(|r| r.ends_with("gamma")))
        })
    });
    assert_eq!(
        status["project"]["extraRoots"].as_array().map(|r| r.len()),
        Some(2),
        "{status}"
    );
    let def = lsp.request(
        "textDocument/definition",
        serde_json::json!({"textDocument": {"uri": gamma_uri},
                           "position": {"line": 0, "character": 25}}),
    );
    assert!(
        def["result"]["uri"]
            .as_str()
            .is_some_and(|u| u.ends_with("gamma_base.sql")),
        "{def}"
    );

    // the primary project is untouched by all of it
    let status = lsp.request(
        "workspace/executeCommand",
        serde_json::json!({"command": "dbt.status", "arguments": [{}]}),
    );
    assert_eq!(
        status["result"]["project"]["name"], "jaffle_shop",
        "{status}"
    );
}
