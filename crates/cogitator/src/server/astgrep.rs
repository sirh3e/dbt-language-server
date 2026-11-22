//! ast-grep in the server (`cogitator-astgrep`): `dbt.astGrep`, the
//! structural search (and rewrite) over the project's dbt files or its
//! compiled SQL, and the project's ast-grep rules (`sgconfig.yml`) as
//! diagnostics with their fixes as quickfixes.

use std::path::{Path, PathBuf};

use cogitator_analysis::context::DbtContext;
use cogitator_astgrep::{DbtLanguage, Finding, Match, RuleSeverity, Rules, Search};
use cogitator_lsp::{
    error_codes, ApplyWorkspaceEditParams, CodeAction, Diagnostic, DocumentChange, Position, Range,
    TextDocumentEdit, TextEdit, WorkspaceEdit,
};
use serde_json::{json, Map, Value};

use crate::server::reply::{Effect, Reply};
use crate::server::snapshot::Snapshot;

/// The diagnostics' and quickfixes' `source`.
pub const SOURCE: &str = "ast-grep";
/// Matches one `dbt.astGrep` answer carries.
pub const MAX_MATCHES: usize = 500;
/// Files larger than this are not searched (a generated model, a seed).
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;

fn range_of(m: &Match) -> Range {
    Range {
        start: Position {
            line: m.start.line,
            character: m.start.character,
        },
        end: Position {
            line: m.end.line,
            character: m.end.character,
        },
    }
}

fn severity(s: RuleSeverity) -> i32 {
    match s {
        RuleSeverity::Error => 1,
        RuleSeverity::Warning => 2,
        RuleSeverity::Info => 3,
        RuleSeverity::Hint => 4,
    }
}

fn diagnostic(f: &Finding) -> Diagnostic {
    let message = match &f.note {
        Some(note) => format!("{}\n{}", f.message, note.trim_end()),
        None => f.message.clone(),
    };
    Diagnostic::plain(
        range_of(&f.at),
        message,
        severity(f.severity),
        f.rule.clone(),
        SOURCE,
    )
}

fn path_of(uri: &str) -> Option<PathBuf> {
    cogitator_lsp::uri_to_fs_path(uri).map(PathBuf::from)
}

/// The project rules' findings in an open document.
pub fn diagnostics(rules: &Rules, uri: &str, text: &str) -> Vec<Diagnostic> {
    if rules.is_empty() {
        return Vec::new();
    }
    let Some(path) = path_of(uri) else {
        return Vec::new();
    };
    rules.check(&path, text).iter().map(diagnostic).collect()
}

/// A quickfix for every finding with a `fix` that touches `range`.
pub fn actions(rules: &Rules, uri: &str, text: &str, range: Range) -> Vec<CodeAction> {
    if rules.is_empty() {
        return Vec::new();
    }
    let Some(path) = path_of(uri) else {
        return Vec::new();
    };
    let touches = |r: Range| {
        (r.start.line, r.start.character) <= (range.end.line, range.end.character)
            && (range.start.line, range.start.character) <= (r.end.line, r.end.character)
    };
    rules
        .check(&path, text)
        .iter()
        .filter(|f| touches(range_of(&f.at)))
        .filter_map(|f| {
            let fix = f.fix.clone()?;
            let edit = WorkspaceEdit {
                document_changes: vec![DocumentChange::Edit(TextDocumentEdit::new(
                    uri.to_string(),
                    vec![TextEdit {
                        range: range_of(&f.at),
                        new_text: fix,
                    }],
                ))],
            };
            Some(CodeAction::quickfix(
                format!("Fix: {} ({})", f.message, f.rule),
                diagnostic(f),
                edit,
            ))
        })
        .collect()
}

/// The `.sql` files below `dir`, sorted; hidden directories left out.
fn sql_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        let hidden = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with('.'));
        if hidden {
            continue;
        }
        if path.is_dir() {
            sql_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("sql") {
            out.push(path);
        }
    }
}

/// The files a project-wide search reads: the dbt files of the project's
/// model, macro, snapshot, test and analysis paths, or for `sql` what dbt
/// compiled (`target/compiled`).
fn project_files(ctx: &DbtContext, language: DbtLanguage) -> Vec<PathBuf> {
    let Some(root) = ctx.project_root.as_deref() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    match language {
        DbtLanguage::Dbt => {
            let p = &ctx.project;
            for dir in p
                .model_paths
                .iter()
                .chain(&p.macro_paths)
                .chain(&p.snapshot_paths)
                .chain(&p.test_paths)
                .chain(&p.analysis_paths)
            {
                sql_files(&root.join(dir), &mut out);
            }
        }
        DbtLanguage::Sql => sql_files(&root.join("target").join("compiled"), &mut out),
    }
    out.sort();
    out.dedup();
    out
}

