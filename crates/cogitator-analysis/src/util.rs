//! Ports of the Go `util/` package (specs/rust-port/03 §7).

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use cogitator_dialects::Dialect;
use cogitator_parser::jinja_spans::{blanked, NOT_CODE};
use regex::{Captures, Regex};
use saphyr::{LoadableYamlNode, MarkedYaml};

/// Project root discovery (03 §7.1): ascending to `/`, then BFS descending.
pub fn get_project_root(file_name: &str, start: &Path) -> Option<PathBuf> {
    find_ascending(file_name, start).or_else(|| find_descending(file_name, start))
}

fn dir_contains(dir: &Path, file_name: &str) -> bool {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .any(|e| e.file_name().to_string_lossy() == file_name)
        })
        .unwrap_or(false)
}

fn find_ascending(file_name: &str, start: &Path) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        if !dir.is_dir() {
            return None; // unreadable → Go errored out of the ascent
        }
        if dir_contains(&dir, file_name) {
            return Some(dir);
        }
        if !dir.pop() || dir.as_os_str().is_empty() {
            return None;
        }
    }
}

fn find_descending(file_name: &str, start: &Path) -> Option<PathBuf> {
    let mut queue = VecDeque::from([start.to_path_buf()]);
    while let Some(dir) = queue.pop_front() {
        let entries = std::fs::read_dir(&dir).ok()?; // Go aborted on read errors
        for entry in entries.flatten() {
            let path = entry.path();
            if entry.file_name().to_string_lossy() == file_name {
                return Some(dir);
            }
            if path.is_dir() {
                queue.push_back(path);
            }
        }
    }
    None
}

/// `{{ env_var("NAME", "default") }}` resolution (03 §7.3) — exact Go regex.
static ENV_VAR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"\{\{\s*env_var\(\s*('|")([^'"]+)('|")\s*(?:,\s*('|")([^'"]*?)('|"))?\s*\)\s*\}\}"#,
    )
    .unwrap()
});

/// `{{ config( … ) }}` spans (non-greedy; good enough for tag extraction).
static CONFIG_SPAN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{\{\s*config\s*\(([\s\S]*?)\)\s*\}\}").unwrap());
/// `tags = [...]` or `tags = 'x'` inside a config span.
static CONFIG_TAGS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"tags\s*=\s*(?:\[([^\]]*)\]|'([^']*)'|"([^"]*)")"#).unwrap());
static QUOTED_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"'([^']*)'|"([^"]*)""#).unwrap());

/// Tags declared in SQL via `{{ config(tags=…) }}` (string or list).
pub fn config_tags(text: &str) -> Vec<String> {
    let mut tags = Vec::new();
    let live = blanked(text, NOT_CODE);
    for span in CONFIG_SPAN_RE.captures_iter(&live) {
        for caps in CONFIG_TAGS_RE.captures_iter(&span[1]) {
            if let Some(list) = caps.get(1) {
                for item in QUOTED_RE.captures_iter(list.as_str()) {
                    let tag = item.get(1).or_else(|| item.get(2)).map(|m| m.as_str());
                    tags.extend(tag.map(str::to_string));
                }
            } else if let Some(single) = caps.get(2).or_else(|| caps.get(3)) {
                tags.push(single.as_str().to_string());
            }
        }
    }
    tags.sort();
    tags.dedup();
    tags
}

/// `env_var()` references in `input` that resolve to nothing: no value in
/// the environment and no inline default (resolve_env_vars emits the
/// DBT_ENV_DEFAULT_ placeholder for exactly these).
pub fn unresolved_env_vars(input: &str) -> Vec<String> {
    ENV_VAR_RE
        .captures_iter(input)
        .filter_map(|caps| {
            let name = &caps[2];
            (std::env::var(name).is_err() && caps.get(4).is_none()).then(|| name.to_string())
        })
        .collect()
}

pub fn resolve_env_vars(input: &str) -> String {
    ENV_VAR_RE
        .replace_all(input, |caps: &Captures| {
            let name = &caps[2];
            if let Ok(value) = std::env::var(name) {
                return value;
            }
            if caps.get(4).is_some() {
                return caps.get(5).map(|m| m.as_str()).unwrap_or("").to_string();
            }
            let fallback = format!("DBT_ENV_DEFAULT_{name}");
            tracing::debug!(%name, %fallback, "env var not set, using fallback");
            fallback
        })
        .into_owned()
}

/// Recursive walk collecting files with extension `ext` (03 §7.4), sorted so
/// "last wins" collisions resolve like Go's lexical `filepath.Walk`.
pub fn walk_filepath(path: &Path, ext: &str) -> Vec<PathBuf> {
    walkdir::WalkDir::new(path)
        .sort_by_file_name()
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|x| format!(".{}", x.to_string_lossy()) == ext)
        })
        .map(|e| e.into_path())
        .collect()
}

