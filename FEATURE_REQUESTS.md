# Feature Requests — Missing LSP Features

Status of the Rust implementation (`crates/`) against the LSP specification.
The server currently implements exactly the original Go server's surface:

| Implemented today | Notes |
|---|---|
| `initialize` / `shutdown` / `exit` | capabilities per `specs/rust-port/01-protocol.md` §3.1 |
| `textDocument/didOpen` / `didChange` / `didSave` | incremental sync (`textDocumentSync: 2`) |
| `textDocument/hover` | refs, sources, vars, env vars, macros, dialect SQL functions, what a `{{ … }}` renders to |
| `textDocument/definition` | models, sources/tables, vars (by scope, to the deciding key), env vars (`.env` line, else profiles.yml), macros, in-file CTEs |
| `textDocument/references` | models, vars, sources, macros, docs, columns, file-local names (Jinja locals, macro parameters, CTEs, aliases) |
| `textDocument/inlayHint` | relations, var values, env var values (secrets masked), macro parameters, opt-in Jinja renderings |
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
      columns (the YAML entry and every consumer at any depth that keeps
      the name — the column rename's walk). `includeDeclaration` adds
      the definition; the model `ref()` references gain the model file.
- [x] **Macro rename**: prepareRename accepts a macro call, its
      `{% macro %}` header or a `macros:` YAML entry; the edit renames the
      header, every call resolving to it (qualified too) and the YAML
      entry. Macros of installed packages are refused with the reason
      (`RequestFailed`), as are invalid and already-taken names.
- [x] **Workspace file operations** (`features/file_ops/`,
      `workspace.fileOperations`): `willRenameFiles` of a model (or seed)
      file answers the model rename's text edits — every `ref('old')` in
      SQL and YAML and the properties-YAML `name:` — without the file
      rename, which the client does; a move that keeps the name, a macro,
      YAML or snapshot file and a folder answer no edits.
      `willDeleteFiles` answers an empty edit and one warning per request
      naming the dependents a deleted model (or folder of models) leaves
      dangling. `didCreateFiles`/`didRenameFiles`/`didDeleteFiles`
      refresh the context, so a new model gets its "Extract schema YAML"
      lens and a renamed one its new name without a watcher registration.
- [x] **`textDocument/declaration`** (`features/declaration.rs`): a
      ref's or model file's declaration is its properties-YAML entry
      (definition stays the SQL), a documented column's its YAML column;
      definition on a YAML column entry now goes to the select item of
      the model's SQL that produces it (the last `as col` / `[q.]col,`
      item, else its first mention). Sources, macros, vars and docs:
      declaration = definition.
- [x] **Column rename** (`textDocument/rename` + `prepareRename`) on a
      model's output column (the SQL alias or bare column, or its YAML
      entry), transitive: the model's item, its YAML entry, constraints,
      `primary_key`/`combination_of_columns`, unit-test row keys, and every
      consumer while the name is kept — downstream models (renamed in turn
      when they output it under the same name), snapshots, analyses,
      singular tests, `relationships` fields and semantic-model `expr`s; a
      re-alias stops the walk. Refused as a whole (`RequestFailed`, file and
      reason) when any affected file's lineage is unclear or the new name
      collides (`features/rename_column/`, docs/column-rename.md).
- [x] **Macro call hierarchy, both ways**: prepare from a call site or a
      definition header; incomingCalls lists models, tests, analyses and
      calling macros; outgoingCalls lists the macros a macro's body (or a
      model) calls.
- [x] **Document links** for macro calls (to the defining file) and
      `doc()` (to the docs block), plus plain-scalar `ref()`/`source()` in
      properties YAML.
- [x] **Var rename** (`textDocument/rename` + `prepareRename`) on a
      `var('x')` call or a `vars:` key (root, scoped, a package's, or a
      cogitator.yml override): every call that resolves to the same
      definition and the keys that decide it, scope-aware
      (docs/variables.md, "Renaming a var").
- [x] **File-local rename** (`textDocument/rename` + `prepareRename`,
      references and document highlights; `features/rename_local/`):
      Jinja `set` / block `set` / loop variables and `call(…)` arguments,
      scoped as Jinja scopes them over minijinja's AST; macro parameters,
      with the keyword arguments of the macro's calls project-wide; CTE
      names and table aliases over the sql-front binder's HIR, refused when
      a written occurrence is one the binder could not follow
      (docs/local-names.md). Checked before the var rename, which still
      wins on a `var('x')` call.
- [x] **env_var() support**: the value inline (`inlay.env_vars`), hover
      with its source, definition into `.env` (else the profiles.yml
      use), an `env-var-unset` warning for a defaultless env_var() the
      server's environment lacks; secret-looking names are masked.
