# 03 — State, dbt Project Analysis & Feature Algorithms

Source files: `analysis/*.go`, `util/*.go` (except fusion/CLI, see 04).
Reference tests: `analysis/state_test.go`, `state_schema_test.go`,
`state_incremental_test.go`, `parse_*_test.go`, `get_completion_items_test.go`,
`util/*_test.go` — all run against `testdata/` (copy the directory unchanged).

## 1. Core state

```rust
struct State {
    documents: HashMap<String, Document>,     // key: uri as sent by client
    dbt_context: DbtContext,
    fusion_enabled: Arc<AtomicBool>,          // Go: bool + RwLock; set by bg thread
    fusion_path: String,                      // value of --fusion flag
    lsp_client_root_path: String,             // from initialize.rootPath
}

struct Document {
    text: String,
    tokens: TokenIndex,                 // 02 §3.4
    def_tokens: HashMap<String, Token>, // CTE name → definition token
}
// Implementation shape: `ParsedDocument` with `text: Arc<str>` and span tokens
// (08 §1.1); this struct states what a document must hold, not the Rust types.

struct DbtContext {
    project_root: String,               // dir containing dbt_project.yml ("" if not found)
    project_yaml: DbtProjectYaml,
    dialect: Dialect,                   // "snowflake" | "duckdb" | "bigquery" | "" | ...
    model_detail_map: HashMap<String, ModelDetails>,
    source_detail_map: HashMap<String, Source>,
    macro_detail_map: HashMap<String /*package*/, HashMap<String, Macro>>,
    variable_detail_map: HashMap<String, Variable>,
}
```

Detail structs:

```rust
struct ModelDetails { uri: String, project_name: String, description: String,
                      schema_uri: String, schema_range: Range }
struct Source       { name: String, description: String, uri: String, range: Range,
                      tables: HashMap<String, SourceTable> }
struct SourceTable  { name: String, description: String, table: String /*=source name*/,
                      uri: String, range: Range }
struct Macro        { name: String, project_name: String, description: String,
                      uri: String, range: Range }
struct Variable     { name: String, value: YamlValue, uri: String, range: Range }
```

All `uri` fields here hold **plain filesystem paths**; the `file://` scheme is
prepended only when building Definition/GoToSchema responses.

### 1.1 Lifecycle operations

- `open_document(uri, text)`: `refresh_dbt_context(lsp_client_root_path)` then
  `parse_document(uri, text)`.
- `save_document(uri)`: `refresh_dbt_context(...)` only.
- `update_document_incremental(uri, changes)`: if uri unknown → no-op. Fold changes
  over the stored text (§2), then `parse_document(uri, new_text)`.
- `parse_document(uri, text)`: run lexer+parser (02) with `dbt_context.dialect`,
  store `Document { text, tokens: index, def_tokens: name_map }`.

### 1.2 `refresh_dbt_context(wd)`

1. `project_root = get_project_root("dbt_project.yml", wd)` (§7.1).
2. `project_yaml = parse_dbt_project_yaml(project_root)` (§3).
3. `dialect = get_dialect(project_yaml.profile.value, wd)` (§7.2).
4. In parallel (3 threads; Go uses goroutines + WaitGroup):
   - `(model_detail_map, source_detail_map) = get_model_details()` (§6)
   - `macro_detail_map = get_macro_details()` (§5)
   - `variable_detail_map = get_project_variables()` (§4)
5. Assign all four maps.

Runs on every didOpen/didSave; it re-walks the whole project each time (no caching,
no file watching). Keep for parity; optimize later behind identical results.

## 2. Incremental text sync (`apply_incremental_change`)

A change with a zero `Range` (all fields 0 — Go compares to the zero struct) is a
full-document replacement. Otherwise, line/char splice:

```
lines = text.split('\n')                       # keeps no terminators
if start.line >= lines.len() || end.line >= lines.len(): return text unchanged
out = lines[..start.line] each + '\n'
if start.line == end.line:
    line = lines[start.line]
    if start.char <= line.len() && end.char <= line.len():
        out += line[..start.char] + change.text + line[end.char..]
    else:
        out += line                            # [Q7] edit silently dropped
else:
    head = if start.char <= lines[start.line].len() { lines[start.line][..start.char] }
           else { lines[start.line] }          # [Q7] clamps to whole line
    tail = if end.char <= lines[end.line].len() { lines[end.line][end.char..] } else { "" }
    out += head + change.text + tail
for l in lines[end.line+1..]: out += '\n' + l
```

