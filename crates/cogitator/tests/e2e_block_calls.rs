//! End-to-end suite (specs/rust-port/05 §7) — macro calls inside
//! statement blocks, nested arguments and `{% call %}` blocks are found
//! by references, rename and the call hierarchy like `{{ m() }}`, and
//! `dbt.unresolved` reports none of the context functions beside them.

mod harness;
use harness::jaffle;
use serde_json::json;

fn position_of(text: &str, needle: &str, offset: u32) -> serde_json::Value {
    let at = text.find(needle).expect("needle");
    let line = text[..at].matches('\n').count();
    let col = at - text[..at].rfind('\n').map_or(0, |i| i + 1);
    json!({"line": line, "character": col as u32 + offset})
}

const PROBE_MODEL: &str = concat!(
    "{% set x = times_five(1) %}\n",
    "{% for c in range(3) %}{% if full_name(c, 'b') %}{% endif %}{% endfor %}\n",
    "{% set d = {'a': 1} %}{% for k, v in d.items() %}{% do log(k) %}{% endfor %}\n",
    "select {{ x | default(times_five(2)) }} from {{ ref('customers') }}\n",
);

const PROBE_MACRO: &str = concat!(
    "{% macro block_wrapper(cols) %}\n",
    "{% for c in cols.split(',') %}{% do full_name(c, 'b') %}{% endfor %}\n",
    "{% call full_name('c', 'd') %}x{% endcall %}\n",
    "{{ return(times_five(3)) }}\n",
    "{% endmacro %}\n",
);

#[test]
fn block_calls_are_references_renames_and_call_edges() {
    let project = jaffle();
    std::fs::write(project.root.join("models/block_probe.sql"), PROBE_MODEL).unwrap();
    std::fs::write(project.root.join("macros/block_wrapper.sql"), PROBE_MACRO).unwrap();
    let (mut lsp, _) = project.booted();

    let macros = project.root.join("macros/jaffle_macros.sql");
    let macros_text = std::fs::read_to_string(&macros).unwrap();
    let macros_uri = cogitator_lsp::fs_path_to_uri(&macros);
    lsp.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": macros_uri, "languageId": "sql", "version": 1,
            "text": macros_text}}),
    );

    // references of times_five from its header: the set, the filter
    // argument and the return's argument
    let refs = lsp.request(
        "textDocument/references",
        json!({"textDocument": {"uri": macros_uri},
            "position": position_of(&macros_text, "times_five(", 2),
            "context": {"includeDeclaration": false}}),
    );
    let locations = refs["result"].as_array().expect("references");
    let in_file = |file: &str| {
        locations
            .iter()
            .filter(|l| l["uri"].as_str().is_some_and(|u| u.ends_with(file)))
            .count()
    };
    assert_eq!(in_file("block_probe.sql"), 2, "{refs}");
    assert_eq!(in_file("block_wrapper.sql"), 1, "{refs}");

    // rename full_name: the if, the do and the call block
    let edit = lsp.request(
        "textDocument/rename",
        json!({"textDocument": {"uri": macros_uri},
            "position": position_of(&macros_text, "full_name(", 2), "newName": "person_name"}),
    );
    let changes = edit["result"]["documentChanges"]
        .as_array()
        .expect("documentChanges");
    let edits_in = |file: &str| -> usize {
        changes
            .iter()
            .filter(|c| {
                c["textDocument"]["uri"]
                    .as_str()
                    .is_some_and(|u| u.ends_with(file))
            })
            .map(|c| c["edits"].as_array().map_or(0, Vec::len))
            .sum()
    };
    assert_eq!(edits_in("block_probe.sql"), 1, "{edit}");
    assert_eq!(edits_in("block_wrapper.sql"), 2, "{edit}");

    // incoming calls of full_name: the probe model and the wrapper macro
    let items = lsp.request(
        "textDocument/prepareCallHierarchy",
        json!({"textDocument": {"uri": macros_uri},
            "position": position_of(&macros_text, "full_name(", 2)}),
    );
    let incoming = lsp.request(
        "callHierarchy/incomingCalls",
        json!({"item": items["result"][0].clone()}),
    );
    let callers: Vec<&str> = incoming["result"]
        .as_array()
        .expect("incoming")
        .iter()
        .filter_map(|c| c["from"]["name"].as_str())
        .collect();
    assert!(callers.contains(&"block_probe"), "{incoming}");
    assert!(callers.contains(&"block_wrapper"), "{incoming}");

    // nothing beside them is reported: range, log, items, split, return
    let unresolved = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.unresolved", "arguments": [{}]}),
    );
    assert_eq!(unresolved["result"]["count"], 0, "{unresolved}");

    lsp.shutdown();
}
