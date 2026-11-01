# Agent Guidelines for dbt-language-server

Rust implementation of the dbt language server (a Cargo workspace; the former
Go implementation lives in git history up to commit `724b0f2` and is fully
specified in `specs/rust-port/`).

## Build/Test Commands
- **Build**: `cargo build --workspace`
- **Test all**: `cargo test --workspace`
- **Test one crate**: `cargo test -p dbt-ls-parser`
- **Test one case**: `cargo test -p dbt-ls-analysis ref_source_var_refined`
- **Lint/format**: `cargo clippy --workspace` (zero warnings expected), `cargo fmt --all`
- **DuckDB feature** (bundled C++ build, slow): `cargo check -p dbt-language-server --features duckdb-parser`
- **Release cross-builds**: `cargo-zigbuild` via the Zig toolchain (specs/rust-port/07 §5; CI: .github/workflows/release.yml)

## Code Style (binding rules in specs/rust-port/05 §2–§3, 08 §1)
- **Functional core, imperative shell**: extraction/feature logic is pure
  functions over values; filesystem, process and LSP I/O stay in thin shell
  modules (`fs_scan`, the `server/` tasks).
- **Errors**: one `Error` enum (`dbt-ls-error`) with `thiserror`; `anyhow`
  only in `main()`/task top-levels. Degradation is explicit at the feature
  layer, never silent deep in helpers.
- **Zero-copy**: tokens are byte spans into `Arc<str>` document text; inbound
  params borrow via `Cow`; a `clone()`/`to_string()` on a hot path must name
  the owner boundary it feeds (08 §1.5).
- **No hand-written lexers**: minijinja machinery + sqlparser only (05 §4).
- **Files stay small**: ≤ ~300 lines per module; split by concern.
- **Logging**: `tracing` spans/events only; nothing but LSP frames on stdout.

## Environment Variables
- **DBT_PROFILES_DIR**: profiles.yml directory when no workspace root is known
  (otherwise `<root>/.dbt/profiles.yml` is used — Q5 in specs/rust-port/00).

## Project Structure (specs/rust-port/09 §1)
- `crates/dbt-ls-error` — error type
- `crates/dbt-ls-lsp` — protocol types + Content-Length framing
- `crates/dbt-ls-dialects` — dialect + bundled SQL function docs
- `crates/dbt-ls-schemas` — typify-generated dbt schema types (`schemas/dbt/`) + saphyr position side-tables
- `crates/dbt-ls-parser` — minijinja+sqlparser pipeline, refinement, TokenIndex, jinja_scan
- `crates/dbt-ls-analysis` — context builders, feature functions, petgraph project graph
- `crates/dbt-ls-duckdb` — Jinja masking + feature-gated DuckDB 2.0 parse diagnostics
- `crates/dbt-language-server` — the binary: CLI, tokio tasks, dispatcher, fusion, observability
- `schemas/dbt/` — vendored dbt-jsonschema (pristine + `processed/` for typify; regenerate with `preprocess.py`)
- `specs/rust-port/` — the binding implementation specs (00–09)
- `testdata/` — shared fixtures (jaffle_shop_duckdb)
