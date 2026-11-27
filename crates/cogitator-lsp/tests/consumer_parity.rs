//! Consumer parity: every `dbt.*` command the server registers
//! ([`cogitator_lsp::COMMANDS`]) is classified in `editors/parity.toml`, and
//! every command a person runs is wrapped by every editor that can wrap one
//! — or listed there as a gap that predates the rule. The gaps only shrink:
//! a new gap fails, and so does a listed gap the editor has since closed.
//!
//! Code actions and code lenses need no wrapping (LSP clients run them), so
//! a command the server only sends inside one is `internal`. The checklist
//! for a new command is the `editor-parity` skill
//! (`.claude/skills/editor-parity/SKILL.md`).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Deserialize;

const SKILL: &str = ".claude/skills/editor-parity/SKILL.md";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    commands: BTreeMap<String, Entry>,
    #[serde(default)]
    gaps: BTreeMap<String, BTreeMap<String, String>>,
    /// User commands Zed reaches through stock LSP, and how.
    #[serde(default)]
    zed: BTreeMap<String, ZedRoute>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ZedRoute {
    via: String,
    /// The file that sends the command in a lens or action.
    sent_in: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    kind: Kind,
    /// For an internal command: where the server sends it.
    via: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Kind {
    User,
    Internal,
}

/// An editor that wraps commands: its name, the files that count, and
/// where a new wrapper goes (said in a failure).
struct Consumer {
    name: &'static str,
    files: Vec<PathBuf>,
    add_to: &'static str,
}

/// An editor's sources, read: the quoted `"dbt.x"` strings in them.
struct Sources {
    name: &'static str,
    add_to: &'static str,
    referenced: BTreeSet<String>,
}

/// The quoted command names in `text`: `"dbt.run"` counts, prose does not.
fn referenced(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut rest = text;
    while let Some(at) = rest.find("\"dbt.") {
        let tail = &rest[at + 1..];
        let end = tail
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '.'))
            .unwrap_or(tail.len());
        if tail[end..].starts_with('"') && end > "dbt.".len() {
            out.insert(tail[..end].to_string());
        }
        rest = &tail[end..];
    }
    out
}

/// Every way the manifest, the server and the editors disagree, each said
/// with what to do about it.
fn problems(
    manifest: &Manifest,
    server: &[&str],
    consumers: &[Sources],
    spec: &str,
) -> Vec<String> {
    let server: BTreeSet<&str> = server.iter().copied().collect();
    let mut out = Vec::new();
    for command in &server {
        if !manifest.commands.contains_key(*command) {
            out.push(format!(
                "`{command}` is registered by the server but not classified in \
                 editors/parity.toml: add it as kind = \"user\" (and wrap it in every \
                 editor) or \"internal\" with `via` (see {SKILL})"
            ));
        }
        if !spec.contains(&format!("`{command}`")) {
            out.push(format!(
                "`{command}` has no row in specs/neovim-plugin/00-plugin.md's command \
                 table: document its argument and result"
            ));
        }
    }
    for (command, entry) in &manifest.commands {
        if !server.contains(command.as_str()) {
            out.push(format!(
                "editors/parity.toml classifies `{command}`, which the server does not \
                 register (cogitator_lsp::COMMANDS): remove it"
            ));
        }
        if entry.kind == Kind::Internal && entry.via.as_deref().is_none_or(str::is_empty) {
            out.push(format!(
                "internal `{command}` must say `via`: the code lens or code action the \
                 server sends it in"
            ));
        }
    }
    let known: BTreeSet<&str> = consumers.iter().map(|c| c.name).collect();
    for name in manifest.gaps.keys() {
        if !known.contains(name.as_str()) {
            out.push(format!(
                "editors/parity.toml has [gaps.{name}], which is no editor that wraps commands"
            ));
        }
    }
    let empty = BTreeMap::new();
    for consumer in consumers {
        let gaps = manifest.gaps.get(consumer.name).unwrap_or(&empty);
        for command in &consumer.referenced {
            if !server.contains(command.as_str()) {
                out.push(format!(
                    "{} references `{command}`, which the server does not register",
                    consumer.name
                ));
            }
        }
        for (command, entry) in &manifest.commands {
            if entry.kind != Kind::User {
                continue;
            }
            let wraps = consumer.referenced.contains(command);
            let listed = gaps.contains_key(command);
            if !wraps && !listed {
                out.push(format!(
                    "{} does not expose `{command}`: wrap it in {} (see {SKILL}); new \
                     commands get no [gaps] line",
                    consumer.name, consumer.add_to
                ));
            }
            if wraps && listed {
                out.push(format!(
                    "{} now exposes `{command}`: remove its line from [gaps.{}] in \
                     editors/parity.toml",
                    consumer.name, consumer.name
                ));
            }
        }
        for command in gaps.keys() {
            match manifest.commands.get(command) {
                None => out.push(format!(
                    "[gaps.{}] lists `{command}`, which editors/parity.toml does not classify",
                    consumer.name
                )),
                Some(entry) if entry.kind == Kind::Internal => out.push(format!(
                    "[gaps.{}] lists internal `{command}`: internal commands need no wrapper",
                    consumer.name
                )),
                Some(_) => {}
            }
        }
    }
    out
}