- [x] **Unused vars**: `unused-var` hints (Unnecessary) on root
      dbt_project.yml `vars:` keys no scope reads, with a removal quickfix.
- [x] **Inline Jinja preview**: hover inside `{{ … }}` shows what it
      renders to (dbt.expandJinja's stubs, fuel- and size-capped); opt-in
      inlay hints for short single-line renderings (`inlay.jinja`).
- [x] **Macro calls inside Jinja blocks**: calls in `{% set %}`, `{% if
      %}`, `{% for %}`, `{% do %}` and `{% call %}`, nested and filter
      arguments are call tokens like `{{ m() }}` (parser,
      `cogitator-parser/src/calls.rs`), so references, rename, the call
      hierarchy, links, hover/definition, the graph's model→macro edges and
      `dbt.unresolved` see them; context functions, methods, filters,
      tests and template-bound names are not calls.
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
      `--select` matches as the FQN's last element, also past dbt's
      64-character md5 alias. A test on a versioned model's base entry is
      one dbt test per version it applies to (versions without their own
      `data_tests:`, columns their `include`/`exclude` keeps), and its lens
      selects them all (`not_null_m_v1_id not_null_m_v2_id`). Arguments
      are read as PyYAML (YAML 1.1: `yes`, `010`, `1e3`) and written as
      Python's `str()`. Checked against dbt-core 1.11.17 on
      testdata/test_names (golden.json, regenerated by golden.sh).
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
- [x] **Breaking-change preview** (`dbt.breakingChanges`,
      `features/breaking/`): the columns a model's SQL produces now (typed
      lineage) against its last built schema — `target/catalog.json`, else
      the documented columns of `target/manifest.json` — as removed, added,
      "possibly renamed" (a removed and an added column with similar names,
      never asserted) and retyped columns (only when both types are known;
      the current side's are the proven inferred output-column types of
      `features::column_types`, of the saved files); for each removed or retyped column its own
      tests and the downstream columns whose lineage reads it, with their
      tests, contracts and `access: public`, plus the model's exposures.
      Computed on open and save, off the request path: `breaking-change`
      diagnostics (error only for a removed column a test or a contracted
      downstream relies on) and an "N breaking changes" lens; silent without
      target/ artifacts (`dbt.status` says so under `breaking_changes`).
      Neovim `:DbtBreakingChanges`, Emacs `cogitator-breaking-changes`,
      VS Code "dbt: Breaking changes against the last build". No git
      baseline (the model at `origin/main`) yet.
- [x] **dbt node selection** (`selectors/`, docs/selectors.md): one pure
      engine for dbt's selector language — methods (`fqn`, `tag`, `path`,
      `file`, `package`, `config.<key>`, `resource_type`, `source`,
      `exposure`, `metric`, `semantic_model`, `unit_test`, `test_type`,
      `test_name`, `group`, `access`, `version`, `wildcard`; `state:`,
      `result:` and `source_status:` reported as needing state), the graph
      operators (`+x`, `x+`, `N+x`, `x+N`, `@x`), unions and
      intersections, selectors.yml definitions (`union`, `intersection`,
      `exclude`, `method`/`value` with `parents`, `children`, depths,
      `childrens_parents`, `indirect_selection`, `selector:` references)
      and dbt's eager/cautious/buildable/empty indirect selection of tests.
      `workspace/symbol`'s `tag:`, the run lenses' selectors and
      `dbt.runSelection`'s validation share it. In selectors.yml:
      completion of methods and values (tags, paths, packages, node names,
      config keys and values, selectors), diagnostics (unknown method or
      key, unknown selector, cycles, a selector that matches nothing, state
      methods), "N nodes match" lenses and a hover with the first names;
      tag names complete in `tags:`/`+tags:` of dbt_project.yml and
      properties files. `dbt.select {selector}` lists the nodes: Neovim
      `:DbtSelect` (quickfix) and `:DbtFind select` (picker), Emacs
      `cogitator-select`, VS Code "dbt: Select nodes".
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
      - [x] output-column types (`features/column_types/`): each output
            column's type off the schema job's cached MIR, with provenance
            (cast, literal, function, aggregate, upstream column, union) and
            a basis (proven only over inferred, catalog or contracted
            upstream types); `column-type-mismatch` on a `data_type:` of
            another type family (Error under an enforced contract) with
            "Set data_type to …", "Fill data_type from the inferred types",
            `unit-test-value-type` hints on impossible unit-test row values,
            the type on hover (YAML column, SQL output column), and
            `columnTypes` in dbt.modelInfo
- [x] **Pull diagnostics** — `textDocument/diagnostic` (full report, same
      static set as the push pipeline); `workspace/diagnostic` still open.
