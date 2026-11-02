# Agent Guidelines for cogitator (dbt language server)

Rust implementation of the dbt language server (a Cargo workspace; the former
Go implementation lives in git history up to commit `724b0f2` and is fully
specified in `specs/rust-port/`).

## Build/Test Commands
- **justfile**: `just --list` — `just build` / `test` / `fmt` / `lint` / `check` (CI set) / `release` / `duckdb-check` / `gen-schemas`
- **Build**: `cargo build --workspace`
- **Test all**: `cargo test --workspace`
- **Test one crate**: `cargo test -p cogitator-parser`
- **Test one case**: `cargo test -p cogitator-analysis ref_source_var_refined`
- **Lint/format**: `cargo clippy --workspace` (zero warnings expected), `cargo fmt --all`
- **DuckDB feature** (bundled C++ build, slow): `cargo check -p cogitator --features duckdb-parser`
- **Release cross-builds**: `cargo-zigbuild` via the Zig toolchain (specs/rust-port/07 §5; CI: .github/workflows/release.yml — linux-amd64, darwin-amd64/arm64, windows-amd64.exe via the `x86_64-pc-windows-gnu` target)
- **CI tests**: .github/workflows/ci.yml runs `cargo test --workspace` on Ubuntu, Windows and macOS plus fmt/clippy
- **Windows rules**: never hand-strip `file://` — URIs go through `cogitator_lsp::uri_to_fs_path` (handles `file:///c%3A/…` drive URIs) and paths become URIs via `cogitator_lsp::fs_path_to_uri`; home lookup uses `cogitator_analysis::util::home_dir()` (`$HOME` → `%USERPROFILE%`); compare paths as `Path` (components), not strings

## Code Style (binding rules in specs/rust-port/05 §2–§3, 08 §1)
- **Functional core, imperative shell**: extraction/feature logic is pure
  functions over values; filesystem, process and LSP I/O stay in thin shell
  modules (`cache::FileCache` at the scan boundary, the `server/` tasks).
- **Errors**: one `Error` enum (`cogitator-error`) with `thiserror`; `anyhow`
  only in `main()`/task top-levels. Degradation is explicit at the feature
  layer, never silent deep in helpers.
- **Zero-copy**: tokens are byte spans into `Arc<str>` document text; inbound
  params borrow via `Cow`; a `clone()`/`to_string()` on a hot path must name
  the owner boundary it feeds (08 §1.5).
- **No hand-written lexers**: minijinja machinery + sqlparser only (05 §4).
- **Files stay small**: ≤ ~300 lines per module; split by concern.
- **Logging**: `tracing` spans/events only; nothing but LSP frames on stdout.

## Environment Variables
- **DBT_PROFILES_DIR**: profiles.yml directory. Full resolution order
  (`util::profiles_candidates`): `--profiles-dir` /
  `initializationOptions.profilesDir` override → `$DBT_PROFILES_DIR` →
  `<root>/profiles.yml` → `<root>/.dbt/profiles.yml` (Q5) →
  `~/.dbt/profiles.yml`; the first file whose profile resolves a target wins.

## Project Structure (specs/rust-port/09 §1)
- `crates/cogitator-error` — error type
- `crates/cogitator-lsp` — protocol types + Content-Length framing
- `crates/cogitator-dialects` — dialect + bundled SQL function docs
- `crates/cogitator-schemas` — typify-generated dbt schema types (`schemas/dbt/`) + saphyr position side-tables
- `crates/cogitator-parser` — minijinja+sqlparser pipeline, refinement, TokenIndex, jinja_scan
- `crates/cogitator-analysis` — context builders, feature functions, petgraph project graph
- `crates/cogitator-duckdb` — Jinja masking + feature-gated DuckDB 2.0 parse diagnostics
- `crates/cogitator-treesitter` — tree-sitter document backend (vendored grammar in `third_party/`)
- `crates/cogitator` — the binary: CLI, tokio tasks, dispatcher, fusion, observability
- `schemas/dbt/` — vendored dbt-jsonschema (pristine + `processed/` for typify; regenerate with `preprocess.py`)
- `specs/rust-port/` — the binding implementation specs (00–09)
- `testdata/` — shared fixtures (jaffle_shop_duckdb)