Offsets are byte offsets (Go string slicing) — same UTF-16 caveat as the lexer
(**[Q7]**; non-ASCII slicing can panic in Rust on a non-boundary — use
`get(..)`/`is_char_boundary` and fall back to "unchanged" like the Go bounds checks).
Note a full-replace change with an *empty* text and zero range is indistinguishable
from "no range" — treated as full replacement; matches LSP.

## 3. `dbt_project.yml` parsing

> **Implementation note (08 §2):** the Rust port deserializes dbt YAML files
> into types generated from dbt-labs/dbt-jsonschema with `typify` — the structs
> in this section and §6.2 define the *subset of fields used* and the default
> rules, not the Rust type definitions. Positions come from a saphyr-marker
> side-table (08 §2.4) rather than wrapping every field; this section's
> `AnnotatedField` formulation remains the behavioral reference for which
> positions exist and how they are computed.

### 3.1 Annotated YAML

Every interesting scalar/map records its YAML node position (0-based):

```rust
struct AnnotatedField<T> { value: T, position: Position }
```

Position = `(node.line - 1, node.column - 1)` from the YAML library's marker.
`AnnotatedMap = HashMap<String, AnnotatedField<AnnotatedValue>>` where
`AnnotatedValue` is `Scalar(YamlValue) | Map(AnnotatedMap)` — nested mappings recurse,
and the position stored for a nested map is the position of its first value node.

Schema (`DbtProjectYaml`), YAML keys:

| field | key | type |
|---|---|---|
| project_name | `name` | AnnotatedField\<String> |
| profile | `profile` | AnnotatedField\<String> |
| model_paths | `model-paths` | AnnotatedField\<Vec\<String>> |
| seed_paths | `seed-paths` | AnnotatedField\<Vec\<String>> |
| macro_paths | `macro-paths` | AnnotatedField\<Vec\<String>> |
| packages_install_path | `packages-install-path` | AnnotatedField\<String> |
| docs_paths | `docs-paths` | AnnotatedField\<Vec\<String>> |
| vars | `vars` | AnnotatedMap |

Unknown keys ignored. Parse failure or unreadable file → default (all-empty) struct,
error printed (not fatal).

### 3.2 Pre-processing: `env_var` resolution (§7.3)

The raw file string is passed through `resolve_env_vars` **before** YAML parsing
(only for `dbt_project.yml`; properties YAML files are *not* resolved).

### 3.3 Post-parse defaults

Let `dirs` = names of directories directly under `project_root`:

- `model_paths` empty → `["models"]` if `models` dir exists.
- `seed_paths` empty → `["seeds"]` if `seeds` exists.
- `macro_paths` empty → `["macros"]` if `macros` exists.
- `packages_install_path` empty → `"dbt_packages"` if that dir exists.
- `docs_paths` empty → start with `["docs"]` if `docs` exists (else empty), then
  **always append** `model_paths` and `macro_paths` (post-default values).
  Note: if `docs-paths` *was* set in YAML, nothing is appended.

## 4. Project variables (`get_project_variables`)

From `project_yaml.vars` (empty map → empty result).
`project_uri = project_root + "/dbt_project.yml"` (path join).

1. For each top-level `vars` entry with key ≠ project name:
   `Variable { name: key, value, uri: project_uri, range: {start: pos, end: pos} }`.
2. If `vars[project_name]` exists and is a nested map: add each of its entries the
   same way, **skipping entries whose value is null**; these can overwrite step-1
   entries with the same name.

`value` keeps the YAML-typed scalar (string/int/bool/float/seq…); it is only ever
displayed via to-string formatting (hover: `"{name}: {value}"`; completion docs).
Go's `fmt` prints e.g. ints without quotes, nested maps as Go map syntax — exact
formatting of non-scalar values is not worth matching; pick a stable debug format.

## 5. Macros (`get_macro_details`, `parse_macros`)

> **Implementation note (06 §2.2):** the Rust port extracts macros with the
> minijinja lexer (and parser when it succeeds), not this regex. The regex below is
> the behavioral reference and the per-file fallback when minijinja tokenization
> fails; outputs (name, signature-as-description, uri, range) are identical.

