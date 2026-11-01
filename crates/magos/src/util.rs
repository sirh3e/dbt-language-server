//! Ports of the Go `util/` package (specs/rust-port/03 §7).

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use lexicanum::Dialect;
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

/// Dialect from profiles.yml (03 §7.2 — Q5 behavior kept: a known workspace
/// root checks only `<root>/.dbt/profiles.yml`).
pub fn get_dialect(profile_name: &str, input_dir: &str) -> Dialect {
    let path = if input_dir.is_empty() {
        match std::env::var("DBT_PROFILES_DIR") {
            Ok(dir) if !dir.is_empty() => PathBuf::from(dir).join("profiles.yml"),
            _ => match std::env::var("HOME") {
                Ok(home) => PathBuf::from(home).join(".dbt").join("profiles.yml"),
                Err(_) => return Dialect::default(),
            },
        }
    } else {
        PathBuf::from(input_dir).join(".dbt").join("profiles.yml")
    };

    let Ok(text) = std::fs::read_to_string(&path) else {
        tracing::debug!(?path, "profiles.yml not readable");
        return Dialect::default();
    };
    dialect_from_profiles(&text, profile_name)
}

pub fn dialect_from_profiles(text: &str, profile_name: &str) -> Dialect {
    let Ok(docs) = MarkedYaml::load_from_str(text) else {
        return Dialect::default();
    };
    let Some(root) = docs.first() else {
        return Dialect::default();
    };
    let get = codex::yaml::get;
    let Some(profile) = get(root, profile_name) else {
        tracing::debug!(profile_name, "profile not found in profiles.yml");
        return Dialect::default();
    };
    let Some(target) = get(profile, "target").and_then(codex::yaml::as_str) else {
        return Dialect::default();
    };
    get(profile, "outputs")
        .and_then(|outputs| get(outputs, target))
        .and_then(|output| get(output, "type"))
        .and_then(codex::yaml::as_str)
        .map(Dialect::new)
        .unwrap_or_default()
}

/// Incremental content change splice (03 §2) — byte offsets, with
/// char-boundary guards replacing Go's silent bounds checks (Q7 behavior:
/// out-of-range edits leave the text unchanged).
pub fn apply_incremental_change(text: &str, range: vox::Range, new_text: &str) -> String {
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