/// `[zed]`: each entry a user command, said how, named in the Zed
/// README, and named in the file it is sent in.
fn zed_problems(
    manifest: &Manifest,
    readme: &str,
    read: &dyn Fn(&str) -> Option<String>,
) -> Vec<String> {
    let mut out = Vec::new();
    for (command, route) in &manifest.zed {
        match manifest.commands.get(command) {
            None => out.push(format!(
                "[zed] lists `{command}`, which editors/parity.toml does not classify"
            )),
            Some(entry) if entry.kind == Kind::Internal => out.push(format!(
                "[zed] lists internal `{command}`: every client runs it already"
            )),
            Some(_) => {}
        }
        if route.via.trim().is_empty() {
            out.push(format!("[zed] `{command}` must say `via`"));
        }
        if !readme.contains(&format!("`{command}`")) {
            out.push(format!(
                "editors/zed/README.md does not name `{command}`, which [zed] says Zed reaches"
            ));
        }
        if let Some(file) = &route.sent_in {
            let sends = read(file).is_some_and(|text| text.contains(&format!("\"{command}\"")));
            if !sends {
                out.push(format!(
                    "[zed] says `{command}` is sent in {file}, which does not name it"
                ));
            }
        }
    }
    out
}

/// docs/commands.md has a section (`` ### `dbt.x` ``) for every command the
/// server registers, and none for a command it does not.
fn reference_problems(server: &[&str], reference: &str) -> Vec<String> {
    let documented: BTreeSet<String> = reference
        .lines()
        .filter_map(|line| line.strip_prefix("### `"))
        .filter_map(|rest| rest.strip_suffix('`'))
        .map(str::to_string)
        .collect();
    let mut out = Vec::new();
    for command in server {
        if !documented.contains(*command) {
            out.push(format!(
                "`{command}` has no section in docs/commands.md: add `### `{command}`` with its \
                 argument, result, side effects and editors"
            ));
        }
    }
    for command in &documented {
        if !server.contains(&command.as_str()) {
            out.push(format!(
                "docs/commands.md documents `{command}`, which the server does not register: remove it"
            ));
        }
    }
    out
}

/// What `extension.ts` registers and what `package.json` contributes, apart.
fn vscode_palette_problems(extension: &str, package: &str) -> Vec<String> {
    let names = |text: &str| -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        let mut rest = text;
        while let Some(at) = rest.find("\"cogitator.") {
            let tail = &rest[at + 1..];
            let end = tail
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '.'))
                .unwrap_or(tail.len());
            if tail[end..].starts_with('"') {
                out.insert(tail[..end].to_string());
            }
            rest = &tail[end..];
        }
        out
    };
    let registered = names(extension);
    let package: serde_json::Value = serde_json::from_str(package).expect("package.json parses");
    let contributed: BTreeSet<String> = package["contributes"]["commands"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| c["command"].as_str().map(str::to_string))
        .collect();
    let mut out = Vec::new();
    for name in registered.difference(&contributed) {
        out.push(format!(
            "vscode registers `{name}` in src/extension.ts but package.json does not \
             contribute it: add it to contributes.commands with a title"
        ));
    }
    for name in contributed.difference(&registered) {
        out.push(format!(
            "vscode's package.json contributes `{name}`, which src/extension.ts never registers"
        ));
    }
    out
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn lua_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            lua_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "lua") {
            out.push(path);
        }
    }
}