Process list = own project + every installed package (§8). For each project in order,
for each `macro_paths` entry: walk `root/<path>` recursively for `.sql` files
(skip path if unreadable); for each file, extract macros from the raw text:

Regex (Rust `regex` syntax identical):

```
(?s)\{%-{0,1}\s*macro\s+(\w+\(.*?\))\s*-{0,1}%\}
```

For each match: capture group 1 = signature `name(args…)`.
- `name` = signature up to the first `(` (skip the match if no paren — unreachable
  given the regex, keep the guard).
- `description` = the **full signature string** (shown on hover/completion).
- `project_name` = the `name:` from **that project's own** `dbt_project.yml`
  (packages get their own package name).
- `uri` = macro file path; `range` = start/end of capture group 1, converted via
  `get_line_and_column` (§7.5) — byte-offset → (line, col), 0-based.

Aggregation: `macro_detail_map[project_name][macro_name] = macro` (later duplicates
overwrite).

## 6. Models, seeds, sources (`get_model_details`, `parse_yaml_models`)

Process list = own project + packages (§8). For each project `p`:

### 6.1 Model files on disk

`create_file_name_map([".sql", ".py"], p.root, p.model_paths)` (§7.4):
`{ filename-without-extension → absolute path }`.

### 6.2 Properties YAML (models + sources)

`parse_yaml_models(p.root, p.yaml)`:

1. Build the docs map (§6.4) from `docs_paths`.
2. For each `model_paths` entry (skip if `root/<path>` unreadable): walk
   `root/<path>/` for `.yml` files (**not** `.yaml` — keep), parse each as:

```yaml
models:
  - name: <str>              # annotated
    description: <str>       # annotated
    config: { alias: <str>, ... }   # annotated map; only `alias` is used
sources:
  - name: <str>              # annotated
    database: <str>          # parsed, unused
    schema: <str>            # parsed, unused
    description: <str>
    tables:
      - name: <str>
        description: <str>
```

   Parse errors → empty result for that file (logged).

3. Models → `model_schema_details[name] = { name, description: docs-expanded (§6.4),
   config{alias}, schema_uri: <yaml file path> }` (last file wins per name).
4. Sources → `source_map[name] = Source { name, description: docs-expanded,
   uri: <yaml file path>, range: name-position (zero-width), tables }`;
   each table → `SourceTable { name, description: docs-expanded, table: source name,
   uri: file, range: table-name position (zero-width) }`.

### 6.3 Merge into `model_detail_map`

For each `(file_stem, path)` in the §6.1 map:

- key = `alias` from the schema entry's config if it is a non-empty string, else file stem.
- If a schema entry exists: take `description`, `schema_uri`,
  `schema_range = name position (zero-width)`; else all empty.
- `model_detail_map[key] = ModelDetails { uri: path, project_name: p.name, ... }`.

Sources from every project merge into one `source_detail_map` (later projects
overwrite on name collision).

Finally, seeds (own project only): `create_file_name_map([".csv"], root, seed_paths)`;
each entry → `ModelDetails { uri, project_name: own project, description: "Seed File",
schema_uri: "", schema_range: default }` (overwrites model names on collision).

### 6.4 `{% docs %}` blocks (`parse_markdown.go`)

> **Implementation note (06 §2.3):** block extraction and the `{{ doc(...) }}`
> substitution use the minijinja lexer in the Rust port; the regexes below are the
> behavioral reference and per-file fallback. This fixes Q2, the greedy cross-block
> capture, and the stale-offset splice by construction.

- Collect files: for each `docs_paths` entry, if `ReadDir(path)` succeeds, walk for
  `.md`. **[Q4]** `path` is used as given (relative), *not* joined to project root —
  only works when the server's CWD is the project root. Recommended fix: join with
  the project root (behavior identical when entries are absolute).
- Extract blocks per file with:

```
(?s){%-{0,1}\s*docs\s+([a-zA-z]+)\s*-{0,1}%}(.*){%-{0,1}\s*enddocs\s*%}
```

  → `{ name: group1, content: trim(group2) }`, last block per name wins.
  (`(.*)` is greedy → with multiple blocks in one file the match spans across blocks;
  **[Q2-adjacent]** faithful port keeps it; fix = lazy `(.*?)` + `[a-zA-Z0-9_]+`.)
