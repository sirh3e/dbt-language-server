# Changelog

All notable changes to cogitator, the dbt language server
(a Rust rewrite of j-clemons/dbt-language-server).

Releases of this fork start at v0.5.0: the v0.1.x–v0.4.x tags in
the git history belong to the upstream Go project and are ignored
here.

<!-- git-cliff: end of header -->
## [0.5.0] - 2026-11-07

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
- *(install)* Windows installer, shell script removed
- *(lsp)* Add dbt.modelInfo for one-round-trip model detail
- *(nvim)* Add :DbtBrowse, the yazi-style project browser
- *(lsp)* Advertise completion trigger characters
- *(analysis)* Source entries carry database, schema, identifier and loader
- *(analysis)* Profiles targets expose database and schema
- *(server)* Dbt.relation resolves the relation under the cursor
- *(server)* Dbt.compiledSql serves compiled SQL over the wire
- *(server)* Dbt.parse re-parses the project
- *(server)* TextDocument/codeAction serves dbt command actions
- *(editors)* :DbtRelation and cogitator-relation surface dbt.relation
- *(server)* Dbt.setProfilesDir switches the profiles directory live
- *(nvim)* Persisted project/profile manager and live profiles switch
- *(emacs)* Cogitator-set-profiles-dir switches the profiles dir live
- *(server)* Target-switch code actions in profiles.yml
- *(nvim)* :DbtQuery — bounded SELECT on the relation under the cursor
- *(emacs)* Query workflow, tags/selectors/profiles pickers and doctor
- *(server)* In-memory derived-data caches and dbt.cache
- *(editors)* :DbtCache and cogitator-cache expose the derived caches
- *(analysis)* Dbt.modelInfo carries description and tags
- *(nvim)* Explorer info tab — description, tags, relation, columns, test jumps
- *(treesitter)* SQL structure from the tree-sitter-sequel grammar
- *(observability)* Instrument the long-running units
- *(observability)* OTLP metrics alongside the trace pipeline
- *(error)* Guidance builder and closest_matches helper
- *(server)* Offload slow handlers with real $/cancelRequest
- *(server)* Did-you-mean guidance for commands, models and targets
- *(analysis)* Dangling source()/var() diagnostics with suggestions
- *(analysis)* Positioned YAML schema errors and unknown-key warnings
- *(server)* Cogitator.yml config core and hot-tunable limits
- *(server)* Hot-reload cogitator.yml — watcher task, dbt.reloadConfig, --config
- *(lsp)* Diagnostic data, tags and relatedInformation
- *(lsp)* Edit-bearing code actions with kinds and context diagnostics
- *(analysis)* Did-you-mean quickfixes from diagnostic data
- *(server)* Cogitator.yml lint and warehouse sections with shared schema
- *(analysis)* Cogitator.yml diagnostics, completion and hover
- *(analysis)* Opt-in style diagnostics behind the lint section
- *(analysis)* Lenient dbt artifact readers
- *(server)* Artifact-backed lenses and diagnostics
- *(server)* Dbt.sourceFreshness command and freshness lenses
- *(server)* Dbt job registry with cancel and arbitrary selectors
- *(analysis)* Graph module split with transitive traversal helpers
- *(analysis)* Exposures, metrics and semantic models as graph citizens
- *(analysis)* Dead-model detection with report command
- *(server)* Dbt.impact with column granularity
- *(analysis)* Package awareness for two-arg ref and dbt deps drift
- *(lsp)* Inlay hints, document links and linked editing
- *(server)* Workspace diagnostics with revision result ids
- *(analysis)* Sql formatting with jinja segmentation
- *(server)* Dbt.expandJinja render preview
- *(analysis)* Convert-to-ref, test scaffolds and inherited descriptions
- *(duckdb)* Warehouse-backed source column completion
- *(treesitter)* Conservative incremental reparse
- *(cache)* Persist the scan cache across sessions
- *(server)* Dbt.doctor health report
- *(schemas)* Vendor official dbt artifact schemas, typify them leniently
- *(analysis)* Catalog.json surface — columns for every adapter
- *(analysis)* Go-to-definition for SQL functions
- *(server)* Editor-visible tracing via window/logMessage
- *(server)* $/setTrace, initialize trace and log.client_level
- *(analysis)* Scan snapshots, singular tests and analyses
- *(analysis)* Model governance — access, groups, contracts, unit tests
- *(config)* Cogitator.yml JSON Schema export
- *(server)* Dbt.preview — dbt show result rows in the editor
- *(server)* PrepareRename and the macro call hierarchy
- *(perf)* Criterion benches and cargo-fuzz scaffolding
- *(vscode)* First-party thin-client extension
- *(server)* Multi-root workspace folders
- Doc('…') blocks — hover, definition, completion, dangling-doc (#4)

### 🐛 Bug Fixes

- *(lsp)* Support rootUri/workspaceFolders and defuse upgrade
- *(install)* Point the installer at this project, not upstream Go
- *(changelog)* Keep upstream Go releases out of the changelog
- *(install)* Restore the unix installer for this project
- *(server)* Compile duckdb push diagnostics under the feature flag
- *(server)* One id counter for all server→client requests
- *(server)* Drop completed fusion job handles
- *(treesitter)* Well-formed comments no longer swallow trailing code
- *(observability)* Per-layer filters, flush on every exit, true names
- *(server)* No request is ever silently dropped
- *(server)* Write dbt.preview captures atomically
- Windows CI — verbatim paths break the profiles-file equality gate (#2)
- Position requests resolve against the owning root's context (#3)

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
- *(agents)* Refresh AGENTS.md for the current tree
- *(contributing)* Define the feature workflow and release cycle
- Document dbt.modelInfo and the :DbtBrowse browser
- Document relation, compiledSql, parse and code actions
- Editor coverage matrix and spec-truth sweep
- *(treesitter)* Record the jinja2 grammar's deliberate incompleteness
- *(specs)* The Reply handler contract and real reader layout
- *(specs)* Concurrent task layout, cancellation and the offload set
- *(specs)* Binding error-guidance style
- Feature brainstorm for future LSP work (spec.md)
- *(specs)* Graduate the spec.md feature batch
- *(specs)* Supersede the no-vendored-artifacts note; ci checks schema freshness
- System-test workflow riders
- *(specs)* The VS Code suite in the testing workflow

### ⚡ Performance

- *(analysis)* Memoize the project DAG depths at first use

### 🚜 Refactor

- Move analysis and server to functional core / imperative shell
- Split oversized modules and fix lost yaml wiring
- Rename the binary to cogitator
- Unify crate naming as cogitator-* and add a justfile
- *(lsp)* Typed capability, workspace-edit, progress and completion-data wire types
- *(completion)* Typed CompletionItem.data payloads
- *(analysis)* Typed graph, model-info and tests payloads; split navigation.rs
- *(analysis)* Rename returns a typed WorkspaceEdit
- *(analysis)* Typed dbt.profiles report
- *(server)* Typed status, setTarget, applyEdit and progress payloads
- *(nvim)* Browse compiled tab fetches dbt.compiledSql
- *(nvim)* Shared ui/actions/render core modules
- *(nvim)* Explorer package, layout engine, LuaLS annotations
- *(error)* Retire never-constructed variants
- *(server)* ClientLink is the single send boundary
- *(analysis)* Documents store Arc<Document>
- *(server)* Reply effect model, Snapshot, and pure handler modules
- Every user-facing message goes through Guidance
- *(server)* Split the dispatcher into six focused modules
- *(server)* Extract the refresh task and split the config module
- *(tests)* Themed e2e suites over a shared harness
- *(analysis)* Split lineage, completion providers and refinement
- Derived-cache artifacts surface and context types split out
- *(analysis)* Artifact readers through the generated types

### 🧪 Testing

- *(testdata)* Staging model selecting from the stripe source
- *(server)* Hot-reload e2e on a temp fixture + config docs
- Refinement, wire-shape and editor-surface coverage — two real fixes
- *(e2e)* Fake-dbt bin and deadline-based harness v2
- *(nvim)* Deterministic headless suite on shared helpers
- *(e2e)* Concurrency suite onto harness v2, cross-platform jobs
- *(e2e)* Core and graph suites onto harness v2
- *(e2e)* Config, artifacts and editing suites onto v2; drop legacy harness
- *(emacs)* Deterministic batch suite on shared helpers
- *(system)* System_editors orchestrator and just system-test
- *(vscode)* Extension-host suite on the deterministic contract

### ⚙️ Miscellaneous Tasks

- Remove the Go implementation
- *(tooling)* Add prek hooks, git-cliff config and recipes
- *(github)* Issue templates for bugs and feature requests
- Shellcheck and rust/c-gated builds
- *(nix)* Declarative dev environment from one tooling source
- Move actions off deprecated Node.js 20
- *(nvim)* Lua tooling — stylua config, just recipes, LuaLS workspace
- *(nvim)* Luacheck linting config and recipe
- *(emacs)* Elisp lint via byte-compile with warnings as errors
- *(just)* Group the recipes for a structured listing
- Run the editor system suites on ubuntu
- Cover the VS Code suite in the editors job
- *(just)* The release flow lands through a pull request
