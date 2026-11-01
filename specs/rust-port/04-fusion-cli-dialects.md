# 04 — CLI, dbt Fusion Diagnostics, Dialect Docs, Build & Release

Source files: `main.go` (CLI), `util/validate_fusion.go`, `analysis/fusion/compile.go`,
`docs/*.go`, `version/version.go`, `build-all-platforms.sh`.

## 1. CLI

Binary name: `dbt-language-server`. Behavior:

1. **`upgrade` subcommand** (checked before flag parsing: `argv[1] == "upgrade"`):
   runs `bash -c "curl -fsSL https://raw.githubusercontent.com/j-clemons/dbt-language-server/main/install | bash"`
   with inherited stdout/stderr; exit 0 on success, fatal log on failure.
   For the Rust port, point the URL at the port's own install script (decide at
   release time); otherwise identical. (`clap` subcommand `upgrade`.)
2. Flags (pflag → clap):
   - `-v, --version` (bool): print version string + newline, exit 0.
   - `-d, --debug` (bool): enable file logging (01 §5).
   - `-f, --fusion [PATH]` (string, **optional value**; when passed without a value
     it means `"dbt"`): enables Fusion mode with that executable path/name.
     clap: `#[arg(short, long, num_args = 0..=1, default_missing_value = "dbt")]`.
     Absent flag → empty string → Fusion disabled.
3. Startup with `--fusion`: spawn a background thread running `validate_fusion`:
   execute `<fusion_path> --version`, capture stdout; valid iff the command succeeds
   and stdout begins with `"dbt-fusion"`. Set the shared `fusion_enabled` flag to the
   result. (Go slices `output[0:len("dbt-fusion")]` — panics if output is shorter;
   guard with `starts_with` in Rust.) Errors are logged; server runs regardless.
4. Then the stdio message loop (01 §3).

## 2. Fusion compile → diagnostics (`fusion_compile`)

Called after handling `didOpen` and `didSave` (01 §3.2). No-op unless
`fusion_enabled` is true.

### 2.1 Model selector from URI

```
dbt_model_selection_from_uri(uri) -> String
```

Find the **last** occurrence of the substring `"models"` in the uri; if absent → `"*"`.
Else take everything after it, trim a leading `/`, trim a trailing `.sql`, replace
every `/` with `.`. E.g. `file:///…/models/staging/stg_orders.sql` → `staging.stg_orders`.
(Substring match, not path segment — a path containing `datamodels` would mis-split;
faithful port keeps it.)

### 2.2 Artifact directories

```
get_fusion_artifact_path(project_name) -> Result<PathBuf>
```

`~/.dbt/dbt-language-server/fusion-artifacts/<project_name>` — create it plus `log/`
and `target/` subdirs (mkdir -p, 0755). `project_name` is
`project_yaml.name` (may be empty → path ends at `fusion-artifacts/`; keep the Go
behavior, it still works).

### 2.3 Process invocation

```
<fusion_path> compile -q --static-analysis on --log-format json --no-write-json \
    --target-path <artifacts>/target --log-path <artifacts>/log --select <selector>
```

Working directory: inherited from the server process (not set explicitly).
Capture stdout and stderr pipes; read both concurrently line-by-line.

### 2.4 Log stream processing

