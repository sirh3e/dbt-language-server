//! `dbt.highlight` (specs/highlighting/00-highlighting.md §2): the
//! grammars' highlights of a document, a text, a model's compiled SQL or a
//! graph node's file, with the project's knowledge for YAML (§5), a line
//! range, the client's version echoed, and a cache of whole answers.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use cogitator_analysis::context::DbtContext;
use cogitator_analysis::graph::NodeKind;
use cogitator_lsp::error_codes;
use cogitator_treesitter::{HighlightLanguage, Highlights, Reference, YamlContext, YamlFile};
use serde_json::{json, Map, Value};

use crate::server::derived_cache::{stamp, DerivedCache, HighlightKey};
use crate::server::reply::Reply;
use crate::server::snapshot::Snapshot;

/// Cap on the spans one answer carries.
pub const MAX_SPANS: usize = 50_000;
/// Larger texts are not highlighted (2 MiB, as the compiled SQL's cap).
pub const MAX_BYTES: usize = 2 * 1024 * 1024;

/// What `dbt.highlight` was asked to colour.
#[derive(Debug, Clone)]
pub enum Source {
    /// `{text, language?}`: any text, `dbt` unless it says otherwise.
    Text(String, Option<String>),
    /// `{model?, compiled: true}`: the SQL dbt compiled the model to.
    Compiled(String, Option<String>),
    /// `{node: {kind, name}, compiled?}`: a graph node's file, or its
    /// compiled SQL.
    Node(String, String, bool),
    /// `{uri, language?}`: the open document, else the file on disk.
    Uri(String, Option<String>),
}

/// A `dbt.highlight` request.
#[derive(Debug, Clone)]
pub struct Request {
    pub source: Source,
    /// Lines `start..end` (end exclusive) to answer the spans of.
    pub range: Option<(u32, u32)>,
    /// The client's version, echoed.
    pub version: Option<i64>,
}

impl Request {
    pub fn of(arg: &Map<String, Value>, uri: &str) -> Self {
        let text = |k: &str| arg.get(k).and_then(|v| v.as_str()).map(str::to_string);
        let compiled = arg.get("compiled").and_then(|v| v.as_bool()) == Some(true);
        let source = if let Some(t) = text("text") {
            Source::Text(t, text("language"))
        } else if let Some(node) = arg.get("node").and_then(|n| n.as_object()) {
            let field = |k: &str| {
                node.get(k)
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string()
            };
            Source::Node(field("kind"), field("name"), compiled)
        } else if compiled {
            Source::Compiled(uri.to_string(), text("model"))
        } else {
            Source::Uri(uri.to_string(), text("language"))
        };
        let line = |v: &Value| v.get("line").and_then(|l| l.as_u64()).map(|l| l as u32);
        let range = arg.get("range").and_then(|r| {
            let start = line(r.get("start")?)?;
            let end = line(r.get("end")?)?;
            Some((start, end.max(start)))
        });
        Self {
            source,
            range,
            version: arg.get("version").and_then(|v| v.as_i64()),
        }
    }
}

/// dbt's config keys: every property of a `*_config(s)` definition in the
/// dbt JSON schemas the server embeds (dbt_project.yml's and the property
/// files'), with and without the `+` of dbt_project.yml.
pub fn config_keys() -> &'static HashSet<String> {
    static KEYS: OnceLock<HashSet<String>> = OnceLock::new();
    KEYS.get_or_init(|| {
        let mut keys = HashSet::new();
        for schema in [
            include_str!("../../../../schemas/dbt/processed/dbt_project.json"),
            include_str!("../../../../schemas/dbt/processed/dbt_yml_files.json"),
        ] {
            let Ok(value) = serde_json::from_str::<Value>(schema) else {
                continue;
            };
            let defs = value
                .get("$defs")
                .or_else(|| value.get("definitions"))
                .and_then(|d| d.as_object());
            for (name, def) in defs.into_iter().flatten() {
                if !(name.ends_with("_configs") || name.ends_with("_config")) {
                    continue;
                }
                if let Some(props) = def.get("properties").and_then(|p| p.as_object()) {
                    for key in props.keys() {
                        keys.insert(key.trim_start_matches('+').to_string());
                    }
                }
            }
        }
        keys
    })
}

