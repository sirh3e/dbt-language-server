//! What the menu's finders need beyond [`menu`](super::menu): the matches of
//! `dbt.search` and `dbt.astGrep` as a document of links, and which of the
//! profiles.yml `dbt.profiles` searched exist (file I/O, so it is done in
//! [`finish`](super::finish), not in the pure `menu::choose`).

use std::path::Path;

use serde_json::Value;

/// Characters a match's text shows on its line.
const TEXT: usize = 120;

/// The answer with `existing`: those of its `searched` candidates that are
/// files. Only the dialog's copy of the answer carries it; the command's
/// own result keeps its shape.
pub fn with_existing_profiles(mut answer: Value, is_file: impl Fn(&Path) -> bool) -> Value {
    let existing: Vec<Value> = answer
        .get("searched")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|p| is_file(Path::new(p)))
        .map(|p| Value::String(p.to_string()))
        .collect();
    if let Value::Object(map) = &mut answer {
        map.insert("existing".into(), Value::Array(existing));
    }
    answer
}

/// A match's text on one line, bounded, as inline code.
fn code(text: &str) -> String {
    let line = text.lines().next().unwrap_or_default().trim();
    let mut shown: String = line.chars().take(TEXT).collect();
    if line.chars().count() > TEXT || text.trim().lines().nth(1).is_some() {
        shown.push('…');
    }
    if shown.is_empty() {
        return String::new();
    }
    if shown.contains('`') {
        format!("`` {shown} ``")
    } else {
        format!("`{shown}`")
    }
}

/// What a match was found by: a search's clause, or ast-grep's
/// metavariables.
fn why(env: Option<&Value>) -> String {
    let Some(Value::Object(env)) = env else {
        return String::new();
    };
    if let Some(clause) = env.get("clause").and_then(Value::as_str) {
        return format!(" — {clause}");
    }
    let vars: Vec<String> = env
        .iter()
        .filter_map(|(k, v)| v.as_str().map(|v| format!("${k} = {v}")))
        .collect();
    if vars.is_empty() {
        String::new()
    } else {
        format!(" — {}", vars.join(", "))
    }
}

/// The matches of `title`'s answer as a markdown document: a heading per
/// file (relative to `root` when it is under it), a line per match linking
/// to its line, and what was not searched.
pub fn markdown(title: &str, answer: &Value, root: Option<&Path>) -> String {
    let mut out = format!("# {title}\n\n");
    if let Some(query) = answer.get("query").and_then(Value::as_str) {
        out.push_str(&format!("Query: `{query}`\n\n"));
    }
    let matches: &[Value] = answer
        .get("matches")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let searched = match (answer.get("searched"), answer.get("files")) {
        (Some(n), _) if n.is_u64() => format!(" in the {n} models searched"),
        (_, Some(n)) if n.is_u64() => format!(" in {n} files"),
        _ => String::new(),
    };
    let noun = if matches.len() == 1 {
        "match"
    } else {
        "matches"
    };
    out.push_str(&format!("{} {noun}{searched}", matches.len()));
    if answer.get("truncated").and_then(Value::as_bool) == Some(true) {
        out.push_str(" (cut short: there are more)");
    }
    out.push_str(".\n\n");

    let mut file: Option<&str> = None;
    for m in matches {
        let Some(uri) = m.get("uri").and_then(Value::as_str) else {
            continue;
        };
        if file != Some(uri) {
            if file.is_some() {
                out.push('\n');
            }
            file = Some(uri);
            out.push_str(&format!("## {}\n\n", shown_path(uri, root)));
        }
        let line = m
            .pointer("/range/start/line")
            .and_then(Value::as_u64)
            .map_or(1, |l| l + 1);
        let text = m.get("text").and_then(Value::as_str).unwrap_or_default();
        out.push_str(&format!("- [line {line}]({uri}#L{line})"));
        let text = code(text);
        if !text.is_empty() {
            out.push(' ');
            out.push_str(&text);
        }
        out.push_str(&why(m.get("env")));
        out.push('\n');
    }
    if file.is_some() {
        out.push('\n');
    }

    let skipped: &[Value] = answer
        .get("notSearched")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    if !skipped.is_empty() {
        out.push_str("## Not searched\n\n");
        for n in skipped {
            let model = n.get("model").and_then(Value::as_str).unwrap_or("?");
            let why = n.get("why").and_then(Value::as_str).unwrap_or_default();
            out.push_str(&format!("- {model}: {why}\n"));
        }
        out.push('\n');
    }
    while out.ends_with("\n\n") {
        out.pop();
    }
    out
}