Each line is one JSON object (dbt Fusion's `--log-format json`). Deserialize
(unknown fields ignored; **field names are case-insensitive in Go** — Fusion emits
lowercase keys; in Rust use lowercase `serde` renames):

```jsonc
{ "data": { "logVersion": int, "version": string /* e.g. "=2.0.0-beta.x" */ },
  "info": { "category": string, "code": string, "extra": {...},
            "invocationId": string, "level": string, "msg": string,
            "name": string, "pid": int, "thread": string, "ts": string } }
```

Unparseable lines: log and skip. For each entry build `(diag_uri, Diagnostic)` via
§2.5; **emit it only if `uri.contains(diag_uri)`** (the request's document uri
contains the relative path from the log). Diagnostics accumulate in a Vec; after
*each* accepted diagnostic Go publishes the whole accumulated list
(`textDocument/publishDiagnostics`, 01 §2.3) and publishes once more (possibly empty)
after the process exits. An empty final publish is what clears old diagnostics when
a file becomes clean — preserve that.

**[Q9]** All of this runs synchronously inside the message handler. Recommended
deviation in 00 (background thread + stdout mutex); if you do, also debounce/serialize
per-uri so didOpen+didSave in quick succession don't interleave publishes.

### 2.5 Parsing one log entry (`parse_compile_log` / `parse_fusion_log_msg`)

Severity from `info.level`: `info`→3, `warning`→2, `error`→1, else 4 (Hint).

Message parsing (`msg`, `code`, `level`):

1. Strip ANSI color codes: regex `(\\u001b\[[0-9;]*m|\x1b\[[0-9;]*m)` (matches both a
   literal ESC byte and the *textual* sequence `\u001b[...m`).
2. Split on `" --> "`. If not exactly 2 parts → return
   `("NO URI FOUND", cleaned_msg, Range::default())` (such diagnostics are then
   dropped by the `uri.contains` filter — "NO URI FOUND" never matches).
3. Message = part 0, trimmed, with literal `\n` **two-character sequences** removed
   (`strings.ReplaceAll(msg, "\\n", "")`), then strip in order:
   leading `"<level>: "` (regex `^<level>: `), leading `"<code>: "` or `"dbt<code>: "`,
   any `"(in <path>:<line>)"` fragments (regex `(\(in .*:\d*\))`), trim again.
   (`level`/`code` are interpolated into regexes — they're alphanumeric in practice;
   use `regex::escape` in Rust to be safe.)
4. Location = part 1 trimmed, first space-separated word, split on `:` →
   `path:line:col` (fewer than 3 parts → `("NO URI FOUND", message, default range)`).
   line/col are 1-based in the log → subtract 1 (parse failure → 0). Range is
   zero-width at that position.

Diagnostic fields: `range`, `message`, `severity`, `code: info.code`,
`source: format!("dbt Fusion {}", data.version)`.

## 3. Dialect function docs (`docs/`)

```rust
enum Dialect { /* wraps the string; see 03 §7.2 */ }
fn function_docs(&self) -> &'static HashMap<String, String>
    // "snowflake" → SNOWFLAKE_FUNCTIONS, "bigquery" → BIGQUERY_FUNCTIONS, else empty
fn function_completion_items(&self) -> Vec<CompletionItem>
    // one item per function: label=key, detail="", documentation=value,
    // kind=3 (Function), insertText=key, sortText=key
```

The data: two big string→markdown maps in `docs/snowflake.go` (~740 entries) and
`docs/bigquery.go` (~390 entries). Don't hand-port: extract once to JSON with a tiny
script (the Go files are flat `"key": "value",` literals — easiest is a 10-line Go
program in the old repo that `json.Marshal`s the two maps), commit as
`src/docs/snowflake.json` / `bigquery.json`, and load with
`include_str!` + `serde_json` behind `LazyLock`. Keys are lowercase function names;
values are markdown with ```sql fences and doc links.

Note the asymmetry (faithful): keyword tables exist for snowflake/duckdb (02),
function docs for snowflake/bigquery. A duckdb project gets keywords but no
function hover/completions; bigquery gets docs but no keywords.

## 4. Version & builds

- Go: `version.Version` (default `"dev"`) set via `-ldflags "-X ...version.Version=…"`.
  Rust: `env!("CARGO_PKG_VERSION")`, or build.rs embedding `git describe` into an env
  var for `-v` output parity (`v0.x.y` format — match whatever the release pipeline tags).
- `build-all-platforms.sh` cross-compiles (linux/darwin/windows × amd64/arm64).
  Rust equivalent: CI matrix with `cargo build --release --target …` (or `cross`/
  `cargo-zigbuild` for linux-musl + windows-gnu from one host). Produce the same
  artifact names the install script expects if the `upgrade` flow is kept.

## 5. Acceptance checklist (end-to-end parity)

Scripted LSP session against `testdata/jaffle_shop_duckdb` (and the Go repo's other
fixtures), run through both binaries, byte-comparing responses after JSON
normalization:

1. `initialize` → capabilities/serverInfo (01 §3.1).
2. `didOpen` on a model → no response; state refreshed.
3. `hover` on: `ref('…')` arg, `source('a','b')` source + table args, `var('…')` arg,
   macro name (own + package-qualified), a SQL function with the dialect configured,
   a plain identifier (empty contents).
4. `definition` on each of the above + a CTE name usage → ranges per 03 §9.2.
5. `completion` inside `ref('`, `source('`, `var('`, after `{{ `, and in plain SQL.
6. `executeCommand dbt.goToSchema` on a ref token and on a model file body.
7. `didChange` incremental edits incl. multi-line replace → re-run 3–5.
8. `shutdown`/`exit` → clean exit 0.
9. (Fusion, manual) with a Fusion install: open a model with an error → diagnostics
   published, fix → empty publish clears them.
