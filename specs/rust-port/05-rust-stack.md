# 05 — Rust Stack, Architecture & Performance (binding decisions)

This file fixes the technology and style decisions for the port. Where it conflicts
with suggestions in 00–04 (which were written stack-agnostic), **this file wins**.
00–04 remain the behavioral reference: *what* the server must do; this file defines
*how* it is built.

Decisions set by the project owner:

- `anyhow` + `thiserror`; one application error type.
- `petgraph` for all graph work (never hand-rolled graph structures).
- `insta` (snapshots), `rstest` (parameterized tests), `pretty_assertions`.
- `sqlparser` for the SQL side.
- Composable, function-style code; split into multiple files as needed.
- Performance is a feature: milliseconds matter on the request path.
- Threads and async I/O where they make sense.

## 1. Cargo manifest

```toml
[package]
name = "dbt-language-server"
edition = "2024"

[dependencies]
anyhow    = "1"
thiserror = "2"
serde      = { version = "1", features = ["derive"] }
serde_json = "1"
clap       = { version = "4", features = ["derive"] }
regex      = "1"
sqlparser  = "0.52"            # SQL tokenization (dialect-aware), see §4
minijinja  = { version = "2", features = ["unstable_machinery"] }  # scan-side Jinja, 06 §2
petgraph   = "0.6"             # project dependency graph, see §5
# planned phase (06 §3): tree-sitter = "…" + cc build-dependency + vendored grammars
saphyr     = "0.0.3"           # low-level YAML with markers (annotated YAML, 03 §3.1)
tokio      = { version = "1", features = ["rt-multi-thread", "io-std", "io-util",
                                          "process", "sync", "time", "macros", "fs"] }
walkdir    = "2"

[dev-dependencies]
insta             = { version = "1", features = ["json", "redactions"] }
rstest            = "0.23"
pretty_assertions = "1"
```

(Versions indicative — pin to current at project creation. `saphyr` may be swapped
for `yaml-rust2` if its marker API fits better; the requirement is per-node
line/column, 03 §3.1.)

## 2. Error handling

One application error type, defined in `src/error.rs` with `thiserror`; every
library-style module returns it. `anyhow` is used only at the *edges*: `main()`,
background task top-levels, and tests/tools — never in core module signatures.

```rust
// src/error.rs
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("malformed LSP frame: {reason}")]
    Frame { reason: String },
    #[error("yaml error in {path}: {message}")]
    Yaml { path: PathBuf, message: String },
    #[error("sql tokenizer: {0}")]
    SqlTokenizer(#[from] sqlparser::tokenizer::TokenizerError),
    #[error("unknown document: {uri}")]
    UnknownDocument { uri: String },
    #[error("no token at {line}:{character}")]
    NoTokenAtCursor { line: u32, character: u32 },
    #[error("dbt project root not found from {start}")]
    ProjectRootNotFound { start: PathBuf },
    #[error("fusion: {0}")]
    Fusion(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
```

Rules:

- The Go server swallows nearly every error and degrades gracefully (empty hover,
  default definition, skipped file). Keep that *behavior*, but make it explicit:
  fallible internals return `Result`, and the **feature layer** converts specific
  errors (`NoTokenAtCursor`, `UnknownDocument`, missing map entries) into the
  documented default responses. Degradation happens in one visible place per
  feature, not by ignoring errors deep in helpers.
- I/O errors during project scanning are logged and the file/path skipped
  (`03 §5–§8` semantics) — a helper like `fn ok_or_log<T>(r: Result<T>) -> Option<T>`
  keeps that pattern uniform.
- `panic!`/`unwrap` only for true invariants (poisoned locks, serializing our own
  structs). Request handling must never panic on client input.

## 3. Code style: composable, function-first

- **Functional core, imperative shell.** All extraction/analysis logic is pure
  functions over values: `fn extract_macros(text: &str, project: &str, path: &Path)
  -> Vec<Macro>`, `fn hover_content(ctx: &DbtContext, doc: &Document, pos: Position)
  -> String`. Filesystem access, process spawning and the LSP loop live in thin
  shell modules that *collect inputs* and *apply outputs*. This mirrors how the Go
  tests are written (pure functions over fixture strings) and makes insta snapshots
  trivial.
