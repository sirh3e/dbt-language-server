# Rust Port — Overview & Architecture

Implementation specification for porting `dbt-language-server` (Go) to Rust.
Source of truth for all behavior: the Go code at commit `724b0f2`. Every spec file
in this directory describes the behavior to reproduce, plus explicitly marked
**[QUIRK]** (faithful-but-odd behavior you may keep or fix) and **[DEVIATION]**
(recommended changes for the Rust version) notes.

## What the server does

A Language Server for dbt projects (SQL + Jinja). It speaks LSP over stdio and provides:

| Capability | LSP method | Behavior |
|---|---|---|
| Hover | `textDocument/hover` | Docs for `ref()`, `source()`, `var()`, macros, and dialect SQL functions |
| Go to definition | `textDocument/definition` | Jump to model file, source/table YAML, variable in `dbt_project.yml`, macro definition, or CTE definition in current file |
| Completion | `textDocument/completion` | Context-aware items inside `ref('`, `source('`, `var('`, `{{` (macros), else dialect functions |
| Go to schema | `workspace/executeCommand` → `dbt.goToSchema` | Jump to the model's entry in its properties (schema) YAML |
| Diagnostics | `textDocument/publishDiagnostics` (push) | Only when `--fusion` is enabled: runs `dbt compile` (Fusion) on open/save and converts its JSON logs to diagnostics |

Document sync is **incremental** (`textDocumentSync: 2`).

## Go package map → Rust module map

| Go package | Lines | Responsibility | Rust module (proposed) |
|---|---|---|---|
| `main.go` | 212 | CLI flags, stdio loop, method dispatch | `src/main.rs` |
| `rpc/` | 63 | LSP base-protocol framing (Content-Length) | `src/rpc.rs` |
| `lsp/` | ~250 | Protocol structs (requests/responses/notifications) | `src/lsp/` |
| `analysis/` | ~1300 | State, dbt project context, hover/definition/completion logic, YAML parsing | `src/analysis/` |
| `analysis/parser/` | ~845 | SQL/Jinja lexer + token-stream parser + cursor index | `src/analysis/parser/` |
| `analysis/fusion/` | 286 | dbt Fusion compile integration → diagnostics | `src/analysis/fusion.rs` |
| `util/` | ~300 | Project-root discovery, profiles.yml dialect, env_var resolution, file walking, logging | `src/util/` |
| `docs/` | ~1175 | Static SQL function documentation (Snowflake, BigQuery) | `src/docs/` + data files |
| `version/` | 3 | Version string injected at build time | `env!("CARGO_PKG_VERSION")` / build-time env |

Spec files:

- [01-protocol.md](01-protocol.md) — framing, message types, dispatch, lifecycle
- [02-lexer-parser.md](02-lexer-parser.md) — tokens, lexer, parser state machine, cursor index
- [03-analysis.md](03-analysis.md) — state, dbt context building, YAML schemas, feature algorithms, util functions
- [04-fusion-cli-dialects.md](04-fusion-cli-dialects.md) — CLI, Fusion diagnostics, dialect docs data, build/release
- [05-rust-stack.md](05-rust-stack.md) — **binding** stack & architecture decisions (error type, sqlparser lexing, petgraph graph, tokio runtime, testing stack); overrides 00–04 where they conflict

## Dependency mapping

Decided stack (details and rationale in [05-rust-stack.md](05-rust-stack.md)):

| Go | Rust (binding) |
|---|---|
| ad-hoc error returns | custom `Error` enum via `thiserror` (`src/error.rs`); `anyhow` only at the edges (05 §2) |
| `encoding/json` | `serde` + `serde_json` (field names given per struct in 01/03) |
| `gopkg.in/yaml.v3` (node positions!) | `saphyr` or `yaml-rust2` **parsed via their low-level `Marker` API** — see note below |
| hand-rolled SQL lexer + keyword tables | `sqlparser` tokenizer (dialect-aware) behind a small Jinja segmenter (05 §4) |
| — (no graph in Go) | `petgraph` project dependency graph (05 §5) |
| `github.com/spf13/pflag` | `clap` (derive) |
| `regexp` | `regex` crate (all patterns in these specs are compatible; compile once with `std::sync::LazyLock`) |
| goroutines + `sync.WaitGroup` | `tokio` runtime for I/O/coordination + `spawn_blocking`/scoped threads for CPU & fs work (05 §6) |
| `bufio.Scanner` + custom split | async framed reader task over `tokio::io::stdin` (01 §1.3 semantics, 05 §6.2) |
| `os/exec` | `tokio::process::Command` |
| Go `testing` tables | `rstest` cases + `insta` snapshots + `pretty_assertions` (05 §7) |

