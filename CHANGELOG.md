# Changelog

All notable changes to cogitator, the dbt language server
(a Rust rewrite of j-clemons/dbt-language-server).

The Rust rewrite has no releases yet; everything sits under
[unreleased]. The v0.1.x–v0.4.x tags in the git history belong to the
upstream Go project and are ignored here — the first release of this
fork should start at v0.5.0 to stay clear of them.

<!-- git-cliff: end of header -->
## [unreleased]

### 🚀 Features

- *(server)* Implement the Rust port as a Cargo workspace
- *(schemas)* Generate dbt schema types via typify::import_types!
- *(lsp)* Implement the feature-request backlog
- *(parser)* Add the tree-sitter document backend
- *(analysis)* Add a project scan cache and column lineage
- *(completion)* Add write assist and full Snowflake support
- *(completion)* Turn completion into a grammar-aware engine
- *(lsp)* Code-lens batch, dbt execution and actionable errors
- Add Windows support
- *(profiles)* Load profiles.yml from configurable locations
- *(nvim)* Add the first-party Neovim plugin
- *(lsp)* Add the model-graph browser (dbt.modelGraph)
- *(emacs)* Add eglot/lsp-mode support with the graph browser
- *(nvim)* Add :DbtProfiles and fix the shutdown response shape
- *(zed)* Add a first-party Zed extension
- *(profiles)* Add the Snowflake connection auth manager
- *(lsp)* Extract-model-to-YAML with versions and go-to-tests
- *(server)* Report status via $/progress and a dbt.status command
- *(lsp)* Search tags and selectors
- *(nvim)* Dbt-docs-style project DAG with columns (Neovim only)

### 🐛 Bug Fixes

- *(lsp)* Support rootUri/workspaceFolders and defuse upgrade
- *(install)* Point the installer at this project, not upstream Go

### 📚 Documentation

- *(specs)* Add the Rust-port implementation specs
- *(specs)* Bind minijinja for macros and plan tree-sitter via C FFI
- *(specs)* Require crate-based lexing and tracing/OTLP observability
- *(specs)* Add DuckDB 2.0 parser and the Zig C/C++ toolchain
- *(specs)* Bind zero-copy rules and typify-generated schema types
- *(specs)* Reconcile the spec stack after amendments
- *(specs)* Split into a workspace with mold and sccache
- Track missing LSP features in FEATURE_REQUESTS.md
- *(specs)* Add the cogitator.nvim implementation spec
- *(readme)* Origin story and the full feature inventory
- *(changelog)* Generate CHANGELOG.md with git-cliff
- *(changelog)* Refresh for the tag/selector search and DAG view

### 🚜 Refactor

- Move analysis and server to functional core / imperative shell
- Split oversized modules and fix lost yaml wiring
- Rename the binary to cogitator
- Unify crate naming as cogitator-* and add a justfile

### ⚙️ Miscellaneous Tasks

- Remove the Go implementation
- *(tooling)* Add prek hooks, git-cliff config and recipes
- *(github)* Issue templates for bugs and feature requests