- **Side-effect boundaries per module:** `fs_scan.rs` (walks, reads → owned
  strings/paths), everything downstream takes `&str`/values. No function both reads
  the disk and parses.
- **Small files, one concern each.** Target ≤ ~300 lines per module; split by the
  layout in 00 (already fine-grained). A module that grows a second responsibility
  gets split, not sectioned with comments.
- **Data in, data out:** feature functions return response *values*; only the
  dispatcher serializes and writes. Builders like completion-item construction are
  free functions taking the maps they need (as in Go), not methods on `State` —
  `State` methods only orchestrate.
- Prefer iterator pipelines over index loops where they stay readable; no `clone()`
  to satisfy the borrow checker on hot paths (restructure instead).

## 4. SQL tokenization with `sqlparser`

The Go lexer is a single hand-rolled byte scanner over SQL-with-Jinja. `sqlparser`
cannot parse Jinja (`{{ ref('x') }}` is not SQL), so the port uses a **two-layer
lexer** in `src/analysis/parser/`.

> Scope note (06 §1): this two-layer lexer is the **live-document** path and must
> sit behind the `DocumentParser` trait (06 §3.5) so tree-sitter can replace the
> segmenter later. Scan-side files (macros, docs) are parsed with minijinja
> (06 §2), never with this pipeline's segmenter.

1. **Jinja segmenter** (`jinja.rs`, hand-rolled, small): one linear scan that splits
   the document into alternating segments: `Sql(span)` and `Jinja { kind: Expr({{}})
   | Stmt({%%}) | Comment({##}) , span }`. Tracks (line, column) per segment start.
   This is unavoidable custom code (~100 lines) and is *not* graph/SQL work.
2. **SQL segments → `sqlparser::tokenizer::Tokenizer`** with the dialect mapped from
   the profile (03 §7.2):

   | profile `type` | sqlparser dialect |
   |---|---|
   | `snowflake` | `SnowflakeDialect` |
   | `duckdb` | `DuckDbDialect` |
   | `bigquery` | `BigQueryDialect` |
   | `postgres` | `PostgreSqlDialect` |
   | anything else / `""` | `GenericDialect` |

   Use `tokenize_with_location()`; sqlparser locations are 1-based → subtract 1 and
   add the segment's base offset to produce the document-absolute 0-based positions
   the features need. Keyword classification comes from `Token::Word { keyword, .. }`
   — this **replaces the Go keyword tables** (02 §1.2) entirely.
3. **Jinja expression segments** are tokenized by the same small scanner into the
   dbt-relevant stream (idents, `.`, `(`, `)`, `,`, quotes/strings), and the
   **refinement pass from 02 §3 runs unchanged in spirit** over the combined stream:
   `ref('x')` → REF token, `source('a','b')` → SOURCE/SOURCE_TABLE, `{{ pkg.m( }}` →
   PACKAGE/MACRO, CTE tracking over the SQL tokens (WITH/IDENT/AS/parens).

Consequences (accepted deviations from Go, strictly better):

