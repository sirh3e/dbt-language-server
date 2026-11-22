# Feature Requests — Missing LSP Features

Status of the Rust implementation (`crates/`) against the LSP specification.
The server currently implements exactly the original Go server's surface:

| Implemented today | Notes |
|---|---|
| `initialize` / `shutdown` / `exit` | capabilities per `specs/rust-port/01-protocol.md` §3.1 |
| `textDocument/didOpen` / `didChange` / `didSave` | incremental sync (`textDocumentSync: 2`) |
| `textDocument/hover` | refs, sources, vars, macros, dialect SQL functions |
| `textDocument/definition` | models, sources/tables, vars (by scope, to the deciding key), macros, in-file CTEs |
| `textDocument/references` | models, vars |
| `textDocument/inlayHint` | relations, var values, macro parameters |
| `textDocument/completion` | context-aware inside `ref('` / `source('` / `var('` / `{{` |
| `workspace/executeCommand` → `dbt.goToSchema` | jump to the model's properties-YAML entry |
| `textDocument/publishDiagnostics` (push) | dbt Fusion compile only (`--fusion`) |

Everything below is missing. Ordered by suggested priority; "building block"
names what already exists in the codebase.

## P1 — cheap wins, real gaps

- [x] **`textDocument/didClose`** — closed documents currently stay in the
      `Documents` store forever (leak inherited from the Go server).
      Building block: `Documents` (`crates/cogitator-analysis/src/lib.rs`) just
      needs a `close(uri)` + dispatch arm.
- [x] **Wire DuckDB parse diagnostics into the dispatcher** — the masking,
      confidence rule and `json_serialize_sql` engine exist
      (`crates/cogitator-duckdb`, feature `duckdb-parser`, spec 07 §3) but are
      not yet called on didOpen/didSave for duckdb projects.
- [x] **`textDocument/references`** — find all models referencing a model.
      Building block: the petgraph dependency graph already answers it
      (`ProjectGraph::dependents_of`, `crates/cogitator-analysis/src/graph.rs`);
      only dispatch + response wiring is missing.
- [x] **Dangling-ref diagnostics** — warn on `ref('nonexistent')` without
      Fusion. Building block: `ProjectGraph::dangling_refs` (spec 05 §5).
- [x] **`initialized` notification** — currently ignored; handle explicitly.

- [x] **Project scan cache** (05 §6.2 fast path): `FileCache` keyed by
      (mtime, size) in `cogitator-analysis/src/cache.rs`; the refresh task
      keeps one `ContextBuilder` alive, so unchanged YAML/macro/docs/model
      files are never re-read or re-parsed between refreshes.
- [x] **Column lineage**: `dbt.columnLineage` workspace command
      (`cogitator-analysis/src/lineage.rs`) — named-placeholder masking,
      sqlparser AST, CTE-transitive resolution incl. `select *` chains;
      declines on Jinja control blocks (07 §3.2 confidence rule).

- [x] **Column hover + go-to-definition**: hovering a column shows
      "Column of <relation>" with its YAML description and, when derivable,
      its lineage sources inline (dbt.columnLineage surfaced in the editor);
      definition jumps to the column's YAML entry (column positions now ride
      the saphyr side-table like model names).
- [x] **Model rename** (`textDocument/rename`): the graph knows every
      dependent, so the returned WorkspaceEdit rewrites each `ref('…')`,
      updates the properties-YAML entry and renames the model file
      (documentChanges with the file rename last). Caveat: dependents are
      read from disk; unsaved buffers of other files aren't visible here.
      Plain-scalar `ref('…')` in properties YAML (relationship tests'
      `to:`, exposures' `depends_on:`) are rewritten too, and a quoted
      YAML `name:` keeps its quotes.
- [x] **References for every dbt kind** (`features/references/`):
      sources (whole or one table, from a `source()` or the YAML entry),
      macros (call sites resolved as dbt resolves them — bare and
      `pkg.macro`, in models, tests, analyses and macro files), docs
      blocks (`doc()` uses from a use or the `{% docs %}` header) and
      documented columns (the YAML entry, the model's SQL, and direct
      dependents whose column lineage reads it). `includeDeclaration` adds
      the definition; the model `ref()` references gain the model file.
- [x] **Macro rename**: prepareRename accepts a macro call, its
      `{% macro %}` header or a `macros:` YAML entry; the edit renames the
      header, every call resolving to it (qualified too) and the YAML
      entry. Macros of installed packages are refused with the reason
      (`RequestFailed`), as are invalid and already-taken names.
- [x] **Macro call hierarchy, both ways**: prepare from a call site or a
      definition header; incomingCalls lists models, tests, analyses and
      calling macros; outgoingCalls lists the macros a macro's body (or a
      model) calls.
- [x] **Document links** for macro calls (to the defining file) and
      `doc()` (to the docs block), plus plain-scalar `ref()`/`source()` in
      properties YAML.