/// `{ file stem → full path }` over several roots/extensions (03 §7.4).
pub fn create_file_name_map(
    exts: &[&str],
    root: &Path,
    paths: &[String],
) -> HashMap<String, PathBuf> {
    let mut map = HashMap::new();
    for p in paths {
        let dir = root.join(p);
        if std::fs::read_dir(&dir).is_err() {
            continue;
        }
        for ext in exts {
            for file in walk_filepath(&dir, ext) {
                if let Some(stem) = file.file_stem().map(|s| s.to_string_lossy().into_owned()) {
                    map.insert(stem, file);
                }
            }
        }
    }
    map
}

/// Byte offset → 0-based (line, column) (03 §7.5).
pub fn get_line_and_column(input: &str, idx: usize) -> (u32, u32) {
    let upto = &input.as_bytes()[..idx.min(input.len())];
    let line = upto.iter().filter(|&&b| b == b'\n').count() as u32;
    let last_nl = upto
        .iter()
        .rposition(|&b| b == b'\n')
        .map(|p| p + 1)
        .unwrap_or(0);
    (line, (idx.min(input.len()) - last_nl) as u32)
}

/// Home directory: `$HOME`, falling back to `%USERPROFILE%` (on Windows,
/// cmd.exe and service contexts often set only the latter).
pub fn home_dir() -> Option<PathBuf> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
}

