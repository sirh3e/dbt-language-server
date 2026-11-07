# 07 — DuckDB 2.0 Native Parser & the Zig Toolchain for C/C++ Builds

Binding decisions, same status as 05/06. Two decisions:

1. For the **duckdb dialect**, the authoritative SQL parser is **DuckDB's own
   parser as shipped in DuckDB 2.0.0**, embedded in-process. `sqlparser` remains
   the token layer for every dialect (05 §4); DuckDB's parser is used where real
   *parsing* is needed (parse diagnostics, later AST features).
2. **All C/C++ compiled during the build goes through the Zig toolchain**
   (`zig cc` / `zig c++`), and release cross-compilation uses `cargo-zigbuild`.

## 1. Why DuckDB's own parser

DuckDB 2.0.0 (October 2026) replaced the old PostgreSQL-derived parser with a
**new PEG parser**. Consequences that matter to this LSP:

- 2.0 accepts a large amount of syntax that generic SQL parsers (sqlparser's
  `DuckDbDialect` included) do not fully model: `FROM`-first queries,
  `GROUP BY ALL` / `ORDER BY ALL`, `QUALIFY`, `SELECT * EXCLUDE/REPLACE`,
  `COLUMNS('regex')`, prefix aliases (`total: sum(amount)`), Python-style lambdas
  (`lambda x: x + 1` — the `x -> x + 1` arrow form is *rejected* in 2.0),
  list comprehensions (`[x * 2 FOR x IN l IF x > 1]`), `LIMIT 50%`,
  lateral column alias reuse, `ASOF`/`POSITIONAL JOIN`, `INSERT ... BY NAME`,
  data-modifying CTEs, `$name` variables, dot chaining, `VARIANT`.
- 2.0 changed token/literal semantics (`X'FF'` is a BLOB, `B'101'` a BITSTRING,
  `~` is partial regex match, `!` postfix binds tighter than `*`).
- `SET enable_legacy_parser = true` restores the old parser for a session —
  DuckDB documents it as a diagnostic, not a fix. **This server always targets
  the 2.0 PEG parser and never sets it.**
- DuckDB `Parser Error`s carry a position — exactly what diagnostics need.

Division of labor (amends 05 §4 for `dialect == duckdb`; other dialects
unchanged):

| Layer | Crate/engine | Used for |
|---|---|---|
| Tokens (hover/definition/completion positions) | `sqlparser` (`DuckDbDialect`) | unchanged — tokenization is grammar-light and unaffected by most PEG-level changes |
| Parsing (syntax validity, AST) | embedded DuckDB 2.0 parser (this file) | parse diagnostics; later AST-driven features |

## 2. Embedding (feature-gated)

```toml
[dependencies]
duckdb = { version = "2", features = ["bundled", "json"], optional = true }  # pin 2.0.x

[features]
duckdb-parser = ["dep:duckdb"]
```

- `bundled` compiles the DuckDB amalgamation (large C++ build — §4 routes it
  through `zig c++`; this is why the integration is a cargo feature, so the core
  LSP still builds in seconds without it). Builds with the feature are the
  default for releases.
- One **lazy, in-memory** connection (`duckdb::Connection::open_in_memory()`),
  created on first use and only when `dbt_context.dialect == "duckdb"`; owned by
  a dedicated `spawn_blocking` worker fed by an `mpsc` channel (DuckDB calls are
  blocking; they never run on the async workers — 05 §6.2 rules apply). No
  database file is ever opened/written, so 2.0 storage-compatibility concerns
  don't apply.