/// Does a reference resolve in the project? `uri` is the file's, for the
/// vars it sees.
fn resolves(ctx: &DbtContext, uri: &str, reference: &Reference) -> bool {
    match reference {
        Reference::Model(name) => {
            ctx.model_detail_map.contains_key(name)
                || ctx
                    .graph
                    .node(NodeKind::Model, name)
                    .is_some_and(|n| !n.uri.as_os_str().is_empty())
        }
        Reference::Source(source, _table) => ctx.source_detail_map.contains_key(source),
        Reference::Doc(name) => ctx.doc_detail_map.contains_key(name),
        // a default makes any var() resolve
        Reference::Var(name, defaulted) => *defaulted || ctx.var_for(uri, name).is_some(),
    }
}

/// The file of a graph node, by its kind: a model's or a macro's from the
/// context, the rest from the graph.
pub fn node_file(ctx: &DbtContext, kind: &str, name: &str) -> Option<PathBuf> {
    let found = match kind {
        "model" => ctx.model_detail_map.get(name).map(|d| d.uri.clone()),
        "macro" | "generic_test" => ctx.macro_detail_map.values().find_map(|m| {
            m.get(name)
                .or_else(|| m.get(&format!("test_{name}")))
                .map(|d| d.uri.clone())
        }),
        "source" => ctx.source_detail_map.get(name).map(|d| d.uri.clone()),
        other => {
            let kind = NodeKind::parse(other)?;
            ctx.graph.node(kind, name).map(|n| n.uri.clone())
        }
    };
    found.filter(|p| !p.as_os_str().is_empty())
}

fn refuse(what: impl Into<String>, fix: &str) -> Reply {
    Reply::error(
        error_codes::INVALID_PARAMS,
        cogitator_error::guidance::Guidance::new(what.into())
            .fix(fix)
            .to_message(),
    )
}

fn language_named(
    name: Option<&str>,
    default: HighlightLanguage,
) -> Result<HighlightLanguage, Reply> {
    match name {
        None => Ok(default),
        Some(name) => HighlightLanguage::parse(name).ok_or_else(|| {
            refuse(
                format!("dbt.highlight does not know the language '{name}'."),
                "pass language \"dbt\" (Jinja over SQL), \"sql\" or \"yaml\".",
            )
        }),
    }
}

/// What to highlight: the text, its language, the path it came from (for
/// YAML's file kind) and the cache key, if it has one.
struct Resolved {
    text: Arc<str>,
    language: HighlightLanguage,
    path: Option<String>,
    key: Option<HighlightKey>,
}

fn read(path: &Path) -> Result<String, Reply> {
    std::fs::read_to_string(path).map_err(|e| {
        refuse(
            format!("cannot read {}: {e}", path.display()),
            "pass an existing file.",
        )
    })
}

fn file_key(path: &Path, language: HighlightLanguage) -> Option<HighlightKey> {
    stamp(path).map(|s| HighlightKey::File(path.to_path_buf(), s, language.name()))
}

/// `None` (inside the `Ok`) is a node or model without compiled SQL.
fn resolve(snap: &Snapshot, source: Source) -> Result<Option<Resolved>, Reply> {
    let ctx = &snap.ctx;
    let compiled_of = |file: &Path| -> Option<PathBuf> {
        let root = ctx.project_root.as_deref()?;
        let path = cogitator_analysis::features::compiled_sql_path(root, &ctx.project.name, file);
        path.is_file().then_some(path)
    };
    match source {
        Source::Text(text, language) => Ok(Some(Resolved {
            text: text.into(),
            language: language_named(language.as_deref(), HighlightLanguage::Dbt)?,
            path: None,
            key: None,
        })),
        Source::Compiled(uri, model) => {
            let (compiled, _) = crate::server::compiled::compiled_sql(
                ctx,
                &uri,
                model.as_deref(),
                snap.compiled_cap_bytes,
            );
            Ok(compiled.and_then(|c| c.content).map(|text| Resolved {
                text: text.into(),
                language: HighlightLanguage::Sql,
                path: None,
                key: None,
            }))
        }
        Source::Node(kind, name, compiled) => {
            let Some(file) = node_file(ctx, &kind, &name) else {
                return Err(refuse(
                    format!("No {kind} '{name}' with a file in this project."),
                    "pass {\"node\": {\"kind\": \"model|macro|test|source|…\", \"name\": …}}.",
                ));
            };
            let path = if compiled {
                if matches!(kind.as_str(), "macro" | "generic_test" | "source") {
                    return Ok(None);
                }
                match compiled_of(&file) {
                    Some(p) => p,
                    None => return Ok(None),
                }
            } else {
                file
            };
            let language = if compiled {
                HighlightLanguage::Sql
            } else {
                HighlightLanguage::of_path(&path.to_string_lossy())
            };
            Ok(Some(Resolved {
                text: read(&path)?.into(),
                key: file_key(&path, language),
                path: Some(path.to_string_lossy().into_owned()),
                language,
            }))
        }
        Source::Uri(uri, language) => {
            let path = cogitator_lsp::uri_to_fs_path(&uri);
            let language = language_named(
                language.as_deref(),
                HighlightLanguage::of_path(path.as_deref().unwrap_or(&uri)),
            )?;
            if let Some(doc) = snap.doc.as_deref() {
                return Ok(Some(Resolved {
                    text: doc.text.clone(),
                    language,
                    path: path.clone(),
                    key: Some(HighlightKey::Doc(
                        uri.clone(),
                        doc.generation,
                        language.name(),
                    )),
                }));
            }
            let Some(path) = path else {
                return Err(refuse(
                    "dbt.highlight needs a uri, a text, a node, or compiled: true.",
                    "pass {\"uri\": \"file://…\"} or {\"text\": \"…\"}.",
                ));
            };
            let file = PathBuf::from(&path);
            Ok(Some(Resolved {
                text: read(&file)?.into(),
                key: file_key(&file, language),
                path: Some(path),
                language,
            }))
        }
    }
}