/// The editors that expose commands. Zed's extension has no command
/// surface, so the server walks it through its own dialogs instead (the
/// "dbt…" code action, `window/showMessageRequest` prompts): a command
/// counts as exposed in Zed when the dialogs' menu names it.
fn consumers(root: &Path) -> Vec<Consumer> {
    let mut nvim = Vec::new();
    lua_files(&root.join("editors/nvim/lua"), &mut nvim);
    lua_files(&root.join("editors/nvim/plugin"), &mut nvim);
    vec![
        Consumer {
            name: "nvim",
            files: nvim,
            add_to: "editors/nvim (a module of lua/cogitator/commands/, a :Dbt* in plugin/cogitator.lua, doc/cogitator.txt)",
        },
        Consumer {
            name: "emacs",
            files: vec![
                root.join("editors/emacs/cogitator.el"),
                root.join("editors/emacs/cogitator-graph.el"),
            ],
            add_to: "editors/emacs/cogitator.el (a cogitator-* command, README table)",
        },
        Consumer {
            name: "vscode",
            files: vec![root.join("editors/vscode/src/extension.ts")],
            add_to: "editors/vscode (src/extension.ts palette, package.json contributes, README)",
        },
        Consumer {
            name: "zed",
            files: vec![root.join("crates/cogitator/src/server/dialogs/menu.rs")],
            add_to: "the server's dialogs (crates/cogitator/src/server/dialogs/menu.rs: a choice in a group, asking what the command needs)",
        },
    ]
}

#[test]
fn every_editor_exposes_every_command_a_person_runs() {
    let root = root();
    let manifest: Manifest = toml::from_str(&read(&root.join("editors/parity.toml")))
        .expect("editors/parity.toml parses");
    let sources: Vec<Sources> = consumers(&root)
        .into_iter()
        .map(|c| Sources {
            name: c.name,
            add_to: c.add_to,
            referenced: c.files.iter().flat_map(|f| referenced(&read(f))).collect(),
        })
        .collect();
    let spec = read(&root.join("specs/neovim-plugin/00-plugin.md"));
    let mut found = problems(&manifest, cogitator_lsp::COMMANDS, &sources, &spec);
    found.extend(reference_problems(
        cogitator_lsp::COMMANDS,
        &read(&root.join("docs/commands.md")),
    ));
    found.extend(vscode_palette_problems(
        &read(&root.join("editors/vscode/src/extension.ts")),
        &read(&root.join("editors/vscode/package.json")),
    ));
    assert!(
        found.is_empty(),
        "consumer parity:\n- {}",
        found.join("\n- ")
    );
}

#[test]
fn zed_says_where_commands_are() {
    let root = root();
    let readme = read(&root.join("editors/zed/README.md"));
    assert!(
        readme.contains("dbt…"),
        "editors/zed/README.md must say that the dbt commands are in the \"dbt…\" code action"
    );
    let extension = read(&root.join("editors/zed/src/lib.rs"));
    assert!(
        extension.contains("\"dialogs\""),
        "editors/zed/src/lib.rs must ask the server for its dialogs (initializationOptions.dialogs)"
    );
    let manifest: Manifest = toml::from_str(&read(&root.join("editors/parity.toml")))
        .expect("editors/parity.toml parses");
    let found = zed_problems(&manifest, &readme, &|file| {
        std::fs::read_to_string(root.join(file)).ok()
    });
    assert!(found.is_empty(), "zed parity:\n- {}", found.join("\n- "));
}

#[cfg(test)]
mod checker {
    use super::*;
    use rstest::rstest;

    fn manifest(text: &str) -> Manifest {
        toml::from_str(text).expect("test manifest parses")
    }

    fn editor(name: &'static str, refs: &[&str]) -> Sources {
        Sources {
            name,
            add_to: "its files",
            referenced: refs.iter().map(|s| s.to_string()).collect(),
        }
    }

    const SPEC: &str = "| `dbt.run` | | `dbt.lens` | |";
    const BASE: &str = "[commands]\n\"dbt.run\" = { kind = \"user\" }\n\"dbt.lens\" = { kind = \"internal\", via = \"code lens\" }\n";

    #[test]
    fn quoted_names_count_and_prose_does_not() {
        let text = "exec(\"dbt.run\") -- dbt.test in prose; \"dbt.\" alone; '\"dbt.splitPlan\"'";
        assert_eq!(
            referenced(text),
            BTreeSet::from(["dbt.run".to_string(), "dbt.splitPlan".to_string()])
        );
    }