- [x] **CodeLens batch** on model files — "N downstream" / "N upstream"
      counts (graph; dbt terminology: `downstream_of`/`upstream_of`),
      "Go to schema" (navigates via window/showDocument), "Column lineage"
      (formatted into window/showMessage), and "dbt run / test / compile"
      executing per-model (`--select <selector>`, project-root cwd; fusion
      path when set, else `dbt`), with a successful compile opening
      target/compiled/<project>/<model>.sql in the editor. Macro files get
      per-macro "N call sites" lenses. Server→client requests use an
      outgoing-id counter; client responses are ignored by a response guard.
- [x] **Quality lenses** (`features/quality/`): a coverage lens on a
      model's SQL file and its YAML entry ("3/7 columns documented · 2/7
      tested", "· no description" for an undescribed model; the SQL side
      counts produced-but-undeclared columns from lineage), a lens on each
      YAML column lacking a description or a test, and the figures in
      dbt.modelInfo (`coverage`) and dbt.status (`project.coverage`).
      "Run this test" lenses on every generic test entry of a properties
      YAML (models, seeds, snapshots, versions, sources), on unit tests and
      on singular test files run exactly that test through dbt.runSelection
      — the selector is dbt's full generic-test name
      (`synthesize_generic_test_names`: `<test>_<target>_<args>`,
      `source_` and package prefixes, `name:` overrides), which a bare
      `--select` matches as the FQN's last element. Caveat: a test on a
      versioned model's base entry is named after the base model.
- [x] **Unit-test scaffold** code action ("Scaffold a unit test for 'm'",
      on the model's SQL and on its YAML entry): a dbt ≥ 1.8 `unit_tests:`
      entry with one `given:` input per ref()/source(), each with a row of
      the columns the model reads from it (lineage, else the input's
      documented columns), and an `expect:` row of the output columns,
      every value `null`; appended to an existing `unit_tests:` list or
      started at the end of the file.
- [x] **rootUri / workspaceFolders** honored at initialize (modern clients
      that omit the deprecated rootPath find the project again).
- [x] **Actionable error feedback**: failures report through
      window/showMessage with reason + solution — missing dbt project,
      unresolved dialect/profiles.yml, invalid fusion binary, dbt executable
      missing or failing (exit code + stderr tail + exact retry command),
      missing schema entry, lineage declines, defused upgrade subcommand.
- [x] **Completion engine** (grammar-aware): the cursor position is
      classified from the token stream — member access (`alias.` → columns
      of the resolved relation via a token-based FROM/JOIN scope index),
      relation position after FROM/JOIN (CTEs plain, models/sources as
      `{{ ref(...) }}` snippets), column positions (SELECT/WHERE/ON/…),
      keyword continuations (`group `→`by`, `left `→`join`), with tiered
      ranking (context-exact < locals < columns < relations < functions <
      keywords). Fixes the inherited quirk where any earlier `{{ }}` on the
      line triggered macro completion.
- [x] **Write-assist completion** in plain SQL: local CTEs, identifiers
      already in the buffer (table aliases, columns in use), documented
      columns of every `ref`/`source` relation in the file (from the
      properties YAML, now carried through `cogitator-schemas`), and SQL
      keywords from sqlparser's table (covers the Snowflake set — QUALIFY,
      ILIKE, …), sorted local-first.
- [x] **Snowflake coverage closed out**: profiles.yml → dialect, keyword
      suggestions, function docs/completion (743 entries), and lineage
      parsing Snowflake syntax (QUALIFY) — all tested.

- [x] **Unresolved references, project-wide** (`dbt.unresolved`): every
      unresolved `ref`, `source`, source table, `doc`, macro and `var` of the
      root project in one list (kind, name, uri, range, message,
      suggestions), from the dangling-ref diagnostics' per-document checks
      plus `{{ macro() }}` calls no package defines and dbt does not
      provide; unsaved buffers count. Neovim `:DbtUnresolved` (quickfix) and
      `:DbtFind unresolved` (picker), Emacs `cogitator-unresolved`, VS Code
      "dbt: Unresolved references".
- [x] **Hover/definition on compiled SQL**: in `target/compiled/…` or
      `target/run/…`, hover shows the Jinja source lines a compiled line came
      from (a link to them) above the SQL hover, and definition jumps there
      (a CTE of the compiled file still wins). The line map is a patience
      diff of the normalized texts: copied lines are exact anchors, the
      lines between anchors come from the source lines between them.

## P2 — quality of life

- [x] **`completionItem/resolve`** — send lean completion items, resolve
      documentation lazily (today every item carries full docs).
- [x] **`$/cancelRequest`** *(REAL since the offload batch: aborts in-flight work, answers -32800)* — ignored today; matters once any request gets
      slower than the current sub-millisecond handlers.
- [x] **`textDocument/signatureHelp`** for macros *(parameters split from the signature text)* — feasible once the
      minijinja-parsed macro parameters land (spec 06 §2.2 enrichment:
      `MacroParam { name, default }`).
- [x] **`workspace/didChangeWatchedFiles`** *(handled as refresh trigger; dynamic registration not requested yet, so some clients never send it)* — file watching as the context
      refresh trigger instead of the didOpen/didSave heuristic
      (pairs with the incremental-refresh fast path, spec 05 §6.2).