fn shown_path(uri: &str, root: Option<&Path>) -> String {
    let Some(path) = cogitator_lsp::uri_to_fs_path(uri) else {
        return uri.to_string();
    };
    let path = Path::new(&path);
    root.and_then(|r| path.strip_prefix(r).ok())
        .unwrap_or(path)
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn every_offered_pattern_is_one_ast_grep_reads() {
        for (title, pattern, language) in super::super::menu::JINJA_PATTERNS {
            let language = cogitator_astgrep::DbtLanguage::parse(language).unwrap();
            let search = cogitator_astgrep::Search::new(pattern, language, None)
                .unwrap_or_else(|e| panic!("{title}: {pattern}: {}", e.0));
            let sample = match title {
                "Every ref()" => "select * from {{ ref('a') }}",
                "Every source()" => "select * from {{ source('s', 't') }}",
                "Every {{ this }}" => "select max(x) from {{ this }}",
                "Every config()" => "{{ config(materialized='table') }}\nselect 1",
                "Every is_incremental() block" => {
                    "select 1\n{% if is_incremental() %}\nwhere x > 1\n{% endif %}"
                }
                "Every ref(var(…))" => "select * from {{ ref(var('m')) }}",
                "select * in the compiled SQL" => "select * from orders",
                other => panic!("no sample for {other}"),
            };
            assert!(
                !search.matches(sample, 5).is_empty(),
                "{title}: {pattern} finds nothing in {sample:?}"
            );
        }
    }

    #[test]
    fn only_the_candidates_on_disk_are_existing() {
        let answer = json!({ "searched": ["/a/profiles.yml", "/b/profiles.yml"], "path": "/b/profiles.yml" });
        let with = with_existing_profiles(answer, |p| p.starts_with("/b"));
        assert_eq!(with["existing"], json!(["/b/profiles.yml"]));
        assert_eq!(with["path"], "/b/profiles.yml");
    }

    #[test]
    fn matches_are_links_by_file_and_what_was_not_searched_is_said() {
        let answer = json!({
            "query": "reads stg_orders",
            "searched": 4,
            "matches": [
                {"uri": "file:///p/models/orders.sql",
                 "range": {"start": {"line": 2, "character": 0}, "end": {"line": 2, "character": 9}},
                 "text": "{{ ref('stg_orders') }}", "env": {"clause": "reads stg_orders", "model": "orders"}},
                {"uri": "file:///p/models/customers.sql",
                 "range": {"start": {"line": 9, "character": 4}, "end": {"line": 9, "character": 9}},
                 "text": "select `a`\nfrom b", "env": {"M": "'x'"}}
            ],
            "notSearched": [{"model": "big", "why": "control blocks"}],
            "truncated": false
        });
        assert_eq!(
            markdown("Who reads it", &answer, Some(Path::new("/p"))),
            "# Who reads it\n\nQuery: `reads stg_orders`\n\n2 matches in the 4 models searched.\n\n\
             ## models/orders.sql\n\n\
             - [line 3](file:///p/models/orders.sql#L3) `{{ ref('stg_orders') }}` — reads stg_orders\n\n\
             ## models/customers.sql\n\n\
             - [line 10](file:///p/models/customers.sql#L10) `` select `a`… `` — $M = 'x'\n\n\
             ## Not searched\n\n- big: control blocks\n"
        );
        assert_eq!(
            markdown(
                "Every ref()",
                &json!({ "files": 3, "matches": [], "truncated": true }),
                None
            ),
            "# Every ref()\n\n0 matches in 3 files (cut short: there are more).\n"
        );
    }
}