fn match_json(uri: &str, m: &Match) -> Value {
    let env: Map<String, Value> = m.env.iter().map(|(k, v)| (k.clone(), json!(v))).collect();
    json!({
        "uri": uri,
        "range": range_of(m),
        "text": m.text,
        "env": env,
        "replacement": m.replacement,
    })
}

fn refuse(what: impl Into<String>, fix: &str) -> Reply {
    Reply::error(
        error_codes::INVALID_PARAMS,
        cogitator_error::guidance::Guidance::new(what.into())
            .fix(fix)
            .to_message(),
    )
}

/// `dbt.astGrep {pattern, language?, rewrite?, uri?, apply?}`.
///
/// `uri` searches that document (its open text) alone, else the project's
/// files of the language: dbt files for `dbt` (the default), compiled SQL
/// for `sql`. With `apply: true` and a `rewrite`, every match is replaced
/// through `workspace/applyEdit`.
pub fn search_reply(snap: &Snapshot, arg: &Map<String, Value>, uri: &str) -> Reply {
    let text_of = |k: &str| {
        arg.get(k)
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
    };
    let Some(pattern) = text_of("pattern") else {
        return refuse(
            "dbt.astGrep needs a pattern.",
            "pass {\"pattern\": \"{{ ref($M) }}\"} — ast-grep's syntax, $M a metavariable.",
        );
    };
    let language = match text_of("language") {
        None => match path_of(uri).and_then(|p| cogitator_astgrep::language_of(&p)) {
            Some(l) if !uri.is_empty() => l,
            _ => DbtLanguage::Dbt,
        },
        Some(name) => match DbtLanguage::parse(name) {
            Some(l) => l,
            None => {
                return refuse(
                    format!("dbt.astGrep does not know the language '{name}'."),
                    "pass \"dbt\" (Jinja over SQL) or \"sql\" (compiled SQL).",
                )
            }
        },
    };
    let rewrite = text_of("rewrite");
    let search = match Search::new(pattern, language, rewrite) {
        Ok(s) => s,
        Err(e) => {
            return refuse(
                e.0,
                "a dbt pattern is Jinja ({{ ref($M) }}); a sql pattern is a whole statement \
                 (select $$$C from $T) — use a rule with context/selector for smaller pieces.",
            )
        }
    };

    // one document, or the project
    let mut files: Vec<(String, String)> = Vec::new();
    if !uri.is_empty() {
        let text = match snap.doc.as_deref() {
            Some(doc) => doc.text.to_string(),
            None => match path_of(uri).and_then(|p| std::fs::read_to_string(p).ok()) {
                Some(t) => t,
                None => return refuse(format!("cannot read {uri}"), "pass an existing file."),
            },
        };
        files.push((uri.to_string(), text));
    } else {
        for path in project_files(&snap.ctx, language) {
            if std::fs::metadata(&path)
                .map(|m| m.len() > MAX_FILE_BYTES)
                .unwrap_or(true)
            {
                continue;
            }
            if let Ok(text) = std::fs::read_to_string(&path) {
                files.push((cogitator_lsp::fs_path_to_uri(&path), text));
            }
        }
    }

    let mut matches = Vec::new();
    let mut edits = Vec::new();
    let mut truncated = false;
    for (file_uri, text) in &files {
        let left = MAX_MATCHES - matches.len();
        let found = search.matches(text, left + 1);
        if found.len() > left {
            truncated = true;
        }
        let mut file_edits = Vec::new();
        for m in found.iter().take(left) {
            matches.push(match_json(file_uri, m));
            if let Some(new_text) = &m.replacement {
                file_edits.push(TextEdit {
                    range: range_of(m),
                    new_text: new_text.clone(),
                });
            }
        }
        if !file_edits.is_empty() {
            edits.push(DocumentChange::Edit(TextDocumentEdit::new(
                file_uri.clone(),
                file_edits,
            )));
        }
        if truncated {
            break;
        }
    }

    let count = matches.len();
    let mut reply = Reply::result(&json!({
        "language": language.name(),
        "files": files.len(),
        "matches": matches,
        "truncated": truncated,
    }));
    let apply = arg.get("apply").and_then(|v| v.as_bool()) == Some(true);
    if apply && rewrite.is_some() && !edits.is_empty() {
        reply = reply.with(Effect::ApplyEdit(ApplyWorkspaceEditParams {
            label: format!("ast-grep: rewrite {count} matches"),
            edit: WorkspaceEdit {
                document_changes: edits,
            },
        }));
    }
    reply
}
