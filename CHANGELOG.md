# Changelog

All notable changes to cogitator, the dbt language server
(a Rust rewrite of j-clemons/dbt-language-server).

Releases of this fork start at v0.5.0: the v0.1.x–v0.4.x tags in
the git history belong to the upstream Go project and are ignored
here.

<!-- git-cliff: end of header -->
## [0.8.0] - 2026-11-27

### 🚀 Features

- *(completion)* Rank alias.column above globals, from a sqlparser scope (#101)
- *(completion)* A column name resolver over the project's schema snapshot (#106)
- Authoring actions, column reach, history and cost, typed checks and completion (#115)
- A formatter on sqlfluff's parser, user-defined functions, invalid models as you type, tests' and incremental strategies' SQL, CTEs without the warehouse, and the lineage suite: alias navigation, inline lineage hints, a column-lineage dialog, rename along lineage, the column graph and its cache (#116)
- *(lineage)* Dbt.columnLineageGraph walks the column graph; retire the plugin's walk (#122)
- *(zed)* The dbt menu as the server's dialogs (#123)

### 🐛 Bug Fixes

- *(test)* Wait for didOpen's refresh before the cached-plan push check (#114)

### ⚙️ Miscellaneous Tasks

- *(dev)* A real Zed in the dev shell, and `just zed-smoke` (#124)
## [0.7.0] - 2026-11-25

### 🚀 Features

- *(lsp)* Workspace file operations and textDocument/declaration (#73)
- *(lsp)* Dispatched macro implementations and macro call checks (#74)
- *(analysis)* Output-column types, data_type checks and type hovers (#75)
- *(lsp)* Rename a model's output column across the whole project (#76)
- *(lsp)* Rename, references and highlights of file-local names (#77)
- *(lsp)* Preview breaking changes against the last built schema (#78)
- *(lsp)* Preview the rows of a CTE (dbt.previewCte) (#79)
- *(selectors)* One engine for dbt's node-selection language (#80)
- *(nvim)* References, macros and rename in the yazi dialogs (#84)
- *(nvim)* A real project's shape, and dbt node selection in the navigator (#72, #82)
- Syntax errors as you type, and cogitator lint / format for CI (#87)
- A column-lineage site (cogitator lineage-site) and a live lineage view in VS Code (#88)
- Sync schema YAML with the SQL project-wide (dbt.syncYaml, cogitator yaml-sync) (#89)
- *(nvim)* Colours, examples and file jumps for the <leader>d keys (#90)
- A model's dependencies dialog, and navigation from anywhere in a call (#91)

### 🚜 Refactor

- *(nvim)* Split the plugin's three largest modules, share three helpers (#85)
- *(schemas)* Read dbt's schemas from the shared dbt-schemas crate (#93)

### ⚙️ Miscellaneous Tasks

- *(release)* V0.7.0 (#94)
## [0.6.0] - 2026-11-22

### 🚀 Features

- *(nvim)* UI foundation for graph colours and motions (#12)
- *(nvim)* Yazi-style walking and vim motions in the browsers (#13)
- *(nvim)* Node navigation and dbt-docs colours in :DbtDag (#14)
- *(ir)* HIR, MIR and LIR query pipeline with optimisation advice (#35)
- *(actions)* Quickfixes, refactors and dbt.deps for code actions (#37)
- *(rules)* DBSP incrementality, splitting at the frontier, and consumer parity (#38)
- *(nvim)* The dbt components spec's missing parts — column-lineage graph, syntax tree, grammar install and more (#39)
- *(nvim)* Nothing waits on the server; large DAGs paint in slices (#40)
- *(nvim)* Fzf-style pickers and a yazi-style navigator (#41)
- *(nvim)* The explorer as upstream | model | downstream over dbt SQL | compiled SQL (#43)
- Tree-sitter-dbt highlighting from the server, and ast-grep for dbt (#46)
- Highlighting for macros, tests and YAML; dbt.nodeInfo (epic #58) (#59)
- *(vars)* Values inline, definitions by scope, overrides, references, YAML colouring (#60)
- *(navigation)* References, rename and call hierarchy for sources, macros, docs and columns (#61)
- *(yaml)* Schema-driven properties completion, add missing columns, unproduced-column warning (#62)
- *(lsp)* Coverage and run-this-test lenses, unit-test scaffold (#63)
- *(plan)* CTE row counts inline, and dbt.incrementalDiff (#64)
- *(server)* YAML semantic tokens, dbt.unresolved, hover on compiled SQL (#65)
- *(nvim)* Install with lazy.nvim in one line, with examples (#42)
- *(vars)* Rename vars by scope, env_var() support, unused vars, inline Jinja preview (#66)
- Macro calls in Jinja blocks, plan findings inline, test names checked against dbt (#67)
- *(nvim)* A yazi-style code-action menu and dialogs walked with h/j/k/l (#68)

### 🐛 Bug Fixes

- *(parser)* Lex SQL across Jinja as one document (#9)
- *(analysis)* One definition of a Jinja comment (#10)
- *(treesitter)* Handle `##}` comment closers and raw blocks (#11)

### 📚 Documentation

- *(editors)* Per-editor install guides pairing plugin and server (#7)
- Guides for Neovim, incrementality, the commands and editor parity (#44)
- *(specs)* Highlighting for macros, tests and YAML property files (#47)

### 🚜 Refactor

- *(treesitter)* Parse with tree-sitter-dbt, its own repository (#36)

### 🧪 Testing

- *(e2e)* Two timing-dependent e2e tests wait for what they mean (#69)

### ⚙️ Miscellaneous Tasks

- Proper release assets — archives, checksums, vsix (#6)
- *(release)* Compile the extension explicitly before vsce (#8)
- *(release)* V0.6.0 (#70)
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
- *(release)* V0.5.0 (#5)