- [x] **Inline plan hints** (specs/ir-pipeline/06): each CTE's row count as
      an inlay hint when catalog.json statistics prove it — exact through
      projections, sorts and windows, an upper bound (`≤`) through filters,
      groups, DISTINCT, LIMIT and semi/anti joins, nothing through other
      joins or without statistics (`features/plan/row_hints.rs`,
      `row_inlay.rs`; `perf.row_hints`, off with `perf.catalog_stats`). The
      plan rules' findings sit at their source as diagnostics
      (`features/plan/diagnostics.rs`, `perf_rules`) and, with
      `perf.inline_findings` (default false), as inlay hints too —
      `⚠ join.cartesian`, the whole finding as the tooltip, a label part
      running `dbt.queryPlan`, merged with the CTE's row count into one hint
      (`1,200 rows · ⚠ model.unused_cte`; `features/plan/finding_inlay.rs`,
      `perf.inlineFindings` in dbt.status/dbt.doctor).
- [x] **Incremental-model helper**: `dbt.incrementalDiff {uri, show?}` renders
      the model with `is_incremental()` false and true (the dbt.expandJinja
      stubs, `{{ this }}` as the model's relation) and diffs them; incremental
      models carry the "Compare incremental / full refresh" lens
      (`features/incremental_diff.rs`; :DbtIncrementalDiff,
      `cogitator-incremental-diff`, the VS Code palette).
- [x] **Preview the rows of a CTE**: a "Preview rows" lens on every CTE of a
      model and on its final select runs `dbt.previewCte {uri, cte?,
      position?, limit?}` — the compiled SQL (or the dbt.expandJinja
      rendering while no current compiled file exists) cut at the CTE with
      sqlparser, refused unless it is one SELECT, run as `dbt show --inline
      … --output json` through the job registry (cancellable, `$/progress`,
      the active target); the answer carries columns, rows, `truncated` and
      the SQL, and the markdown table opens from
      `target/.cogitator/preview/` (`features/cte_preview/`,
      `server/preview_cte.rs`; cogitator.yml `preview.rows`/`max_rows`;
      :DbtPreviewCte, `cogitator-preview-cte`, the VS Code palette).
- [ ] **AST-driven features for duckdb projects** (spec 07 §4,
      `json_serialize_sql` AST): column-name completion, AST-based graph
      edges, better CTE information.

## P4 — not planned / needs a Jinja renderer or major new scope

Implemented from this bucket (they turned out cheap on the existing data):
- [x] `textDocument/implementation` for dispatched macros: every
      `<adapter>__x` / `default__x` in dbt's dispatch search order (the
      `dispatch:` config, the root project, the namespace), asked from an
      `adapter.dispatch` call, a call of the dispatching macro or its
      header; hover names the implementation the active adapter picks
      (`features/dispatch/`, docs/macros.md)
- [x] Macro call checks: `macro-arg-count`, `macro-unknown-kwarg`
      (did-you-mean quickfix), `macro-duplicate-kwarg` and
      `dispatch-no-default`, arguments read by minijinja's parser, lenient
      for bodies reading `varargs`/`kwargs`; on by default, `lint:
      {macro_calls, dispatch}` turns them off (`features/macro_calls/`)
- [x] Syntax diagnostics without `dbt compile`: `jinja-syntax` (minijinja's
      parser, dbt's block tags understood) and `sql-syntax` (sqlparser in the
      project's dialect over the masked SQL, under the 07 §3.2 confidence
      rule), pushed on didChange too; on by default, `lint: { syntax }` turns
      them off (`features/syntax/`, docs/lint-and-format.md)
- [x] `cogitator lint` / `cogitator format` subcommands: the editor-free
      diagnostics over the whole project for CI (text/JSON/GitHub output,
      exit codes 0/1/2), and the formatter in place or `--check`/`--diff`
- [x] `textDocument/documentHighlight` (same literal + kind occurrences)
- [x] `workspace/symbol` (models, sources, macros)

Still open, with reasons:

- `rangeFormatting` / `onTypeFormatting` (`textDocument/formatting` is
  implemented, and `cogitator format` runs it from the command line)
- `textDocument/codeAction`, `prepareRename`,
  `inlineValue`,
  `documentColor`
- `textDocument/typeDefinition`
  (little meaning for dbt sources; `declaration` and `implementation`
  are implemented above)
- Call/type hierarchy (could model macro call chains someday)
- workspace folders / multi-root (only the deprecated
  `rootPath` is read), `workspace/didChangeConfiguration`,
  `workspace/applyEdit`, `$/progress` (file operations: implemented
  above)

- [ ] **Quickfix code actions from did-you-mean suggestions** — purely
      additive on top of the Guidance diagnostics: needs Diagnostic.data +
      relatedInformation wire fields and a diagnostics-context codeAction
      path; the suggestions already ride every dangling-ref/-source/
      unknown-var message.