**Critical YAML constraint:** the Go code relies on `yaml.Node.Line`/`.Column` to
record the position of every parsed value (`AnnotatedField`, see 03). `serde_yaml`
(deprecated) and plain serde deserialization *cannot* provide this. Build a small
"annotated YAML" layer on `saphyr`/`yaml-rust2`'s event/marker API that produces:

```rust
struct AnnotatedField<T> { value: T, position: Position }  // position: 0-based line/char
type AnnotatedMap = HashMap<String, AnnotatedField<AnnotatedValue>>; // recursive
```

YAML markers are 1-based; the Go code stores `line-1` / `column-1` (0-based).

## Proposed crate layout

Single binary crate (the project is small; a workspace adds no value):

```
dbt-language-server/
├── Cargo.toml
├── build.rs                  # optional: embed git describe as version
├── src/
│   ├── main.rs               # CLI + message loop + dispatch
│   ├── rpc.rs                # framing: encode/decode/split
│   ├── lsp/
│   │   ├── mod.rs            # Request/Response/Notification bases
│   │   ├── types.rs          # Position, Range, Location, TextDocument*
│   │   ├── initialize.rs     # capabilities, server info
│   │   ├── features.rs       # hover/definition/completion/executeCommand/diagnostic types
│   │   ├── completion_kind.rs
│   │   └── diagnostic_severity.rs
│   ├── analysis/
│   │   ├── mod.rs            # State, Document, DbtContext
│   │   ├── features.rs       # hover, definition, completion, go_to_schema
│   │   ├── dbt_yaml.rs       # dbt_project.yml + properties yaml (annotated)
│   │   ├── models.rs         # model/seed/source detail maps
│   │   ├── macros.rs         # macro extraction
│   │   ├── variables.rs      # project vars
│   │   ├── packages.rs       # dbt_packages traversal
│   │   ├── markdown_docs.rs  # {% docs %} blocks
│   │   ├── completion_items.rs
│   │   ├── fusion.rs         # dbt Fusion compile → diagnostics
│   │   └── parser/
│   │       ├── mod.rs
│   │       ├── token.rs      # TokenType, keyword tables
│   │       ├── lexer.rs
│   │       └── parser.rs     # token refinement + TokenIndex
│   ├── docs/
│   │   ├── mod.rs            # Dialect enum, FunctionDocs lookup
│   │   ├── snowflake.json    # extracted data (include_str!)
│   │   └── bigquery.json
│   └── util/
│       ├── mod.rs
│       ├── project_root.rs
│       ├── sql_dialect.rs    # profiles.yml → dialect
│       ├── env_vars.rs       # {{ env_var(...) }} resolution
│       ├── files.rs          # walk, filename maps, line/col from byte offset
│       └── logging.rs
└── testdata/                 # copy as-is from the Go repo (jaffle_shop_duckdb etc.)
```