- Strings and comments are tokenized correctly (Go lexes `'customers'` as three
  tokens and doesn't know comments exist). The refinement pass therefore matches on
  *string tokens* inside `ref(...)` instead of the Go quote/ident/quote triple —
  02's token-pattern tables must be transposed accordingly when porting
  `parser_test.go` (the *outcomes* — which identifier becomes REF/SOURCE/etc. and at
  what position — stay identical; port the Go tests as behavior tests, not
  token-stream-dump tests).
- Keyword sets differ slightly from the two hand-copied Go tables, and dialects
  beyond snowflake/duckdb get real keyword handling. Hover/definition behavior is
  unaffected (keywords were only ever "not IDENT").
- Positions: keep everything in bytes internally (like Go, Q6) for v1 parity;
  the segmenter is the single place that later adds a byte↔UTF-16 conversion table
  per line (fixing Q6/Q7 in one spot).

The unified token model (replaces 02 §1 for the implementation):

```rust
pub struct Token { pub kind: TokenKind, pub text: SmolStrOrString, pub line: u32, pub column: u32 }
pub enum TokenKind {
    Word { keyword: sqlparser::keywords::Keyword },  // keyword == NoKeyword ⇒ IDENT
    Ref, Var, SourceName, SourceTable, Macro, Package, Config,  // refined dbt kinds
    StringLit, Number, Symbol(char), JinjaOpen(JinjaKind), JinjaClose(JinjaKind),
}
```

`TokenIndex` (per-line sorted vec + `partition_point` cursor lookup, prev-links as
indices) stays exactly as specced in 02 §3.4–3.6.

## 5. `petgraph` project graph

All graph-shaped state uses petgraph — nothing hand-rolled. New module
`src/analysis/graph.rs`:

```rust
pub enum NodeKind { Model, Seed, Source /* source.table */, Macro }
pub struct NodeData { pub kind: NodeKind, pub name: String, pub uri: PathBuf }
pub enum EdgeKind { Ref, Source, MacroCall }

pub struct ProjectGraph {
    graph: petgraph::stable_graph::StableDiGraph<NodeData, EdgeKind>,
    by_name: HashMap<(NodeKindKey, String), NodeIndex>,
}
```

- Built during context refresh (03 §1.2) as a **fourth parallel builder**: for every
  model file, run the lexer/refinement pass (§4) once and record its REF/SOURCE/
  MACRO tokens as edges `model → target`. The per-file token work is the same code
  path the editor uses, so it's tested for free.
- Uses, in order of adoption:
  1. **Incremental invalidation** (the performance core, §6): on save of one model,
     only that node's edges are rebuilt; the detail maps are patched, not rebuilt.
  2. **Dangling-ref diagnostics** (cheap, no Fusion needed): a `Ref` edge whose
     target node doesn't exist → `publishDiagnostics` warning. (New feature; ship
     behind the parity milestone.)
  3. Future features the Go server lacks: find-references for a model
     (incoming `Ref` edges), lineage/`+model+` style queries
     (`petgraph::visit::Bfs` up/downstream), cycle detection
     (`petgraph::algo::is_cyclic_directed`) as a diagnostic.
- Keep petgraph types out of public signatures: expose
  `fn dependents_of(&self, name: &str) -> impl Iterator<Item = &NodeData>` etc., so
  features stay decoupled from the graph library.

## 6. Runtime architecture & performance

### 6.1 Latency budget

| Operation | Budget | Why it holds |
|---|---|---|
| hover / definition / goToSchema | < 1 ms | HashMap lookups on an in-memory snapshot |
| completion | < 5 ms | item building over existing maps; no I/O ever |
| didChange → reparse one document | < 2 ms | single-file tokenize (sqlparser is µs-scale on model-sized SQL) |
| didOpen/didSave context refresh | off the request path | background task, snapshot swap |

Golden rule: **the request path never touches the filesystem and never waits on the
project scanner.** (The Go server violates this: didOpen/didSave block on a full
project rescan, and Fusion blocks the whole loop — Q9.)

### 6.2 Task layout (tokio, multi-thread runtime)

```
 stdin ──► reader task ──► dispatcher (owns Documents) ──► stdout writer task
                              │  reads Arc<DbtContext> snapshot (watch::Receiver)
                              ├─► refresh task  ── builds DbtContext+graph ──► watch::Sender
                              └─► fusion task(s) ── tokio::process ──► writer channel
```

- **Reader task:** async-reads stdin (`tokio::io::stdin`), frames per 01 §1.3, sends
  `(method, bytes)` over an `mpsc` channel.
- **Dispatcher task:** single owner of `Documents` (no lock needed); handles every
  request inline — that's fine because handlers are sub-millisecond pure functions
  over the snapshot. Request ordering is preserved (LSP requires didChange before
  the completion that follows it).
- **Writer task:** owns stdout; receives already-encoded frames over `mpsc`. This is
  the interleaving guard that Go lacks (01 §4) — everyone (dispatcher, fusion,
  refresh-triggered diagnostics) writes through this channel.
- **`DbtContext` snapshots:** the refresh task builds a complete
  `Arc<DbtContext>` (detail maps + `ProjectGraph`) and publishes it through
  `tokio::sync::watch`. Handlers call `borrow().clone()` (an `Arc` clone, ~ns) and
  work on an immutable snapshot — zero contention, no torn reads.