- Parser access is through SQL, no C-API FFI of our own:
  `SELECT json_serialize_sql($1)` — returns the statement's AST as JSON, or an
  object with `"error": true`, an error message and a **position** when parsing
  fails. (Exact call shape and error fields pinned against duckdb-rs 2.0 at
  implementation time; `duckdb -manual json_serialize_sql` and
  https://duckdb.org/docs/2.0/ are the references.)

## 3. Feature 1: parse diagnostics for dbt-duckdb projects

Runs on didOpen/didSave (and debounced didChange), per open `.sql` model, when
the feature is compiled in and the dialect is duckdb. Publishes via the standard
diagnostics channel (01 §2.3), `source: "duckdb 2.0"`. This gives duckdb users
syntax checking **without dbt Fusion** (04 §2 remains the Fusion path and takes
precedence when enabled — don't double-publish: Fusion on ⇒ DuckDB parse
diagnostics off).

### 3.1 Position-preserving Jinja masking

DuckDB must receive plain SQL, but LSP positions must map back to the template.
Mask Jinja **byte-for-byte** (every replacement has exactly the original byte
length, so offsets and line/column are identity-mapped — reuses the minijinja
token spans from 05 §4, no second scan):

| Template construct | Replacement (same byte length) |
|---|---|
| `{{ … }}` expression | an identifier: `_` followed by `j`-padding, e.g. `{{ ref('stg_orders') }}` → `_jjjjjjjjjjjjjjjjjjjjjj` |
| `{% … %}` statement | spaces (newlines inside are kept, preserving line numbers) |
| `{# … #}` comment | spaces (newlines kept) |

### 3.2 Confidence rule

Masking `{% if %}`/`{% for %}` control blocks with spaces can leave SQL that is
invalid only because a branch was blanked. To avoid noise:

- Template has **no** `{% … %}` statement blocks (or only `config`/`set` blocks):
  parse errors are published as Error-severity diagnostics at the reported
  position (DuckDB positions are offsets into the masked text = offsets into the
  template; convert to 0-based line/character via 03 §7.5).
- Template **has** control blocks: publish nothing on parse failure (silently
  skip). Correctness over coverage; revisit after real-world feedback.
- `json_serialize_sql` handles statements it can parse; non-SELECT or
  multi-statement edge cases that error structurally (not positionally) are
  logged (tracing event, 05 §6.3) and skipped.

## 4. Feature 2 (later): AST-driven features

The serialized AST (JSON) from `json_serialize_sql` is a structured view of the
*rendered* model: column lists, table references, CTE structure. Candidate uses,
each specced separately when picked up: column-name completion inside a model,
better CTE/def-token information than the token heuristic (02 §3), and
graph edges (05 §5) extracted from the AST instead of refined tokens for duckdb
projects. Out of scope for parity.

## 5. Zig toolchain for all C/C++ compilation

Everything in this project that compiles C or C++ uses Zig as the compiler
driver — no system `cc`, no MSVC, no cross-compiler zoo:

- **What is compiled:** the tree-sitter C runtime + vendored grammar sources
  (06 §3.2, via the `cc` crate in `build.rs`) and the bundled DuckDB C++
  amalgamation (§2, built by duckdb-rs's own `cc` usage).
- **Mechanism:** the `cc` crate honors the standard env vars — CI and dev
  builds set

  ```sh
  CC="zig cc"  CXX="zig c++"  AR="zig ar"
  ```

  (per-target variants like `CC_x86_64_unknown_linux_gnu` where needed).
- **Release cross-compilation: `cargo-zigbuild`** replaces the Go repo's
  `build-all-platforms.sh` story (04 §4). Target matrix, from any single build
  host:

  | Target | Note |
  |---|---|
  | `x86_64-unknown-linux-gnu.2.17` / `aarch64-unknown-linux-gnu.2.17` | glibc floor pinned via zigbuild's `.<glibc>` suffix |
  | `x86_64-unknown-linux-musl` / `aarch64-unknown-linux-musl` | fully static |
  | `x86_64-apple-darwin` / `aarch64-apple-darwin` | needs the macOS SDK present (`SDKROOT`) |
  | `x86_64-pc-windows-gnu` | windows-msvc stays out of the matrix (zig targets gnu) |

- **Pinning:** one Zig version for the whole project (e.g. a `ZIG_VERSION` in CI
  and a line in the README; zig is downloaded, not assumed installed). Compiler
  upgrades are explicit commits.
- **Release surface** (release.yml): the matrix's binaries ship as
  `cogitator-<os>-<arch>.tar.gz` (linux/darwin) and `.zip` (windows),
  each carrying the binary + LICENSE + README, alongside one
  `SHA256SUMS` and the packaged VS Code extension
  (`cogitator-<version>.vsix`, versioned from the released
  Cargo.toml). Asset names carry **no version**: they are the
  installers' `releases/latest/download/<asset>` contract. A
  `workflow_dispatch` with a `tag` input rebuilds that tag's tree and
  replaces its release; without the input it is a dry-run build.
- `sccache`/zig's own caching keeps the DuckDB amalgamation from dominating CI
  (sccache setup in 09 §4); the `duckdb-parser` feature and the dedicated
  `dbt-ls-duckdb` workspace crate (09 §1) keep it out of the dev inner loop
  entirely.

## 6. Cross-file deltas

- 05 §1 manifest: optional `duckdb` dependency + `duckdb-parser` feature (§2).
- 05 §4: for duckdb dialect, sqlparser = tokens only; DuckDB parser = parsing
  (§1 table).
- 06 §3.2: grammar C sources are compiled with `zig cc` through the `cc` crate
  (§5).
- 04 §4: release builds use `cargo-zigbuild` with §5's target matrix.
- 00: spec index + dependency-mapping rows.
