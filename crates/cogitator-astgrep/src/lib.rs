//! ast-grep for dbt: structural search, rewrite and lint rules over
//! tree-sitter-dbt (Jinja over SQL) and tree-sitter-sequel (plain SQL,
//! e.g. dbt's compiled output), with the server's own grammars.
//!
//! To tree-sitter-dbt the SQL between the tags is one opaque `content`
//! node, so a `dbt` pattern matches Jinja structure — `{{ ref($M) }}`,
//! `{{ config($$$ARGS) }}` — and a pattern for SQL structure is written in
//! `sql` and run on compiled SQL.
//!
//! Metavariables are written `$M`, as everywhere in ast-grep. Neither
//! grammar takes `$` in an identifier, so a pattern is parsed with `_` in
//! its place (the expando character); the ast-grep CLI's `sgconfig.yml`
//! says `expandoChar: _` for the same reason.

use std::borrow::Cow;
use std::path::Path;

use ast_grep_config::{from_yaml_string, GlobalRules, RuleCollection, RuleConfig, Severity};
use ast_grep_core::matcher::{Pattern, PatternBuilder, PatternError};
use ast_grep_core::meta_var::MetaVariable;
use ast_grep_core::replacer::Replacer;
use ast_grep_core::tree_sitter::{LanguageExt, StrDoc, TSLanguage};
use ast_grep_core::{Language, NodeMatch};
use serde::{Deserialize, Serialize};

/// The two languages ast-grep reads here, named as rules and the CLI name
/// them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DbtLanguage {
    /// A dbt model, macro or test: Jinja over SQL.
    Dbt,
    /// Plain SQL, e.g. `target/compiled/**`.
    Sql,
}

impl DbtLanguage {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "dbt" => Some(Self::Dbt),
            "sql" => Some(Self::Sql),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Dbt => "dbt",
            Self::Sql => "sql",
        }
    }
}

/// The metavariable character written in a pattern, and the one the
/// grammars see in its place.
pub const META_VAR_CHAR: char = '$';
pub const EXPANDO_CHAR: char = '_';

impl Language for DbtLanguage {
    fn meta_var_char(&self) -> char {
        META_VAR_CHAR
    }

    fn expando_char(&self) -> char {
        EXPANDO_CHAR
    }

    fn pre_process_pattern<'q>(&self, query: &'q str) -> Cow<'q, str> {
        if query.contains(META_VAR_CHAR) {
            Cow::Owned(query.replace(META_VAR_CHAR, &EXPANDO_CHAR.to_string()))
        } else {
            Cow::Borrowed(query)
        }
    }

    fn from_path<P: AsRef<Path>>(path: P) -> Option<Self> {
        language_of(path.as_ref())
    }

    fn kind_to_id(&self, kind: &str) -> u16 {
        self.get_ts_language().id_for_node_kind(kind, true)
    }

    fn field_to_id(&self, field: &str) -> Option<u16> {
        self.get_ts_language()
            .field_id_for_name(field)
            .map(|f| f.get())
    }

    fn build_pattern(&self, builder: &PatternBuilder) -> Result<Pattern, PatternError> {
        builder.build(|src| StrDoc::try_new(src, *self))
    }
}

impl LanguageExt for DbtLanguage {
    fn get_ts_language(&self) -> TSLanguage {
        match self {
            Self::Dbt => tree_sitter_dbt::LANGUAGE.into(),
            Self::Sql => tree_sitter_sequel::LANGUAGE.into(),
        }
    }
}

/// Which language a file is read as: compiled SQL (`target/compiled`,
/// `target/run`) is plain SQL, any other `.sql` file dbt.
pub fn language_of(path: &Path) -> Option<DbtLanguage> {
    if path.extension().and_then(|e| e.to_str()) != Some("sql") {
        return None;
    }
    let parts: Vec<_> = path
        .components()
        .map(|c| c.as_os_str().to_owned())
        .collect();
    let compiled = parts
        .windows(2)
        .any(|w| w[0] == "target" && (w[1] == "compiled" || w[1] == "run"));
    Some(if compiled {
        DbtLanguage::Sql
    } else {
        DbtLanguage::Dbt
    })
}

/// A position in LSP terms: 0-based line, UTF-16 code units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Point {
    pub line: u32,
    pub character: u32,
}

/// One match in a text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    pub start: Point,
    pub end: Point,
    /// Byte range in the text, for edits.
    pub bytes: std::ops::Range<usize>,
    pub text: String,
    /// The metavariables and what they matched, by name.
    pub env: Vec<(String, String)>,
    /// What a rewrite turns the match into.
    pub replacement: Option<String>,
}

/// Byte offset → LSP position.
struct Lines<'a> {
    text: &'a str,
    starts: Vec<usize>,
}

impl<'a> Lines<'a> {
    fn new(text: &'a str) -> Self {
        let mut starts = vec![0];
        starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
        Self { text, starts }
    }