/// `~` / `~/x` → the home directory (editor configs often pass these).
fn expand_home(dir: &str) -> PathBuf {
    if dir == "~" {
        return home_dir().unwrap_or_else(|| PathBuf::from(dir));
    }
    if let Some(rest) = dir.strip_prefix("~/").or_else(|| dir.strip_prefix("~\\")) {
        if let Some(home) = home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(dir)
}

/// Candidate `profiles.yml` files in precedence order, dbt-style:
/// 1. an explicit override (`--profiles-dir` / `initializationOptions.profilesDir`)
/// 2. `$DBT_PROFILES_DIR`
/// 3. `<workspace root>/profiles.yml` (dbt's project-local lookup)
/// 4. `<workspace root>/.dbt/profiles.yml` (Q5 behavior, kept)
/// 5. `~/.dbt/profiles.yml`
pub fn profiles_candidates(override_dir: &str, workspace_root: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if !override_dir.is_empty() {
        out.push(expand_home(override_dir).join("profiles.yml"));
    }
    if let Ok(dir) = std::env::var("DBT_PROFILES_DIR") {
        if !dir.is_empty() {
            out.push(expand_home(&dir).join("profiles.yml"));
        }
    }
    if !workspace_root.is_empty() {
        let root = PathBuf::from(workspace_root);
        out.push(root.join("profiles.yml"));
        out.push(root.join(".dbt").join("profiles.yml"));
    }
    if let Some(home) = home_dir() {
        out.push(home.join(".dbt").join("profiles.yml"));
    }
    out
}

/// Dialect from profiles.yml (03 §7.2, extended): the first candidate from
/// [`profiles_candidates`] that parses and resolves this profile's target
/// wins; an unreadable or non-matching file falls through to the next.
/// `target` (dbt.setTarget / --target) replaces the profile's default.
pub fn get_dialect(
    profile_name: &str,
    workspace_root: &str,
    profiles_dir: &str,
    target: Option<&str>,
) -> Dialect {
    for path in profiles_candidates(profiles_dir, workspace_root) {
        let Ok(text) = std::fs::read_to_string(&path) else {
            tracing::debug!(?path, "profiles.yml candidate not readable");
            continue;
        };
        let dialect = dialect_from_profiles(&text, profile_name, target);
        if !dialect.as_str().is_empty() {
            tracing::debug!(?path, dialect = dialect.as_str(), "dialect resolved");
            return dialect;
        }
        tracing::debug!(?path, profile_name, "candidate has no matching target");
    }
    Dialect::default()
}

pub fn dialect_from_profiles(text: &str, profile_name: &str, target: Option<&str>) -> Dialect {
    let Ok(docs) = MarkedYaml::load_from_str(text) else {
        return Dialect::default();
    };
    let Some(root) = docs.first() else {
        return Dialect::default();
    };
    let get = cogitator_schemas::yaml::get;
    let Some(profile) = get(root, profile_name) else {
        tracing::debug!(profile_name, "profile not found in profiles.yml");
        return Dialect::default();
    };
    let target = match target.filter(|t| !t.is_empty()) {
        Some(t) => t,
        None => match get(profile, "target").and_then(cogitator_schemas::yaml::as_str) {
            Some(t) => t,
            None => return Dialect::default(),
        },
    };
    get(profile, "outputs")
        .and_then(|outputs| get(outputs, target))
        .and_then(|output| get(output, "type"))
        .and_then(cogitator_schemas::yaml::as_str)
        .map(Dialect::new)
        .unwrap_or_default()
}

/// Incremental content change splice (03 §2) — byte offsets, with
/// char-boundary guards replacing Go's silent bounds checks (Q7 behavior:
/// out-of-range edits leave the text unchanged).
pub fn apply_incremental_change(text: &str, range: cogitator_lsp::Range, new_text: &str) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let (start_line, start_char) = (range.start.line as usize, range.start.character as usize);
    let (end_line, end_char) = (range.end.line as usize, range.end.character as usize);
    if start_line >= lines.len() || end_line >= lines.len() {
        return text.to_string();
    }

    let mut result = String::with_capacity(text.len() + new_text.len());
    for line in &lines[..start_line] {
        result.push_str(line);
        result.push('\n');
    }

    if start_line == end_line {
        let line = lines[start_line];
        if start_char <= line.len()
            && end_char <= line.len()
            && line.is_char_boundary(start_char)
            && line.is_char_boundary(end_char)
        {
            result.push_str(&line[..start_char]);
            result.push_str(new_text);
            result.push_str(&line[end_char..]);
        } else {
            result.push_str(line);
        }
    } else {
        let start_text = lines[start_line];
        let head = if start_char <= start_text.len() && start_text.is_char_boundary(start_char) {
            &start_text[..start_char]
        } else {
            start_text
        };
        let end_text = lines[end_line];
        let tail = if end_char <= end_text.len() && end_text.is_char_boundary(end_char) {
            &end_text[end_char..]
        } else {
            ""
        };
        result.push_str(head);
        result.push_str(new_text);
        result.push_str(tail);
    }

    for line in &lines[end_line + 1..] {
        result.push('\n');
        result.push_str(line);
    }
    result
}

/// Byte offset of a (line, byte-column) position; None when the
/// position does not sit on a char boundary inside the text.
fn byte_offset(text: &str, line: u32, character: u32) -> Option<usize> {
    let mut offset = 0usize;
    for (i, l) in text.split('\n').enumerate() {
        if i as u32 == line {
            let character = character as usize;
            return (character <= l.len() && l.is_char_boundary(character))
                .then_some(offset + character);
        }
        offset += l.len() + 1;
    }
    None
}

/// The tree-sitter-shaped edit record for one ranged didChange applied
/// to `before` (06 §3.4). None (→ full reparse) when positions do not
/// resolve cleanly.
pub fn edit_for_change(
    before: &str,
    range: cogitator_lsp::Range,
    new_text: &str,
) -> Option<cogitator_parser::AppliedEdit> {
    let start_byte = byte_offset(before, range.start.line, range.start.character)?;
    let old_end_byte = byte_offset(before, range.end.line, range.end.character)?;
    if old_end_byte < start_byte {
        return None;
    }
    let newlines = new_text.matches('\n').count() as u32;
    let last_len = new_text.rsplit('\n').next().unwrap_or("").len() as u32;
    let new_end_point = if newlines == 0 {
        (range.start.line, range.start.character + last_len)
    } else {
        (range.start.line + newlines, last_len)
    };
    Some(cogitator_parser::AppliedEdit {
        start_byte,
        old_end_byte,
        new_end_byte: start_byte + new_text.len(),
        start_point: (range.start.line, range.start.character),
        old_end_point: (range.end.line, range.end.character),
        new_end_point,
    })
}