- [x] **Proper JSON-RPC error responses** *(MethodNotFound / InvalidParams on requests)* — malformed requests are silently
      dropped (faithful to Go); unknown methods should answer
      `MethodNotFound`, bad params `InvalidParams`.

## P3 — planned phases (already specced)

- [x] **Tree-sitter backend shipped** (`crates/cogitator-treesitter`, 06 §3): the
      tree-sitter-dbt grammar (sirh3e/tree-sitter-dbt, once a vendored dbt-labs
      jinja2 grammar) segments documents error-tolerantly —
      broken constructs degrade locally instead of staling the whole document
      — and is the default `DocumentParser` (fallback: `--no-tree-sitter`).
      06 §3.4 incremental `tree.edit()` reparse: DONE for documents past a
      32 KiB gate (equivalence-tested against full reparse). The grammar is
      full Jinja with dbt's blocks and lives in its own repository, pinned by
      `rev` in the workspace Cargo.toml.
- [x] **Tree-sitter batch — token-based v1 shipped** (06 §3.6, the tree can
      later upgrade fidelity: richer folding/symbols from real nodes):
      - [x] `textDocument/semanticTokens` (full and range); YAML property
            files get the dbt-aware captures of `dbt.highlight` (models,
            sources, columns, tests, macros, refs, `doc()`, config keys,
            `.unresolved`) and the Jinja/SQL inside values through the same
            stream — legend types `type`/`property`/`parameter` and
            modifiers `declaration`, `dbt`, `unresolved` (specs/highlighting
            §12), so stock clients (Zed) colour YAML too
      - [x] `textDocument/foldingRange` (multi-line paren pairs)
      - [x] `textDocument/documentSymbol` (CTEs + macros, flat)
      - [x] `textDocument/selectionRange` (token → line → document)
- [x] **dbt YAML intelligence — basic v1** (spec 08 §2.6):
      - [x] syntax + dbt-schema deserialize diagnostics for `dbt_project.yml`
            and properties files
      - [x] property-name completion in dbt `.yml` files (heuristic section
            detection; enum-value completion still open)
      - [x] schema-driven properties completion (docs/properties-yaml.md):
            keys and enum/boolean values at any path from the vendored
            schema, generic test names (built-ins, `{% test %}` blocks,
            `test_*` macros, packages') with their arguments, and the
            model's output columns under `columns:`
      - [x] `yaml-unproduced-column`: tests on a column the model does not
            produce (lineage-certain only), rename/remove quickfixes
      - [x] "Add missing columns" offered from the model's YAML entry too
- [x] **Pull diagnostics** — `textDocument/diagnostic` (full report, same
      static set as the push pipeline); `workspace/diagnostic` still open.
- [x] **Inline plan hints** (specs/ir-pipeline/06): each CTE's row count as
      an inlay hint when catalog.json statistics prove it — exact through
      projections, sorts and windows, an upper bound (`≤`) through filters,
      groups, DISTINCT, LIMIT and semi/anti joins, nothing through other
      joins or without statistics (`features/plan/row_hints.rs`,
      `row_inlay.rs`; `perf.row_hints`, off with `perf.catalog_stats`). The
      plan rules' findings already sit at their source as diagnostics
      (`features/plan/diagnostics.rs`, `perf_rules`).
- [x] **Incremental-model helper**: `dbt.incrementalDiff {uri, show?}` renders
      the model with `is_incremental()` false and true (the dbt.expandJinja
      stubs, `{{ this }}` as the model's relation) and diffs them; incremental
      models carry the "Compare incremental / full refresh" lens
      (`features/incremental_diff.rs`; :DbtIncrementalDiff,
      `cogitator-incremental-diff`, the VS Code palette).
- [ ] **AST-driven features for duckdb projects** (spec 07 §4,
      `json_serialize_sql` AST): column-name completion, AST-based graph
      edges, better CTE information.

## P4 — not planned / needs a Jinja renderer or major new scope

Implemented from this bucket (they turned out cheap on the existing data):
- [x] `textDocument/documentHighlight` (same literal + kind occurrences)
- [x] `workspace/symbol` (models, sources, macros)

Still open, with reasons:

- `textDocument/formatting` / `rangeFormatting` / `onTypeFormatting`
  (would mean embedding a SQL formatter)
- `textDocument/codeAction`, `prepareRename`,
  `inlineValue`,
  `documentColor`
- `textDocument/typeDefinition` / `implementation` / `declaration`
  (little meaning for dbt sources)
- Call/type hierarchy (could model macro call chains someday)
- workspace folders / multi-root (only the deprecated
  `rootPath` is read), `workspace/didChangeConfiguration`,
  `workspace/applyEdit`, file-operation notifications, `$/progress`

- [ ] **Quickfix code actions from did-you-mean suggestions** — purely
      additive on top of the Guidance diagnostics: needs Diagnostic.data +
      relatedInformation wire fields and a diagnostics-context codeAction
      path; the suggestions already ride every dangling-ref/-source/
      unknown-var message.