    fn point(&self, byte: usize) -> Point {
        let line = self.starts.partition_point(|&s| s <= byte) - 1;
        let prefix = self.text.get(self.starts[line]..byte).unwrap_or("");
        Point {
            line: line as u32,
            character: prefix.encode_utf16().count() as u32,
        }
    }
}

/// Why a pattern could not be used, in words for a person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrepError(pub String);

impl std::fmt::Display for GrepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for GrepError {}

/// A compiled pattern, kept for many texts.
pub struct Search {
    language: DbtLanguage,
    pattern: Pattern,
    rewrite: Option<String>,
}

impl Search {
    pub fn new(
        pattern: &str,
        language: DbtLanguage,
        rewrite: Option<&str>,
    ) -> Result<Self, GrepError> {
        let pattern = Pattern::try_new(pattern, language).map_err(|e| {
            GrepError(format!(
                "the pattern does not parse as {}: {e}",
                language.name()
            ))
        })?;
        Ok(Self {
            language,
            pattern,
            rewrite: rewrite.map(str::to_string),
        })
    }

    pub fn language(&self) -> DbtLanguage {
        self.language
    }

    /// Every match in `text`, at most `max`.
    pub fn matches(&self, text: &str, max: usize) -> Vec<Match> {
        let grep = self.language.ast_grep(text);
        let lines = Lines::new(text);
        let rewrite = self.rewrite.as_deref();
        grep.root()
            .find_all(&self.pattern)
            .take(max)
            .map(|nm| to_match(&nm, &lines, rewrite))
            .collect()
    }
}

fn to_match(
    nm: &NodeMatch<'_, StrDoc<DbtLanguage>>,
    lines: &Lines,
    rewrite: Option<&str>,
) -> Match {
    let range = nm.range();
    let env = nm.get_env();
    let mut vars: Vec<(String, String)> = env
        .get_matched_variables()
        .filter_map(|v| match v {
            MetaVariable::Capture(name, _) => {
                let text = env.get_match(&name)?.text().to_string();
                Some((name, text))
            }
            MetaVariable::MultiCapture(name) => {
                let nodes = env.get_multiple_matches(&name);
                let (first, last) = (nodes.first()?, nodes.last()?);
                let text = lines
                    .text
                    .get(first.range().start..last.range().end)?
                    .to_string();
                Some((name, text))
            }
            _ => None,
        })
        .collect();
    vars.sort();
    vars.dedup();
    Match {
        start: lines.point(range.start),
        end: lines.point(range.end),
        bytes: range,
        text: nm.text().to_string(),
        env: vars,
        replacement: rewrite
            .map(|r| String::from_utf8_lossy(&r.generate_replacement(nm)).into_owned()),
    }
}

/// How bad a rule's finding is, as the rule says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

/// A finding of a project rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub rule: String,
    pub severity: RuleSeverity,
    pub message: String,
    pub note: Option<String>,
    pub at: Match,
    /// The rule's `fix`, applied to this match.
    pub fix: Option<String>,
}

/// The project's ast-grep rules (`sgconfig.yml`'s `ruleDirs`) for dbt and
/// SQL; rules of other languages are the CLI's.
pub struct Rules {
    collection: RuleCollection<DbtLanguage>,
    ids: Vec<String>,
    /// What could not be read, a line each: the file and why.
    pub errors: Vec<String>,
    /// Rules of a language this server does not read.
    pub skipped: usize,
}

impl Default for Rules {
    fn default() -> Self {
        Self::from_configs(Vec::new(), Vec::new(), 0)
    }
}

impl Rules {
    fn from_configs(
        configs: Vec<RuleConfig<DbtLanguage>>,
        mut errors: Vec<String>,
        skipped: usize,
    ) -> Self {
        let mut ids: Vec<String> = configs.iter().map(|c| c.id.clone()).collect();
        ids.sort();
        let collection = RuleCollection::try_new(configs).unwrap_or_else(|e| {
            errors.push(format!("a rule's files/ignores glob: {e}"));
            RuleCollection::try_new(Vec::new()).expect("no rules, no globs")
        });
        Self {
            collection,
            ids,
            errors,
            skipped,
        }
    }