**[DEVIATION — optional]** Instead of hand-rolled `src/lsp/` + `src/rpc.rs`, you may use
`lsp-server` + `lsp-types` (rust-analyzer's crates). If you do, keep the *response
shapes* identical where the Go server is non-standard (documented in 01), or accept
the standard-compliant improvements deliberately. For a first 1:1 port, hand-rolled
is simpler to verify against the Go binary.

This layout is amended by 05 §8: adds `src/error.rs`, `src/analysis/graph.rs`,
`src/analysis/parser/jinja.rs`, and replaces the flat main-loop code with a
`src/server/` module (reader/dispatcher/writer/refresh tasks).

## State & concurrency model

Go (see 03 for details): one `State` struct owned by a single-threaded message loop;
a `sync.RWMutex` guards only the `FusionEnabled` flag (set from a background
validation goroutine). Context refresh (`refreshDbtContext`) runs three builders in
parallel and **blocks the message loop**, as does the Fusion compile (Q9).

Rust (full design in 05 §6): tokio task pipeline — reader task → dispatcher (sole
owner of `Documents`) → writer task owning stdout. `DbtContext` is rebuilt by a
debounced background refresh task and published as immutable `Arc` snapshots through
`tokio::sync::watch`; request handlers never touch the filesystem or wait on a scan.
Fusion compiles run as async child processes feeding the writer channel, cancelled
per-URI when superseded. `fusion_enabled` is an `Arc<AtomicBool>` set by the
background `--version` validation.

## Porting order (each step independently testable)

1. **`rpc`** — framing codec. Port `rpc/rpc_test.go` (encode/decode round-trip).
2. **`lsp` types** — serde structs; snapshot-test JSON output against the spec examples in 01.
3. **`util`** — project root (asc+desc search), env_var regex, dialect from profiles.yml,
   file walking, `get_line_and_column`. Port the Go tests (`util/*_test.go`).
4. **`parser`** — token.rs, lexer.rs, parser.rs. Port `lexer_test.go` and `parser_test.go` fully;
   these define the exact token streams expected.
5. **`docs`** — extract the two Go map literals to JSON (one-off script), load lazily.
6. **`analysis`** — YAML (annotated), model/macro/var/source maps, then the four features.
   Port `state_test.go`, `state_schema_test.go`, `state_incremental_test.go`,
   `parse_*_test.go`, `get_completion_items_test.go` against the shared `testdata/`.
7. **`fusion`** — log parsing (pure functions first: `parse_fusion_log_msg`,
   `dbt_model_selection_from_uri`), then process orchestration.
8. **`main`** — CLI, loop, dispatch. Port `main_test.go` (end-to-end message handling).
9. **Parity harness** — run Go and Rust binaries side by side on scripted LSP sessions over
   `testdata/jaffle_shop_duckdb`; diff raw response bytes (after normalizing key order).

## Known bugs / quirks index

Documented in context in the other files; collected here so the port makes conscious choices:

| # | Where | Issue |
|---|---|---|
| Q1 | completion regexes | `[a-zA-z]` char class (includes `[ \ ] ^ _` backtick) — typo for `[a-zA-Z]` (03) |
| Q2 | docs-block regex | `([-zA-z]+)` — typo; intended `([a-zA-Z_]+)` or similar (03) |
| Q3 | `getQuoteType` | comment says "last" quote, code returns the *first* quote in the line (03) |
| Q4 | `getDocsFiles` | docs paths are not joined with the project root → only works if server CWD == project root (03) |
| Q5 | `GetDialect` | when a workspace root path is known, *only* `<root>/.dbt/profiles.yml` is consulted; `DBT_PROFILES_DIR` and `~/.dbt` are used only when root path is empty (03) |
| Q6 | lexer | byte-based; columns are byte offsets, not UTF-16 code units as LSP requires → positions drift on non-ASCII lines (02) |
| Q7 | incremental sync | same byte-vs-UTF-16 issue; also silently drops edits whose char offsets exceed line length (03) |
| Q8 | hover/definition on unknown doc URI | Go indexes `s.Documents[uri]` and calls methods on a `nil` `Tokens` index — `FindTokenAtCursor` on nil map lookup happens to be safe in Go; in Rust model `documents: HashMap<String, Document>` and return the empty/default response when absent (03) |
| Q9 | fusion compile | runs synchronously in the handler; blocks all LSP traffic (04) |
| Q10 | `rpc::split` | assumes the header block is exactly one `Content-Length:` header; breaks if a client sends `Content-Type` too (01) |
| Q11 | parser `tokens` vec | Go stores `PrevToken *TokenLL` pointers into a reallocating slice (works by accident); in Rust use `prev: Option<usize>` indices into the token vec (02) |

Recommended defaults for the port: fix Q1–Q4, Q8, Q10, Q11 (invisible or strictly better),
keep Q5–Q7, Q9 behavior-compatible first and fix in a follow-up (they change observable
behavior / timing).

## Non-goals (same as the Go server)

- No Jinja templating engine, no dbt manifest parsing, no `target/` artifacts.
- No `textDocument/didClose`, no workspace folders, no configuration requests.
- No formatting, references, rename, semantic tokens.