- `replace_description_docs_blocks(description, docs_map)`: find all matches of

```
{{\s*doc\(('|")([-zA-z]+)('|")\)\s*}}
```

  For each match **in order**, if `docs_map` has the captured name, splice
  `content` in place of the full match. **[QUIRK]** Replacement reuses the *original*
  string's match indices while editing a *copy* — with multiple `doc()` references in
  one description the later splices use stale offsets. Faithful port: replicate
  index-based splice; recommended fix: `Regex::replace_all` with a lookup closure.
  **[Q2]** `[-zA-z]` char class as written.

#### Addendum: doc() as a first-class reference (beyond the Go original)

The docs map is no longer dropped after description expansion: it lives
on the context as `doc_detail_map` (`context/docs.rs`) — block name →
file, zero-width range at the `{% docs name %}` identifier, content and
project name. Names are flat, as `doc('name')` references them; on a
collision the FIRST definition wins and the own project is scanned
first, so it beats installed packages. Two-argument
`doc('package', 'name')` is not resolved yet (deferred with
cross-project refs). On top of the map, with the `doc('…')` argument
refined to its own token kind (02 §3.3):

- hover shows the block's markdown; definition jumps to the block
  header — in properties YAML too (same token path);
- `doc('` completes block names with the usual closing-suffix
  mirroring (§9.4.1), content resolved lazily via completionItem/resolve;
- a `doc()` naming no known block warns as `dangling-doc` with
  did-you-mean suggestions, in SQL and in properties YAML (the YAML
  diagnostics pass runs this check even though it skips the SQL
  dangling-ref pass). Column descriptions still skip doc() EXPANSION
  (unchanged from the Go port); the dangling check covers them anyway.

## 7. Util functions

### 7.1 Project root (`get_project_root(file_name, wd)`)

- `wd == ""` → use `std::env::current_dir()`.
- **Ascending**: from `wd` upward (`path = parent(path)`), return the first dir whose
  direct entries contain `file_name`; stop with error after checking `/`.
  Unreadable dir → error immediately.
- If ascending failed, **descending**: BFS from `wd` over subdirectories; return the
  first dir containing `file_name` (checks entries in `read_dir` order; queues every
  subdir). Unreadable dir during BFS → error. (No depth limit, no symlink guard —
  consider a depth cap as a safe deviation.)
- Both failed → return `""` (logged).

### 7.2 SQL dialect (`get_dialect(profile_name, input_dir)`)

profiles.yml location — **[Q5]**:

- `input_dir != ""` (the normal case: workspace root) → `input_dir/.dbt/profiles.yml`.
- `input_dir == ""` → `$DBT_PROFILES_DIR/profiles.yml` if the env var is set, else
  `~/.dbt/profiles.yml`.

Parse:

```yaml
<profile_name>:
  target: <default_target>     # key: "target"
  outputs:
    <target>:
      type: <dialect>          # key: "type"
```

Return `outputs[target].type` as the dialect string; any failure (file missing,
profile missing, target missing) → `""` (logged). Dialect is a plain string
("snowflake", "duckdb", "bigquery", "postgres", …); only the first two select
keyword tables (02 §1.2) and only snowflake/bigquery have function docs (04).

### 7.3 `resolve_env_vars(input) -> String`

Regex:

```
\{\{\s*env_var\(\s*('|")([^'"]+)('|")\s*(?:,\s*('|")([^'"]*?)('|"))?\s*\)\s*\}\}
```

Replace every match: if `$<group2>` is set → its value; else if a default was given
(group 4 non-empty, i.e. the optional part matched) → group 5 (may be empty string);
else the literal string `DBT_ENV_DEFAULT_<name>` (and log). Table of cases in
`util/resolve_env_vars_test.go` — port it.

### 7.4 File maps & walking

- `walk_filepath(path, ext) -> Vec<String>`: recursive walk, collect files whose
  extension == `ext` (exact match incl. dot, e.g. `.sql`). Go's `filepath.Walk`
  visits in lexical order per directory — use `walkdir` with `sort_by_file_name`
  so "last wins" collisions resolve identically.