- **Refresh scheduling:** didOpen/didSave *send a refresh request* (debounced:
  coalesce requests arriving within ~200 ms; drop if one is already running and
  queue exactly one follow-up). First refresh on `initialize` so hover works before
  the first save. Inside a refresh: the three builders of 03 §1.2 plus the graph
  builder run via `std::thread::scope`/`tokio::task::spawn_blocking` (CPU + fs
  work doesn't belong on the async workers); directory walking and per-file parsing
  parallelize per project/package.
- **Incremental refresh (fast path):** on didSave of a `.sql` model file, skip the
  full walk — re-extract only that file (macros/graph edges/detail-map entry) and
  publish a patched snapshot; full rescans only for YAML/`dbt_project.yml` changes
  or on open. (Behavioral parity is preserved: same maps, fresher sooner.)
- **Fusion:** `tokio::process::Command`, stdout/stderr read as async line streams,
  diagnostics forwarded to the writer channel exactly per 04 §2.4 cadence. Per-URI:
  if a compile is already running for that URI, abort it (kill child) and start the
  new one — fixes Q9 without changing published payloads.
- **Blocking is allowed** in `spawn_blocking`/scoped threads only; the async side
  does I/O and coordination exclusively.

### 6.3 Micro-level

- Compile every regex once (`std::sync::LazyLock<Regex>`).
- Reuse allocations in the frame reader (one growable `Vec<u8>` buffer).
- Intern nothing prematurely; `String` keys in maps are fine at dbt-project scale
  (thousands of models). Measure before optimizing further: add a
  `--debug`-gated per-request timing log line (handler + serialize + write µs).
- `cargo build --release` with `lto = "thin"`, `codegen-units = 1` in
  `[profile.release]`.

## 7. Testing

Stack: `rstest` for parameterized cases, `insta` for snapshots,
`pretty_assertions` for readable diffs. Fixtures: the Go repo's `testdata/`
copied verbatim.

- **Port Go table tests as `rstest` cases.** Every `tests := []struct{...}` table in
  the Go repo (`lexer_test.go`, `parser_test.go`, `resolve_env_vars_test.go`,
  `get_completion_items_test.go`, …) becomes `#[rstest] #[case(...)]` functions —
  same inputs, same expected outcomes (for the parser: expected *refined kinds and
  positions*, per §4's note, not raw token dumps).
- **Snapshot the big outputs with `insta`:**
  - full token stream of representative model files → `assert_debug_snapshot!`;
  - `DbtContext` built from `testdata/jaffle_shop_duckdb` (maps sorted for
    determinism) → `assert_yaml_snapshot!`;
  - raw JSON responses for the acceptance checklist in 04 §5
    (`assert_json_snapshot!` on the serialized response) — this doubles as the
    wire-format regression suite;
  - the project graph as sorted edge list `(from, kind, to)`.
- **`pretty_assertions::assert_eq`** imported in every test module for the non-
  snapshot comparisons.
- **End-to-end:** one integration test spawns the compiled binary, speaks framed
  LSP over stdio (initialize → didOpen → hover/definition/completion → shutdown/
  exit), snapshots each response. This is the Rust half of the Go-vs-Rust parity
  harness (00 §porting-order step 9).
- Pure-function discipline from §3 is what keeps all of this cheap: no mocking
  framework is needed anywhere.

## 8. File layout delta vs. 00

Changes to the tree in 00 §"Proposed crate layout":

```
src/error.rs                      # §2 (new; + Error::Jinja variant per 06 §2.4)
src/analysis/parser/mod.rs        # DocumentParser trait + ParsedDocument (06 §3.5)
src/analysis/parser/jinja.rs      # §4 segmenter (new; live-document path)
src/analysis/parser/jinja_scan.rs # minijinja scan-side extraction (06 §2)
src/analysis/parser/lexer.rs      # thin: segmenter + sqlparser tokenizer + merge
src/analysis/parser/ts/           # phase 2: tree-sitter ffi.rs/parser.rs/queries (06 §3)
src/analysis/graph.rs             # §5 (new)
src/server/                       # replaces flat main-loop code:
├── mod.rs                        #   task wiring (§6.2)
├── dispatcher.rs                 #   method match → feature calls
├── reader.rs / writer.rs         #   framed stdin/stdout tasks
└── refresh.rs                    #   debounced context refresh + snapshot publish
tests/
├── e2e_lsp.rs                    # §7 end-to-end
└── snapshots/                    # insta snapshots (committed)
```

Everything else in 00–04 (types, algorithms, regexes, quirks, acceptance checklist)
stands as written.