    /// Rules from YAML files, each `(name, text)`.
    pub fn from_yaml<'a>(files: impl IntoIterator<Item = (String, &'a str)>) -> Self {
        let globals = GlobalRules::default();
        let (mut configs, mut errors, mut skipped) = (Vec::new(), Vec::new(), 0);
        for (name, yaml) in files {
            for doc in split_documents(yaml) {
                let language = doc
                    .lines()
                    .find_map(|l| l.strip_prefix("language:"))
                    .map(|l| l.trim().trim_matches(['"', '\'']).to_ascii_lowercase());
                if language.as_deref().and_then(DbtLanguage::parse).is_none() {
                    skipped += 1;
                    continue;
                }
                match from_yaml_string::<DbtLanguage>(&doc, &globals) {
                    Ok(rules) => configs.extend(rules),
                    Err(e) => errors.push(format!("{name}: {e}")),
                }
            }
        }
        Self::from_configs(configs, errors, skipped)
    }

    /// The rules of `<root>/sgconfig.yml`'s `ruleDirs`. A project without
    /// the file has none.
    pub fn load(root: &Path) -> Self {
        let (paths, mut errors) = rule_files(root);
        let mut files = Vec::new();
        for path in paths.iter().skip(1) {
            match std::fs::read_to_string(path) {
                Ok(text) => files.push((path.display().to_string(), text)),
                Err(e) => errors.push(format!("{}: {e}", path.display())),
            }
        }
        let mut rules = Self::from_yaml(files.iter().map(|(n, t)| (n.clone(), t.as_str())));
        errors.append(&mut rules.errors);
        rules.errors = errors;
        rules
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// The rules' ids, sorted.
    pub fn ids(&self) -> &[String] {
        &self.ids
    }

    /// Every rule that applies to `path` (by its language and the rule's
    /// `files`/`ignores`), run over `text`.
    pub fn check(&self, path: &Path, text: &str) -> Vec<Finding> {
        let Some(language) = language_of(path) else {
            return Vec::new();
        };
        let grep = language.ast_grep(text);
        let lines = Lines::new(text);
        let mut out = Vec::new();
        for rule in self.collection.get_rule_from_lang(path, language) {
            let severity = match rule.severity {
                Severity::Off => continue,
                Severity::Error => RuleSeverity::Error,
                Severity::Warning => RuleSeverity::Warning,
                Severity::Info => RuleSeverity::Info,
                Severity::Hint => RuleSeverity::Hint,
            };
            for nm in grep.root().find_all(&rule.matcher) {
                let fix = rule
                    .fixer
                    .first()
                    .map(|f| String::from_utf8_lossy(&f.generate_replacement(&nm)).into_owned());
                out.push(Finding {
                    rule: rule.id.clone(),
                    severity,
                    message: rule.get_message(&nm),
                    note: rule.note.clone(),
                    at: to_match(&nm, &lines, None),
                    fix,
                });
            }
        }
        out.sort_by(|a, b| (a.at.bytes.start, &a.rule).cmp(&(b.at.bytes.start, &b.rule)));
        out
    }
}

/// The files the project's rules come from: `<root>/sgconfig.yml` first,
/// then every `.yml`/`.yaml` of its `ruleDirs`, sorted; and the rule
/// directories that are missing. No sgconfig.yml: no files. A caller
/// watches these to know when to load the rules again.
pub fn rule_files(root: &Path) -> (Vec<std::path::PathBuf>, Vec<String>) {
    let config = root.join("sgconfig.yml");
    let Ok(text) = std::fs::read_to_string(&config) else {
        return (Vec::new(), Vec::new());
    };
    let (mut files, mut errors) = (vec![config], Vec::new());
    for dir in rule_dirs(&text) {
        let dir = root.join(dir);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            errors.push(format!("{}: no such rule directory", dir.display()));
            continue;
        };
        let mut paths: Vec<_> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| matches!(p.extension().and_then(|e| e.to_str()), Some("yml" | "yaml")))
            .collect();
        paths.sort();
        files.extend(paths);
    }
    (files, errors)
}

/// `ruleDirs:` of an sgconfig.yml, as a block list or a flow list.
fn rule_dirs(config: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut lines = config.lines().peekable();
    while let Some(line) = lines.next() {
        let Some(rest) = line.trim_start().strip_prefix("ruleDirs:") else {
            continue;
        };
        let rest = rest.trim();
        if let Some(flow) = rest.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
            out.extend(
                flow.split(',')
                    .map(|d| unquote(d.trim()))
                    .filter(|d| !d.is_empty()),
            );
            continue;
        }
        while let Some(next) = lines.peek() {
            let item = next.trim_start();
            if let Some(dir) = item.strip_prefix("- ") {
                out.push(unquote(dir.trim()));
                lines.next();
            } else if item.is_empty() || item.starts_with('#') {
                lines.next();
            } else {
                break;
            }
        }
    }
    out
}

fn unquote(s: &str) -> String {
    s.trim_matches(['"', '\'']).to_string()
}

/// The documents of a multi-document YAML file.
fn split_documents(yaml: &str) -> Vec<String> {
    let mut docs = vec![String::new()];
    for line in yaml.lines() {
        if line.trim_end() == "---" {
            docs.push(String::new());
        } else if let Some(doc) = docs.last_mut() {
            doc.push_str(line);
            doc.push('\n');
        }
    }
    docs.into_iter().filter(|d| !d.trim().is_empty()).collect()
}

#[cfg(test)]
mod tests;