- `create_file_name_map(exts, root, paths)`: for each `p` in `paths`, if
  `root/<p>` readable, for each ext: walk and insert
  `basename minus ext → full path`. Later entries overwrite.

### 7.5 `get_line_and_column(input, byte_idx) -> (line, column)`

Count `\n` in `input[..idx]`; line = count, column = `idx - (index after last \n)`.
Byte-based, 0-based.

## 8. Installed packages (`parse_packages.go`)

- `package_root_paths`: if `packages_install_path` empty → none. Else list direct
  subdirectories of `project_root/<packages_install_path>`.
- For each package dir: parse its own `dbt_project.yml` (§3, incl. env_var resolution
  and defaults). This yields `ProjectDetails { root_path, yaml }` used by §5/§6.
  (Go computes a "valid model/macro paths" list and throws it away — skip that.)

## 9. Feature algorithms

All features start with `find_token_at_cursor(line, character)` on the document's
TokenIndex (02 §3.5). **[Q8]** If the uri was never opened, Go reads zero-value maps
and nil pointers that happen to be safe; in Rust: unknown uri or lookup error →
return the default response described per feature.

### 9.1 Hover

Default/failure result: `{ contents: "" }`.

By cursor-token type:

- `REF` → `model_detail_map[lit].description` (missing → "").
- `SOURCE` → `source_detail_map[lit].description`.
- `SOURCE_TABLE` → lookback `(SOURCE, 4)`; if matched with source name `s`:
  `format!("Source: {}\n{}\n\nTable: {}\n{}", src.name, src.description, tbl.name, tbl.description)`
  where `tbl = source_detail_map[s].tables[lit]` (missing entries → empty fields).
  No lookback match → "".
- `VAR` → `format!("{}: {}", lit, variable_detail_map[lit].value)` (missing var →
  Go prints `<nil>`; use a sensible empty representation).
- `MACRO` → package = lookback `(PACKAGE, 2)` literal if matched, else own project
  name; result `macro_detail_map[pkg][lit].description`.
- anything else → `dialect.function_docs()[lit]` (missing → ""). Lookup key is the
  raw literal — docs maps are lowercase, so `ABS(` in caps gets no hover. Keep.

### 9.2 Definition

Default result: `Location { uri: <request uri>, range: zero-width at cursor }` —
returned whenever lookup fails at any point.

- `REF` → if `model_detail_map[lit]` exists (Go: != zero struct):
  `uri = "file://" + model.uri`, range = zero-width at (0,0).
- `SOURCE` → if `source_detail_map[lit].name != ""`:
  `uri = "file://" + source.uri`, range = source.range.
- `SOURCE_TABLE` → lookback `(SOURCE, 4)`; table entry's
  `"file://" + uri` + its range (if `name != ""`).
- `VAR` → if variable exists: `"file://" + var.uri`, var.range.
- `MACRO` → package as in hover; if macro exists: `"file://" + macro.uri`, macro.range.
- default → CTE jump: uri stays the request uri; if `def_tokens[lit]` exists,
  range = zero-width at the definition token's (line, column).

### 9.3 Go to schema (`dbt.goToSchema`)

Result: `Location | null` (null when nothing found).

1. If the document exists and the cursor is on a `REF` token and that model has a
   non-empty `schema_uri`: return `{ uri: "file://" + schema_uri, range: schema_range }`.
2. Else derive the model name from the request uri: strip `file://` prefix, basename,
   must end in `.sql` (else name = "" → null). Look it up; if found with non-empty
   `schema_uri` → Location as above; else null.

### 9.4 Completion

Context detection on the cursor line (`documents[uri].text` split on `\n`;
Go panics if `position.line` is out of range — guard in Rust, return empty list):

```
before = line[..character]   # byte slice
after  = line[character..]
```

First regex that matches `before` wins (**[Q1]** `[a-zA-z]` as written):

1. `\bref\(('|")[a-zA-z]*$` → ref items from `model_detail_map`
2. `\bsource\(('|")[a-zA-z]*$` → source items
3. `\bvar\(('|")[a-zA-z]*$` → variable items
4. `\{\{\s*` (anywhere in `before`) → macro items
5. else → `dialect.function_completion_items()` (04 §3)

Item builders (fields per 01 §2.3; map iteration order is random in Go — order is
irrelevant, clients sort by `sortText`):