    #[rstest]
    #[case::in_sync(BASE, &["dbt.run", "dbt.lens"], &["dbt.run"], None)]
    #[case::internal_needs_no_wrapper(BASE, &["dbt.run", "dbt.lens"], &["dbt.run"], None)]
    #[case::unclassified(BASE, &["dbt.run", "dbt.lens", "dbt.new"], &["dbt.run"], Some("not classified"))]
    #[case::classified_but_gone(BASE, &["dbt.run"], &["dbt.run"], Some("does not register"))]
    #[case::not_wrapped(BASE, &["dbt.run", "dbt.lens"], &[], Some("does not expose `dbt.run`"))]
    #[case::grandfathered(
        &format!("{BASE}[gaps.a]\n\"dbt.run\" = \"old\"\n"),
        &["dbt.run", "dbt.lens"],
        &[],
        None
    )]
    #[case::closed_gap(
        &format!("{BASE}[gaps.a]\n\"dbt.run\" = \"old\"\n"),
        &["dbt.run", "dbt.lens"],
        &["dbt.run"],
        Some("remove its line")
    )]
    #[case::gap_for_internal(
        &format!("{BASE}[gaps.a]\n\"dbt.lens\" = \"old\"\n"),
        &["dbt.run", "dbt.lens"],
        &["dbt.run"],
        Some("need no wrapper")
    )]
    #[case::unknown_reference(BASE, &["dbt.run", "dbt.lens"], &["dbt.run", "dbt.typo"], Some("`dbt.typo`"))]
    #[case::internal_without_via(
        "[commands]\n\"dbt.run\" = { kind = \"user\" }\n\"dbt.lens\" = { kind = \"internal\" }\n",
        &["dbt.run", "dbt.lens"],
        &["dbt.run"],
        Some("must say `via`")
    )]
    #[case::gaps_of_no_editor(
        &format!("{BASE}[gaps.zed]\n\"dbt.run\" = \"old\"\n"),
        &["dbt.run", "dbt.lens"],
        &["dbt.run"],
        Some("[gaps.zed]")
    )]
    fn the_rule(
        #[case] text: &str,
        #[case] server: &[&str],
        #[case] refs: &[&str],
        #[case] expect: Option<&str>,
    ) {
        let found = problems(&manifest(text), server, &[editor("a", refs)], SPEC);
        match expect {
            None => assert!(found.is_empty(), "{found:?}"),
            Some(needle) => assert!(
                found.iter().any(|p| p.contains(needle)),
                "expected one saying {needle:?} in {found:?}"
            ),
        }
    }

    #[rstest]
    #[case::in_sync(
        "\"dbt.run\" = { via = \"a lens\", sent_in = \"f\" }",
        "`dbt.run`",
        None
    )]
    #[case::not_in_the_readme("\"dbt.run\" = { via = \"a lens\" }", "", Some("does not name"))]
    #[case::not_sent_there(
        "\"dbt.run\" = { via = \"a lens\", sent_in = \"g\" }",
        "`dbt.run`",
        Some("does not name it")
    )]
    #[case::internal("\"dbt.lens\" = { via = \"x\" }", "`dbt.lens`", Some("internal"))]
    #[case::unknown(
        "\"dbt.new\" = { via = \"x\" }",
        "`dbt.new`",
        Some("does not classify")
    )]
    #[case::empty_via("\"dbt.run\" = { via = \" \" }", "`dbt.run`", Some("must say `via`"))]
    fn the_zed_table(#[case] line: &str, #[case] readme: &str, #[case] expect: Option<&str>) {
        let text = format!("{BASE}[zed]\n{line}\n");
        let read = |file: &str| (file == "f").then(|| "command: \"dbt.run\"".to_string());
        let found = zed_problems(&manifest(&text), readme, &read);
        match expect {
            None => assert!(found.is_empty(), "{found:?}"),
            Some(needle) => assert!(
                found.iter().any(|p| p.contains(needle)),
                "expected one saying {needle:?} in {found:?}"
            ),
        }
    }

    #[test]
    fn a_command_without_a_spec_row_fails() {
        let found = problems(
            &manifest(BASE),
            &["dbt.run", "dbt.lens"],
            &[editor("a", &["dbt.run"])],
            "",
        );
        assert!(found.iter().any(|p| p.contains("no row")), "{found:?}");
    }

    #[test]
    fn every_command_has_a_section_in_the_reference() {
        let doc = "# x\n### `dbt.run`\n- Argument\n### `dbt.gone`\n";
        let found = reference_problems(&["dbt.run", "dbt.new"], doc);
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(found[0].contains("`dbt.new` has no section"));
        assert!(found[1].contains("documents `dbt.gone`"));
    }

    #[test]
    fn the_vscode_palette_and_its_contributions_agree() {
        let ext = "[\"cogitator.run\", run], [\"cogitator.extra\", x]";
        let pkg = r#"{"contributes":{"commands":[{"command":"cogitator.run"},{"command":"cogitator.ghost"}]}}"#;
        let found = vscode_palette_problems(ext, pkg);
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(found[0].contains("cogitator.extra") && found[1].contains("cogitator.ghost"));
    }
}
