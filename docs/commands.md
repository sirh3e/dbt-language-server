# Command reference

Every `dbt.*` command the cogitator server answers through
`workspace/executeCommand`, with its argument, its result, what it does
besides answering, and which editors expose it. The server advertises the
same list in `initialize` (`executeCommandProvider.commands`, append-only:
`dbt.goToSchema` stays first).

Every command takes **one argument object**:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "workspace/executeCommand",
  "params": {
    "command": "dbt.modelInfo",
    "arguments": [{ "uri": "file:///path/to/models/customers.sql" }]
  }
}
```

`uri` is the document the command is about; a few commands read more fields
(listed below). Commands that run dbt answer `{token}` at once and report
through `window/showMessage` and `$/progress`; `dbt.cancel` takes the token.

**Editors.** ✓ means the editor wraps the command (a Neovim `:Dbt…` command,
an Emacs `cogitator-…` command, a VS Code palette entry); — means it does
not yet. Commands the server sends inside a code lens or code action run in
every LSP client, Zed included. The coverage comes from
[`editors/parity.toml`](../editors/parity.toml), which a test keeps true
([editor-parity.md](editor-parity.md)); a test also fails while a command
of the server has no section here.

The binding contract is `specs/neovim-plugin/00-plugin.md` §1.6; the
incrementality commands are explained in [incrementality.md](incrementality.md).

## Navigation

### `dbt.goToSchema`

- **Argument**: `{uri, position}` — the model's file; `position` on a `ref()` targets that model
- **Result**: `Location` or `null`
- **Side effects**: none (client jumps)
- **Editors**: Neovim ✓ · Emacs ✓ · VS Code —

### `dbt.openSchema`

- **Argument**: `{uri}` — the document the command is about
- **Result**: `null`
- **Side effects**: `window/showDocument` or warning
- **Editors**: every LSP client — the server sends it in a code lens and code action 'Go to schema'

### `dbt.goToTests`

- **Argument**: `{uri}` — the document the command is about
- **Result**: `null`
- **Side effects**: showDocument to the first test + INFO list
- **Editors**: every LSP client — the server sends it in a code lens 'N tests' and its code action

### `dbt.openModel`

- **Argument**: `{uri}` or `{model}`
- **Result**: `null`
- **Side effects**: showDocument to the model's file (`{uri}` or `{model}`), or a warning
- **Editors**: every LSP client — the server sends it in a code action 'Open model'

### `dbt.modelTests`

- **Argument**: `{uri}` — the document the command is about
- **Result**: `{model, tests: [{name, column?, version?, uri, range}]}` or `null`
- **Side effects**: none
- **Editors**: Neovim ✓ · Emacs ✓ · VS Code —

### `dbt.relation`

- **Argument**: `{uri, position}` — `position` on a `ref()`/`source()` resolves that relation
- **Result**: `{kind, name, source, database, schema, identifier, relation}` or `null`
- **Side effects**: none
- **Editors**: Neovim ✓ · Emacs ✓ · VS Code —

## Running dbt

### `dbt.run`

- **Argument**: `{uri}` — the document the command is about
- **Result**: `{token}` (immediately)
- **Side effects**: async showMessage; compile success opens the compiled SQL via showDocument; the token feeds `dbt.cancel`
- **Editors**: Neovim ✓ · Emacs ✓ · VS Code ✓

### `dbt.test`

- **Argument**: `{uri}` — the document the command is about
- **Result**: `{token}` (immediately)
- **Side effects**: async showMessage; compile success opens the compiled SQL via showDocument; the token feeds `dbt.cancel`
- **Editors**: Neovim ✓ · Emacs ✓ · VS Code ✓

### `dbt.compile`

- **Argument**: `{uri}` — the document the command is about
- **Result**: `{token}` (immediately)
- **Side effects**: async showMessage; compile success opens the compiled SQL via showDocument; the token feeds `dbt.cancel`
- **Editors**: Neovim ✓ · Emacs ✓ · VS Code ✓

### `dbt.preview`

- **Argument**: `{uri, limit?}` — rows to show (default 50)
- **Result**: `{token}` (job started)
- **Side effects**: `dbt show --select <model> --limit N` (N from `limit`, default 50) through the job registry; on success the result table lands in `target/.cogitator/preview/<model>.txt` and opens via showDocument — :DbtPreview [limit]
- **Editors**: Neovim ✓ · Emacs — · VS Code ✓

### `dbt.previewCte`

- **Argument**: `{uri, cte?, position?, limit?}` — the model; the CTE to preview, else the CTE under `position`, else the final select; rows to show (default cogitator.yml `preview.rows`, 20, capped at `preview.max_rows`, 1000)
- **Result**: `{model, cte, columns, rows, truncated, limit, sql, source, uri, table}` once dbt has answered (`source` is `compiled` when `target/compiled` is current, else `rendered` by the dbt.expandJinja renderer; `table` is the markdown); RequestFailed with a Guidance when the SQL is not one SELECT, has no such CTE, or dbt fails; RequestCancelled after `dbt.cancel`
- **Side effects**: `dbt show --inline "<with … up to the CTE> select * from <cte> limit N+1" --limit N+1 --output json` as a job of the registry (active target and profiles, token in the `$/progress` title); the table lands in `target/.cogitator/preview/<model>.<cte|final>.md` and opens via showDocument. The "Preview rows" code lens on every CTE and on the final select runs it
- **Editors**: Neovim ✓ (:DbtPreviewCte [limit], a split) · Emacs ✓ (`cogitator-preview-cte`, `*cogitator-preview*`) · VS Code ✓ (`dbt: Preview rows of the CTE under the cursor`)

### `dbt.parse`

- **Argument**: `{uri}` — the document the command is about
- **Result**: `{token}` (immediately)
- **Side effects**: async showMessage/$\/progress; project-wide (no `--select`), success refreshes the server context
- **Editors**: Neovim ✓ · Emacs ✓ · VS Code ✓

### `dbt.deps`

- **Argument**: `{uri}` — the document the command is about
- **Result**: `{token}` (immediately)
- **Side effects**: async showMessage/$\/progress; project-wide `dbt deps`, success refreshes the server context (the packages.yml quickfix runs it)
- **Editors**: every LSP client — the server sends it in a code action 'Run dbt deps' for a missing package

### `dbt.sourceFreshness`

- **Argument**: `{uri}` — the document the command is about
- **Result**: `{token}` (immediately)
- **Side effects**: async `dbt source freshness` (project-wide); the refreshed sources.json feeds the freshness lenses on source YAML entries
- **Editors**: Neovim ✓ · Emacs — · VS Code —

### `dbt.runSelection`

- **Argument**: `{uri, selector, exclude?, kind?}` — any dbt selector, and what to leave out of it (dbt's `--exclude`, checked the same way); `kind` is `run` (default), `test`, `build` or `compile`
- **Result**: `{token}` or InvalidParams
- **Side effects**: runs dbt with an arbitrary validated selector (`{selector, kind?: run\|test\|build\|compile}`); the "Run this test" / "Run this unit test" lenses on a properties YAML's tests and on singular test files send it with `kind: test` and the test's dbt node name (a test on a versioned model's base entry is one test per version, so its lens sends their names as a space-separated union)
- **Editors**: Neovim ✓ · Emacs — · VS Code —

### `dbt.cancel`

- **Argument**: `{uri, token}` — a token a job command answered
- **Result**: `{token, label}` or InvalidParams
- **Side effects**: aborts the running invocation (the child process is killed) + confirmation message
- **Editors**: Neovim ✓ · Emacs — · VS Code —

## The project and its graph

### `dbt.projectGraph`

- **Argument**: `{uri}` — the document the command is about
- **Result**: `{nodes: [{id, name, kind, layer, uri, tags, columns}], edges: [{from, to}]}`
- **Side effects**: none
- **Editors**: Neovim ✓ · Emacs ✓ · VS Code —

### `dbt.modelGraph`

- **Argument**: `{uri, model?}` — the buffer's model, or `model`
- **Result**: graph payload or `null`
- **Side effects**: none
- **Editors**: Neovim ✓ · Emacs ✓ · VS Code —

### `dbt.modelInfo`

- **Argument**: `{uri?, model?}` — `model` wins
- **Result**: `{model, uri, description, tags, upstream, downstream, versions, tests, compiledUri, coverage, columnTypes?}` or `null` — `coverage` is `{columns, documented, tested, described}` from the YAML entry; `columnTypes` (once the project schema job has typed the saved model) is `{model, revision, columns: [{name, dataType, description, nullable, basis, provenance, feeds, why}]}`: `dataType` is the dialect's spelling and only set when the type is pinned exactly, `basis` is `proven`, `declared` (rests on an unenforced YAML `data_type`) or `unknown` (see [properties-yaml.md](properties-yaml.md#column-types))
- **Side effects**: none
- **Editors**: Neovim ✓ · Emacs — · VS Code —

### `dbt.compiledSql`

- **Argument**: `{uri?, model?}` — `model` wins
- **Result**: `{model, uri, content}` or `null`
- **Side effects**: WARNING when the file is over the wire cap (2 MiB default; `limits.compiled_sql_mib`)
- **Editors**: Neovim ✓ · Emacs — · VS Code —

### `dbt.columnLineage`

- **Argument**: `{uri}` — the document the command is about
- **Result**: lineage JSON or `null`
- **Side effects**: none
- **Editors**: Neovim ✓ · Emacs — · VS Code —

### `dbt.showLineage`

- **Argument**: `{uri}` — the document the command is about
- **Result**: `null`
- **Side effects**: formatted `window/showMessage`
- **Editors**: Neovim ✓ · Emacs ✓ · VS Code ✓

### `dbt.impact`

- **Argument**: `{uri, model?, column?}` — the buffer's model, or `model`; `column` narrows to one column
- **Result**: `{model, column, summary, models: [{name, uri, depth}], exposures, columnHits, truncated, reach?}`. With a `column`, `reach` says who reads it past the models (docs/column-reach.md): `{summary, models, tests: [{model, column, test}], semanticItems: [{semanticModel, model, column, list?, name?, uri, range}], semanticModels, metrics: [{name, uri, range, via}], exposures: [{name, level: "node"}], dashboards: [{name, url?, level: "column"|"node", model, column?}], notes?}`, and `summary` ends in "read by N models, M tests, K semantic models, J dashboards"
- **Side effects**: none (column hits are an under-approximation: `*` and unresolved lineage stay out; lineage fan-out capped at 200 models; exposures are node level only; metric filters are not followed). On open/save the server caches every output column's reach of the model for the column hover
- **Editors**: Neovim ✓ (`:DbtImpact [column]` quickfix) · Emacs ✓ (`M-x cogitator-impact`) · VS Code ✓ (`dbt: Who reads this column?`)

### `dbt.breakingChanges`

- **Argument**: `{uri}` — a model's `.sql` buffer
- **Result**: `{model, status, baseline?: {source, generatedAt?}, summary, diff: {removed: [{name, dataType?, possiblyRenamedTo?}], added: [{name, dataType?}], retyped: [{name, from, to}]}, impacts: [{column, change, severity, tests, downstream: [{model, column, tests, contracted, public}], semantic?, metrics?, range?, keepAlias?}], exposures, public, truncated, sinceSave?}`. `sinceSave` is the same report of the open buffer against the file as last saved (baseline source `saved`), present while the edit breaks something; `range` is where the column's diagnostic sits in the SQL; `keepAlias` the TextEdit of the "Keep '<old>' as an alias" quick fix; `semantic`/`metrics` the column-level semantic-model items and metrics (docs/column-reach.md). `status` is `compared`, `noBaseline` (no `target/catalog.json` or `target/manifest.json` entry with columns: nothing is reported) or `unknownColumns` (an unexpanded `*`, unparseable SQL). The current columns come from the open buffer's typed lineage; the baseline is `target/catalog.json` (warehouse columns and types), else the manifest's documented columns. `possiblyRenamedTo` pairs a removed column with an added one of a similar name — a hint, never asserted. Type changes are reported only when both types are known (the current side's types are the inferred output-column types of the saved files, and only a proven type with an exact spelling counts; any other column is compared by name). `severity`: `error` for a removed column a test or a contracted downstream model relies on, `warning` when anything reads it (downstream columns, a public model, exposures), `information` otherwise
- **Side effects**: none. On open and save, and after the refresh debounce on change, the server also computes the report off the request path; its findings are `breaking-change` diagnostics (push and pull) and a lens `N breaking changes` that runs this command
- **Editors**: Neovim ✓ (`:DbtBreakingChanges` quickfix, `:Dbt breaking`) · Emacs ✓ (`M-x cogitator-breaking-changes`) · VS Code ✓ (`dbt: Breaking changes against the last build`)

### `dbt.deadModels`

- **Argument**: `{uri}` — the document the command is about
- **Result**: `{models: [{name, uri, reasons}]}`
- **Side effects**: none (conservative: seeds/package models never flagged; selector mentions keep a model alive)
- **Editors**: Neovim ✓ · Emacs — · VS Code —

### `dbt.extractModelYaml`

- **Argument**: `{uri}` — the document the command is about
- **Result**: `null`
- **Side effects**: workspace/applyEdit (CreateFile scaffold, or version TextEdits) + showDocument
- **Editors**: Neovim ✓ · Emacs ✓ · VS Code —

### `dbt.syncYaml`

- **Argument**: `{uri?, models?, prune?, inheritTypes?, reorder?, newEntries?, apply?}` — `models` names the models to sync; without it a `.sql` `uri` narrows to its model and a properties-file `uri` to the models of its entries, and no `uri` is every SQL model of the root project. `prune` removes documented columns no longer produced, `inheritTypes` fills `data_type` from proven inferred types, `reorder` orders column items as the SQL outputs them, `newEntries: "folder"` puts new entries in `_<folder>__models.yml` (default `<model>.yml`, as `dbt.extractModelYaml`), `apply: true` applies the plan
- **Result**: `{changes, applied, files: [{uri, created, edits, diff}], models: [{model, uri?, createdEntry, added, inherited: [{column, from: SourceRef, via?}], typed: [{column, dataType}], stale, pruned, reordered, undocumented, skipped: [{column?, reason}], changes}]}` — `from` is the documented upstream column (`{kind: "model", name, column}` or `{kind: "source", source, table, column}`), `via` the undocumented models walked through; `edits` are line hunks against the file as read (the whole content for a created file), `diff` a unified diff for previews. `changes: 0` means the YAML is in sync. See [yaml-sync.md](yaml-sync.md)
- **Side effects**: with `apply: true` and something to change, workspace/applyEdit (CreateFile + TextEdits, label "Sync schema YAML"); a summary message; a warning while the project's schemas are not computed yet. The code action "Sync schema YAML of 'm' (N changes)" on a model's SQL and its YAML entry carries the same edit for one model
- **Editors**: Neovim ✓ (`:DbtSyncYaml`, preview split then confirm) · Emacs ✓ (`M-x cogitator-sync-yaml`) · VS Code ✓ (`dbt: Sync schema YAML`); the CLI `cogitator yaml-sync` plans the same

## Analysis

### `dbt.queryPlan`

- **Argument**: `{uri}` — the document the command is about
- **Result**: `{model, dialect, backend, schemaRevision, status: complete\|partial\|declined, guidance, origin, levels: {hir, mir, lir: {available, fidelity, text, unavailable, degradations, violations}}, findings, trace: {entries, incomplete}, needs: {available, missing}, rowHints?: [{cte, span, rows: {exact\|atMost}}]}`
- **Side effects**: none (read-only): runs the IR pipeline on the open buffer or file; a level dropped by a verifier violation is shown as unavailable; models with control flow other than a lone `is_incremental()` come back `declined` with a Guidance pointing at `dbt compile` until compiled fallback lands — :DbtPlan
- **Inlay hints**: the findings' label parts (`perf.inline_findings: true`, specs/ir-pipeline/08) carry `dbt.queryPlan {uri}` as their command, so a click on `⚠ join.cartesian` asks for this plan
- **Editors**: Neovim ✓ · Emacs — · VS Code —

### `dbt.incrementality`

- **Argument**: `{uri}` — the document the command is about
- **Result**: `{models, incremental}`
- **Side effects**: none; every table model's incrementality verdict, slowest last run first (specs/ir-pipeline/10)
- **Editors**: Neovim ✓ · Emacs ✓ · VS Code ✓

### `dbt.splitPlan`

- **Argument**: `{uri}` — the document the command is about
- **Result**: `{model, sexp, part: {name, cte, text}}` or `null`
- **Side effects**: none; the split at the incrementality frontier as an S-expression (specs/ir-pipeline/10 §6)
- **Editors**: Neovim ✓ · Emacs ✓ · VS Code ✓

### `dbt.incrementalDiff`

- **Argument**: `{uri, show?}` — the model; `show: true` also opens the diff as a document (what the code lens sends)
- **Result**: `{model, fullRefresh, incremental, diff, identical}` — the model rendered with the dbt.expandJinja stubs and `is_incremental()` false (a full refresh) and true, `{{ this }}` as the model's relation, and the unified diff full refresh → incremental (empty when identical); a render error is InvalidParams Guidance
- **Side effects**: none; with `show: true` writes `target/.cogitator/incremental/<model>.diff` and opens it via showDocument. Incremental models (the file's `config(materialized=…)`, else the manifest's) carry the code lens "Compare incremental / full refresh" — :DbtIncrementalDiff
- **Editors**: Neovim ✓ · Emacs ✓ · VS Code ✓

### `dbt.expandJinja`

- **Argument**: `{uri}` — the document the command is about
- **Result**: `{sql}` or InvalidParams Guidance
- **Side effects**: renders the open buffer with minijinja — project macros, ref/source → relations, var(), config()/is_incremental()/target stubs; adapter calls need a real compile
- **Editors**: Neovim ✓ · Emacs — · VS Code —

### `dbt.syntaxTree`

- **Argument**: `{uri}` — the document the command is about
- **Result**: `{nodes: [{depth, kind, field, error, missing, range}], truncated}` or InvalidParams
- **Side effects**: none (read-only): the open document's tree-sitter-dbt tree, named + ERROR + MISSING nodes in pre-order, ranges in UTF-16, at most 20 000 nodes — :DbtSyntaxTree
- **Editors**: Neovim ✓ · Emacs ✓ · VS Code ✓

### `dbt.highlight`

- **Argument**: one of
  - `{uri, language?}` — the open document or the file, its language from the path (`.yml` → yaml, `target/compiled|run` → sql, otherwise dbt);
  - `{text, language?}` — any text, `dbt` (default), `sql` or `yaml`;
  - `{model?, compiled: true}` — the SQL dbt compiled the model (or the file) to;
  - `{node: {kind, name}, compiled?}` — a macro's, test's, source's… file, or a test's compiled SQL.

  Any of them may add `range: {start: {line}, end: {line}}` (only those lines' spans) and `version` (echoed).
- **Result**: `{language, version?, spans: [{range, capture, layer}], truncated, layers}`; `null` for compiled SQL that does not exist (an uncompiled model or test, a macro)
- **Side effects**: none. The grammars' own highlights: tree-sitter-dbt for the Jinja, tree-sitter-sequel for the SQL between the tags, tree-sitter-yaml for YAML, plus — in YAML — the Jinja and SQL inside values and the dbt-aware captures (`type.dbt.model`, `property.dbt.column`, `function.dbt.test`, …; `.unresolved` for a reference the project lacks). `layer` is `yaml`, `sql`, `jinja` or `dbt`. Whole answers are cached (`dbt.cache` reports them as `highlights`). See [highlighting.md](highlighting.md)
- **Editors**: Neovim ✓ · Emacs ✓ · VS Code ✓

### `dbt.astGrep`

- **Argument**: `{pattern, language?, rewrite?, uri?, apply?}` — an ast-grep pattern (`{{ ref($M) }}`); `language` `dbt` (default) or `sql`; `uri` searches that document, else the project's dbt files (`dbt`) or `target/compiled` (`sql`)
- **Result**: `{language, files, matches: [{uri, range, text, env, replacement}], truncated}` (at most 500 matches)
- **Side effects**: with `apply: true` and a `rewrite`, every match is replaced through `workspace/applyEdit`. The project's `sgconfig.yml` rules also show as diagnostics (`source: ast-grep`) with their fixes as quickfixes — see [ast-grep.md](ast-grep.md)
- **Editors**: Neovim ✓ · Emacs ✓ · VS Code ✓

### `dbt.nodeInfo`

- **Argument**: `{node: {kind, name}, quiet?}` — `quiet: true` answers an unknown node with `null` alone (a client trying kinds in turn); `kind` one of `model`, `macro`, `test`, `generic_test`, `source`, `exposure`, `metric`, `semantic_model`, `analysis`
- **Result**: the card data of the node, by kind:
  - `model`: `dbt.modelInfo`'s answer with `kind`;
  - `macro`: `{package, description, params, uri, range, callers, calls, usedByTests}`;
  - `generic_test` (a `{% test %}` definition): `{params, uri, range, appliedTo: [{model, column, uri, range}]}`;
  - any other kind: `{uri, refs, calls, upstream, downstream, compiledUri, description}`.

  Unknown nodes answer `null`.
- **Side effects**: a WARNING with the closest names for an unknown node
- **Editors**: Neovim ✓ · Emacs ✓ · VS Code ✓

### `dbt.unresolved`

- **Argument**: `{kinds?}` — a list of `ref`, `source`, `source_table`, `doc`, `macro`, `var` to keep (all when absent)
- **Result**: `{items: [{kind, name, uri, range, message, suggestions?}], count, byKind: {kind: n}, truncated}` — every unresolved name of the root project's models, snapshots, macros, tests, analyses and property YAML, by file then position (at most 5 000 items; `count` is the total). The same checks as the dangling-ref diagnostics (`dangling-ref`, `dangling-source`, `dangling-source-table`, `unknown-var`, `dangling-doc`, `dangling-exposure-ref`), plus macro calls (in `{{ }}` or a `{% %}` block) no package defines and dbt does not provide; open documents count with their unsaved text
- **Side effects**: none
- **Editors**: Neovim ✓ (`:DbtUnresolved` quickfix, `:DbtFind unresolved` picker) · Emacs ✓ (`M-x cogitator-unresolved`) · VS Code ✓ (`dbt: Unresolved references`)

### `dbt.select`

- **Argument**: `{select, exclude?, vocabulary?, uri?}` — dbt's node-selection syntax as `--select` takes it (space a union, `,` an intersection, `[@][n+]method:value[+n]`); `select` may carry an inline `--exclude …`, and `exclude` is one more; `selector` is accepted for `select` (the selectors.yml lenses send it); `selector:<name>` evaluates a selectors.yml definition (the open buffer's text when selectors.yml is open); `vocabulary: true` adds what completion offers
- **Result**: `{selector, count, counts, nodes: [{uri, name, label, kind, resourceType, uniqueId, version, package, path, tags, range, graphId}], issues, error, vocabulary?}` — what `dbt ls --select … --exclude …` would list (eager indirect selection brings tests along), by resource type then name; `name` is dbt's name (a source's table, a model without its version), `label` how `dbt ls --output name` prints it (`landing.items`, `orders.v1`), `graphId` the node's id in `dbt.projectGraph` where it has one, `counts` how many of each resource type. What dbt refuses — a text that does not parse, an empty criterion (`a,,b`), an unknown method, resource type or selector, `state:` / `result:` / `source_status:` (they need a previous run), a cycle between selectors — answers `error: {message, start, end}` (byte offsets into `select`, or into `exclude`) and no nodes; `issues` says what was not evaluated, in words. One engine serves it with every other selector feature: see [selectors.md](selectors.md). `oracles/selection.sh` diffs it with `dbt ls`: on testdata/vault_lake every selection of its tests without `--exclude` is what dbt-core 1.12.7 lists, but `tag:nightly` (dbt ignores a model entry's top-level `tags:`, which the engine reads)
- **Side effects**: none (read-only; reads the project's files off the dispatcher). The "N nodes match" lens on each selectors.yml definition sends it with `selector:<name>`
- **Editors**: Neovim ✓ (`:DbtSelect`, `:Dbt select`, `:DbtFind select`, the navigator's `f` and `/`: live, `<Tab>` completes, `<C-a>` marks every match, `<C-r>` runs it through `dbt.runSelection` with `exclude`) · Emacs ✓ (`M-x cogitator-select`) · VS Code ✓ (`dbt: Select nodes…`, live)

### `dbt.lineageSite`

- **Argument**: `{uri?, assets?, write?}` — `uri` picks the project (multi-root), else the primary one; `assets: true` adds the page's script and stylesheet; `write: true` writes the page instead of answering its data
- **Result**: `{site, counts}` — `site` is the lineage page's data (format 1: the `dbt.projectGraph` nodes and edges, each node's details and columns, and each traced column's inputs one hop up with how it is made); with `assets`, also `assets: {script, style}`; with `write`, `{written: {path, uri}, counts}` and no `site`. `null` with no project. See [lineage-site.md](lineage-site.md)
- **Side effects**: `write: true` writes `target/cogitator-lineage/index.html` and `lineage.json` under the project root; otherwise none (reads model files, compiled SQL and `target/` artifacts off the dispatcher)
- **Editors**: Neovim ✓ (`:DbtLineageSite` writes and opens the page) · Emacs ✓ (`M-x cogitator-lineage-site`, the same) · VS Code ✓ (`dbt: Show lineage view`, a live webview)

### `dbt.modelDependencies`

- **Argument**: `{uri, model?, version?}` — the model named, else the model of `uri`
- **Result**: `{model, parents, children, upstream, downstream}` — each a node `{name, kind, package, uri, line, depth, tags, config: {materialized, incrementalStrategy, uniqueKey, partitionBy, clusterBy, onSchemaChange}, columns}`; `parents` and `children` are the direct neighbours, `upstream` and `downstream` everything, nearest first (tests left out); `columns` a model's inferred output columns, else `null`. `null` when there is no such model
- **Side effects**: none (read-only)
- **Editors**: Neovim ✓ (`:DbtDependencies [model]`, `<leader>dp`, "This model" in the code-action menu) · Emacs ✓ (`M-x cogitator-model-dependencies`) · VS Code ✓ (`dbt: Model dependencies`)

## Profiles, configuration and health

### `dbt.profiles`

- **Argument**: `{uri}` — the document the command is about
- **Result**: redacted profiles inventory
- **Side effects**: warning when no profiles.yml
- **Editors**: Neovim ✓ · Emacs ✓ · VS Code —

### `dbt.setTarget`

- **Argument**: `{target, profile?}`
- **Result**: `{profile, target}` or `null`
- **Side effects**: context refresh + confirmation message
- **Editors**: Neovim ✓ · Emacs ✓ · VS Code —

### `dbt.setProfilesDir`

- **Argument**: `{dir}` — the profiles directory to switch to
- **Result**: `{dir}` or `null`
- **Side effects**: live profiles-directory switch: profile/target overrides reset, context refresh, confirmation message — no client restart
- **Editors**: Neovim ✓ · Emacs ✓ · VS Code —

### `dbt.reloadConfig`

- **Argument**: `{uri}`
- **Result**: `null`
- **Side effects**: forces an immediate cogitator.yml reload pass (otherwise: 2 s stamp poll / didSave fast-path)
- **Editors**: Neovim ✓ · Emacs — · VS Code —

### `dbt.status`

- **Argument**: `{uri}` — the document the command is about
- **Result**: server/project/profiles/refresh/caches/config snapshot; `project.coverage` is `{models, modelsDescribed, columns, documented, tested}` over the project's own models
- **Side effects**: none
- **Editors**: Neovim ✓ · Emacs ✓ · VS Code ✓

### `dbt.doctor`

- **Argument**: `{}`
- **Result**: `{serverVersion, projectRoot/Name, dialect, dbtBinary(+Found), profiles, artifacts (ages incl. catalogAgeS + driftCount), caches, scanCachePersisted, graph, lintEnabled, runningJobs, config}`
- **Side effects**: none — the one health report :checkhealth/doctor render
- **Editors**: Neovim ✓ · Emacs ✓ · VS Code —

### `dbt.cache`

- **Argument**: `{uri, action?}` — `"clear"` empties the derived caches
- **Result**: `{profiles, lineage, results, artifacts, plans}` each `{entries, hits, misses}`
- **Side effects**: `{action: "clear"}` empties the derived caches and refreshes the context
- **Editors**: Neovim ✓ · Emacs ✓ · VS Code —
