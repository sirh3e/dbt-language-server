//! `dbt.lineageSite` over the wire (the VS Code view and the Neovim/Emacs
//! page) and the `cogitator lineage-site` subcommand.

mod harness;
use harness::jaffle;
use serde_json::json;

#[test]
fn lineage_site_answers_the_dag_with_columns() {
    let project = jaffle();
    let (mut lsp, uri) = project.booted();
    let answer = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.lineageSite", "arguments": [{"uri": uri, "assets": true}]}),
    );
    let result = &answer["result"];
    let site = &result["site"];
    assert_eq!(site["format"], 1, "{answer}");
    assert_eq!(site["project"], "jaffle_shop");
    let dag = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.projectGraph", "arguments": [{}]}),
    );
    assert_eq!(
        site["nodes"].as_array().unwrap().len(),
        dag["result"]["nodes"].as_array().unwrap().len()
    );
    let stg = site["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == "model:stg_customers")
        .expect("stg_customers");
    let id = stg["columns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "customer_id")
        .expect("customer_id");
    assert_eq!(id["inputs"][0]["node"], "model:raw_customers", "{id}");
    assert_eq!(id["inputs"][0]["column"], "id");
    let orders = site["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == "model:orders")
        .unwrap();
    assert_eq!(orders["lineage"], "declined");
    assert!(result["assets"]["script"]
        .as_str()
        .unwrap()
        .contains("CogitatorLineage"));
    assert!(result["assets"]["style"].as_str().unwrap().contains("--bg"));
    assert_eq!(result["counts"], site["counts"]);

    // write: the page lands under target/, the data stays off the wire
    let written = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.lineageSite", "arguments": [{"uri": uri, "write": true}]}),
    );
    let path = written["result"]["written"]["path"].as_str().expect("path");
    assert!(written["result"].get("site").is_none(), "{written}");
    let page = std::fs::read_to_string(path).unwrap();
    assert!(path.ends_with("index.html"));
    assert!(std::path::Path::new(path).starts_with(project.root.join("target/cogitator-lineage")));
    assert!(page.contains("\"stg_customers\""));
}

#[test]
fn the_subcommand_writes_a_self_contained_page() {
    let project = jaffle();
    let out = project.root.join("site-out");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_cogitator"))
        .args(["lineage-site", "--project-dir"])
        .arg(&project.root)
        .arg("--out")
        .arg(&out)
        .output()
        .expect("binary runs");
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.trim().ends_with("index.html"), "{stdout}");
    let page = std::fs::read_to_string(out.join("index.html")).unwrap();
    assert!(page.contains("id=\"cogitator-data\""));
    assert!(page.contains("\"model:customers\""));
    assert!(!page.contains("https://"));
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("lineage.json")).unwrap()).unwrap();
    assert!(json["counts"]["columnsTraced"].as_u64().unwrap() > 0);

    let missing = std::process::Command::new(env!("CARGO_BIN_EXE_cogitator"))
        .args(["lineage-site", "--project-dir"])
        .arg(project.root.join("models"))
        .arg("--out")
        .arg(&out)
        .output()
        .unwrap();
    assert_eq!(missing.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&missing.stderr);
    assert!(
        stderr.contains("dbt_project.yml") && stderr.contains("Solution"),
        "{stderr}"
    );
}