/// Highlight a resolved text with the project's YAML knowledge.
fn compute(ctx: &DbtContext, r: &Resolved) -> Option<Highlights> {
    let keys = config_keys();
    let config_key = |k: &str| keys.contains(k.trim_start_matches('+'));
    let uri = r
        .path
        .as_deref()
        .map(|p| cogitator_lsp::fs_path_to_uri(Path::new(p)))
        .unwrap_or_default();
    let resolver = |reference: &Reference| resolves(ctx, &uri, reference);
    let yaml = YamlContext {
        file: r
            .path
            .as_deref()
            .map(YamlFile::of_path)
            .unwrap_or(YamlFile::Properties),
        config_key: &config_key,
        resolves: &resolver,
    };
    cogitator_treesitter::highlight_with(&r.text, r.language, MAX_SPANS, &yaml)
}

/// The cached highlights of a document as `dbt.highlight` answers them
/// (no range, no version), for the semantic-tokens stream of YAML files
/// (`server/semantic_yaml.rs`): one source of truth for both channels.
pub fn document_highlights(
    snap: &Snapshot,
    derived: &DerivedCache,
    uri: &str,
) -> Option<Arc<Highlights>> {
    let resolved = resolve(snap, Source::Uri(uri.to_string(), None)).ok()??;
    if resolved.text.len() > MAX_BYTES {
        return None;
    }
    match &resolved.key {
        Some(key) => derived.highlights(key.clone(), || compute(&snap.ctx, &resolved)),
        None => compute(&snap.ctx, &resolved).map(Arc::new),
    }
}

/// `dbt.highlight`.
pub fn reply(snap: &Snapshot, derived: &DerivedCache, request: Request) -> Reply {
    let resolved = match resolve(snap, request.source) {
        Ok(Some(r)) => r,
        Ok(None) => return Reply::null(),
        Err(reply) => return reply,
    };
    let language = resolved.language;
    if resolved.text.len() > MAX_BYTES {
        return Reply::result(&json!({
            "language": language.name(), "version": request.version,
            "spans": [], "truncated": true, "layers": [],
        }));
    }
    let found = match &resolved.key {
        Some(key) => derived.highlights(key.clone(), || compute(&snap.ctx, &resolved)),
        None => compute(&snap.ctx, &resolved).map(Arc::new),
    };
    let Some(found) = found else {
        return Reply::error(
            error_codes::INTERNAL_ERROR,
            "the tree-sitter grammars could not be loaded".to_string(),
        );
    };
    let spans = match request.range {
        Some((start, end)) => found.in_lines(start, end),
        None => found.spans.clone(),
    };
    let position =
        |p: cogitator_treesitter::Point| json!({"line": p.line, "character": p.character});
    let spans: Vec<Value> = spans
        .into_iter()
        .map(|s| {
            json!({
                "range": {"start": position(s.start), "end": position(s.end)},
                "capture": s.capture,
                "layer": s.layer.name(),
            })
        })
        .collect();
    let mut answer = json!({
        "language": language.name(),
        "spans": spans,
        "truncated": found.truncated,
        "layers": found.layers.iter().map(|l| l.name()).collect::<Vec<_>>(),
    });
    if let Some(version) = request.version {
        answer["version"] = json!(version);
    }
    Reply::result(&answer)
}