- **ref**: label=model name, detail=`"Project: {project_name}"`, documentation=
  description, kind=18 (Reference), insertText=`name + suffix`, sortText=name.
- **source** (one item per source×table): label=`"{source} - {table}"`,
  detail=`"Source: {source}"`, documentation=
  `"{src.description}\n\nTable: {tbl}\n{tbl.description}"`, kind=18,
  insertText=`"{source}{q}, {q}{table}{suffix}"` (`q` = quote type, §9.4.2),
  sortText=source name.
- **var**: label=name, detail=name, documentation=`"{value}"`, kind=6 (Variable),
  insertText=`name + suffix`, sortText=name.
- **macro** (all packages): label=name, detail=`"Project: {package}"`,
  documentation=macro.description, kind=15 (Snippet), insertText= name if package ==
  own project else `"{package}.{name}"`, sortText=name.

#### 9.4.1 Closing-suffix computation (`get_suffix(line, after, kind)`)

If `after != ""` → suffix `""` (don't double-close). Else:

- `prefix = first match of `\{\{\s*<kind>\(('|")` in the full line` (kind ∈ ref|source|var).
- Remove the first occurrence of `<kind>` from it, then reverse the remaining
  characters, mapping `(`→`)` and `{`→`}` (every other char passes through).

Example: line `select * from {{ ref('` → prefix `{{ ref('` → minus `ref` → `{{ ('`
→ suffix `') }}`. So a completed insert is `customers') }}`. Note the mirrored string
preserves the user's spacing and quote style. (Only `(`/`{` are mapped — quotes mirror
as themselves, which is what makes this work.)

#### 9.4.2 Quote type (`get_quote_type(line)`)

First match of `['"]` in the line, or `""` if none. (Go comment claims "last";
`FindString` returns the first — **[Q3]**, keep or fix consciously; used only for the
source item's middle quotes.)

## 10. Feature-batch additions (binding)

- **dbt artifacts** (`cogitator-analysis/src/artifacts/`): readers for
  `target/run_results.json`, a minimal `manifest.json` index (model
  name → unique_id/path; test unique_id → attached_node),
  `sources.json` and `catalog.json`, deserialized through the types
  GENERATED from the vendored schemas.getdbt.com definitions (08 §4)
  behind the same lean public structs. Leniency is MANUFACTURED in the
  preprocess artifact mode plus the adapters: every field `Option`,
  unknown fields ignored, statuses plain strings, a row missing
  `unique_id` drops while its siblings survive, and schema-version
  mismatches trace (shared `trace_schema_version`) — read, trace,
  never reject. Parse failures degrade the artifact features off with
  a Guidance trace — artifacts must never break core diagnostics.
  Pure joins in `artifacts/status.rs` (run status per model, test
  failures, manifest drift, source freshness); the shell stamp-caches
  per file and skips files over `limits.artifact_mib`.
- **catalog.json** (`artifacts/catalog.rs`): `dbt docs generate`
  output as a `CatalogIndex` keyed by unique_id segments, columns
  ordinal-sorted with warehouse types/comments. Source tables whose
  YAML declares no columns are enriched from it during the context
  refresh — ALWAYS on (one stamp-cached local file, adapter-agnostic;
  the duckdb warehouse pass wins where both run), shown as
  "<TYPE> (from catalog)" in completion/hover. Model columns stay
  YAML-only so the `undocumented_columns` lint keeps meaning (filling
  them from the catalog is a recorded possible follow-up).
  `dbt.doctor` reports the catalog age.
- **Graph**: `graph/` splits into `mod.rs` + `traverse.rs`; `NodeKind`
  gains `Exposure`, `Metric`, `SemanticModel`; transitive helpers
  (`transitive_downstream/upstream`, `downstream_closure_with_depth`,
  petgraph Bfs, deduped, excluding self) power dead-model detection and
  `dbt.impact`.
- **Exposures/metrics/semantic models**: `prune_for_server` keeps their
  properties-YAML sections; `context/exposures.rs` builds detail maps
  with positions; they join the graph with Ref edges; dangling targets
  warn on the YAML range.
- **Packages**: `packages.yml` parses through the generated schema type;
  `dbt_packages/<pkg>` contributes macros (full) and model NAMES only
  (no SQL parse, file-count capped) so two-arg `ref('pkg','model')`
  resolves and dangling-ref checks consult package models.
- **Lint** (`features/lint/`): opt-in, flag-per-check (cogitator.yml
  `lint:`), each check a module ≤ ~100 lines over the token stream /
  lineage; staging models exempt from `select_star`.
- **Config schema** (`config_schema.rs`): the single `KeyDoc` table that
  cogitator.yml parsing, diagnostics, completion and hover all consume.
- **Scan-cache persistence**: the FileCache (mtime,size) index plus
  payloads serialize to `<target-path>/.cogitator-cache` (JSON, versioned
  header, ignore-on-any-error, atomic same-dir rename). This supersedes
  the earlier "no on-disk persistence by design" note — scope is the
  SCAN cache only; the derived cache stays memory-only.

## Addendum: §9.2 definition on SQL functions

Definition on a documented built-in function (snowflake/bigquery
bundles, 04 §3) materializes the bundled markdown at
`target/.cogitator/docs/<dialect>/<name>.md` (rewritten only when the
content drifted — the docs are static per binary) and answers a
Location into it: the stdlib-sources move, in Markdown. Precedence
matches the `_` arm — a CTE or resolvable column named like the
function still wins. The write happens off the dispatcher task; any
write failure falls back to the plain self-location, never an error.
Function-doc lookups are case-insensitive everywhere (`SUM` hovers
and resolves like `sum`).

## Addendum: §6 snapshots, singular tests and analyses

The scan covers three more dbt path lists (`snapshot-paths`,
`test-paths`, `analysis-paths`; defaults `snapshots`, `tests`,
`analyses`/`analysis`, each only when the directory exists):

- **Snapshots** are ref-able: every `{% snapshot name %}` block under a
  project's snapshot paths (scan-cached per file, persisted like the
  other caches) joins `model_detail_map` by **block name** with
  `ModelKind::Snapshot` and description `Snapshot File`. A name
  collision with a real model keeps the model. Completion, hover,
  definition and dangling-ref checks then cover snapshot refs for free.
- `ModelDetails` carries a `ModelKind` (`Model`/`Seed`/`Snapshot`);
  dead-model detection excludes every non-`Model` kind (a seed backs
  ad-hoc queries, a snapshot is its own end product) instead of the old
  `.csv`-extension probe.
- **Singular tests and analyses** (own project only) join the project
  graph as leaf dependents (`NodeKind::Test`/`Analysis`, named by file
  stem, same edge-seed cache): find-references, impact, the DAG and
  workspace dangling-ref coverage see them; they are never ref-able and
  never in `model_detail_map`.
- After the model scan the graph back-fills empty-uri model nodes from
  `model_detail_map` (`fill_model_uris`): refs to seeds and snapshots —
  first seen as bare edge targets — stop counting as dangling.

## Addendum: §10 model governance

Properties parsing carries the governance keys of each model entry —
`access`, `group` (entry- or config-level) and
`config.contract.enforced` — through `ModelEntry` into `ModelDetails`.
Two diagnostics consume them:

- **private-ref** (push + pull): a `ref()` whose target has
  `access: private` and a group different from the referencing
  model's warns at the ref token. A private model without a group
  restricts nothing, matching dbt's group-scoped access rule.
- **contract-drift** (pull path only — it needs lineage): with
  `contract.enforced: true` and documented columns, the YAML column
  set is compared by name (case-insensitive) against the produced
  columns; both directions warn, and any `*` production disables the
  check instead of guessing.

`unit_tests:` entries targeting a model in its own properties file
ride along in dbt.modelTests / dbt.goToTests with `unit: true` on the
wire; data tests carry `unit: false`.

## Addendum: §10 config JSON Schema export

`config_schema::json::config_json_schema()` renders the KeyDoc table
as a draft-07 JSON Schema (typed wire structs, no ad-hoc JSON); the
`cogitator schema` subcommand prints it, `just gen-config-schema`
writes the committed `schemas/cogitator.schema.json`, and a sync test
fails the suite while the committed copy is stale. Editors point
yaml-language-server at the file to validate cogitator.yml without
the server running. value_hint drives the typing: `bool` → boolean,
`MiB`/`ms` → integer ≥ 0, `[names]` → string array, `a|b|c` → closed
string enum, anything else string.
