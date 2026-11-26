# cogitator.nvim — implementation spec

A Neovim plugin that drives the `cogitator` dbt language server. This spec is
written against the server as of the repository head; every wire shape quoted
here is ground truth from `crates/cogitator-lsp/src/types/init.rs` and
`crates/cogitator/src/server/dispatcher.rs` — when in doubt, those files win.

Target: **Neovim ≥ 0.11** (`vim.lsp.config` / `vim.lsp.enable`). §10 notes the
0.10 fallback via `nvim-lspconfig`. Language: Lua only, no external deps.

---

## 1. Server contract (what the plugin talks to)

### 1.1 Transport and lifecycle

- stdio, standard `Content-Length` framing. Nothing but LSP frames on stdout.
- `initialize` → `initialized` → normal traffic → `shutdown` → `exit`.
  The server honors `workspaceFolders[0]`, then `rootUri`, then the
  deprecated `rootPath`.
- The server never requests dynamic registration. There is no `$/progress`.
- `exit` terminates the process immediately (exit 0). EOF on stdin is also a
  clean shutdown.

### 1.2 CLI flags the plugin may surface

| flag | meaning |
|---|---|
| `--profiles-dir <dir>` / `-p` | profiles.yml directory (`~` expanded) |
| `--fusion [path]` / `-f` | enable dbt Fusion static analysis; optional binary path (default `dbt`) |
| `--no-tree-sitter` | fall back to the phase-1 minijinja+sqlparser pipeline |
| `--debug` / `-d` | debug log to `log.txt` next to the executable |
| `--otlp [endpoint]` | OTLP trace export |
| `-v` / `--version` | print version and exit |

### 1.3 initializationOptions

```json
{ "profilesDir": "/data/dbt-profiles" }
```

`profilesDir` (also accepted: `profiles_dir`) replaces a `--profiles-dir`
launch flag. The server also accepts `profile`, `target` and
`profilesAllow` (`profiles_allow`) — the same overrides as the matching
CLI flags; the Neovim manager store injects profilesDir/profile/target
per project root at attach. Any other shape of `initializationOptions`
is ignored, never an error.

A fourth, **hot** configuration layer sits underneath these:
`<root>/cogitator.yml` (see the README's "Configuration file"), applied
at initialize and re-applied changed-keys-only while the server runs.
Precedence stays defaults < file < CLI < initializationOptions at
launch; later file edits win over earlier runtime switches (last
writer wins).

### 1.4 Advertised capabilities (initialize result)

- `textDocumentSync: 2` (incremental)
- `hoverProvider`, `definitionProvider` (incl. SQL built-ins → their
  bundled docs materialized under `target/.cogitator/docs/`),
  `referencesProvider`,
  `documentSymbolProvider`, `workspaceSymbolProvider`,
  `documentHighlightProvider`, `foldingRangeProvider`,
  `selectionRangeProvider`, `renameProvider`: `true`
- `completionProvider: { resolveProvider: true, triggerCharacters:
  [".", "'", "\"", " ", "("] }`; see §6.3
- `signatureHelpProvider: { triggerCharacters: ["(", ","] }`
- `inlayHintProvider: true` (relation after ref()/source(), macro
  parameter names), `documentLinkProvider: { resolveProvider: false }`
  (ref/source → definition targets), `linkedEditingRangeProvider:
  true` (CTE names only)
- `declarationProvider: true` — a model's (or ref's) properties-YAML
  entry and a documented column's YAML entry; definition stays the SQL
  (a YAML column's definition is the select item producing it)
- `workspace.fileOperations` — willRenameFiles answers the ref/YAML
  edits of a renamed model file (the client renames the file),
  willDeleteFiles an empty edit plus one showMessage warning naming the
  dependents, the did* notifications refresh the context. Neovim 0.11
  sends none of these by itself; VS Code does.
- `implementationProvider: true` — a dispatched macro's
  `<adapter>__x` / `default__x` implementations in dispatch order
  (from the dispatch call, a call of the macro, or its header)
- `documentFormattingProvider: true` — sqlformat over Jinja-segmented
  SQL; full-line `{% %}` lines pass verbatim, `{{ }}` expressions are
  sentinel-masked, and ANY doubt (inline statements, sentinel
  miscount) declines with zero edits
- `codeActionProvider: true` — besides the executable-lens set, the
  response now carries edit-bearing actions: did-you-mean quickfixes
  from diagnostic data, "Convert to ref()/source()" on literal
  relations matching known coordinates (refactor.rewrite), and — at a
  properties-YAML model entry — key-column test scaffolds plus
  inherit-description fixes sourced from column lineage. Historically:
  **command-only** actions (no edits, no
  kinds, no resolve): the executable lens set at the cursor, so every
  stock LSP editor gets dbt workflows without client wiring. A non-empty
  `context.only` filter answers `[]` (the actions are kind-less). On a
  `ref('x')` token, "Open schema of 'x'" is prepended; in a properties
  YAML, the entry at/above the cursor offers "Open model" (via
  `dbt.openModel`) and "Go to tests". In the **active profiles.yml**
  the action set is instead one "Switch target: profile/target" per
  allowed profile×target (the active pair marked) running
  `dbt.setTarget` — target switching without a picker, usable from any
  stock LSP editor including Zed.
- `semanticTokensProvider`: `full: true`, legend token types (in index
  order): `keyword, function, macro, variable, string, number, operator,
  namespace`; no modifiers
- `diagnosticProvider: { interFileDependencies: false, workspaceDiagnostics: false }`
  (document pull diagnostics)
- `codeLensProvider: { resolveProvider: false }`
- `executeCommandProvider.commands`:
  `dbt.goToSchema`, `dbt.columnLineage`, `dbt.openSchema`, `dbt.showLineage`,
  `dbt.run`, `dbt.test`, `dbt.compile`, `dbt.modelGraph`, `dbt.profiles`,
  `dbt.setTarget`, `dbt.extractModelYaml`, `dbt.modelTests`,
  `dbt.goToTests`, `dbt.status`, `dbt.projectGraph`, `dbt.modelInfo`,
  `dbt.relation`, `dbt.compiledSql`, `dbt.parse`, `dbt.openModel`,
  `dbt.setProfilesDir`, `dbt.cache`, `dbt.reloadConfig`,
  `dbt.sourceFreshness`, `dbt.cancel`, `dbt.runSelection`,
  `dbt.deadModels`, `dbt.impact`, `dbt.expandJinja`, `dbt.doctor`,
  `dbt.preview`, `dbt.queryPlan`, `dbt.deps`, `dbt.incrementality`,
  `dbt.splitPlan`, `dbt.incrementalDiff`, `dbt.syntaxTree`, `dbt.highlight`,
  `dbt.astGrep`, `dbt.nodeInfo`, `dbt.unresolved`, `dbt.breakingChanges`,
  `dbt.previewCte`, `dbt.select`, `dbt.lineageSite`, `dbt.syncYaml`,
  `dbt.modelDependencies`
  (append-only: clients may assume index 0 is `dbt.goToSchema`)
- `serverInfo.name == "cogitator"`

### 1.5 Server→client traffic the plugin must handle

- `window/showMessage` — all user feedback, including multi-line
  `Reason: … Solution: …` texts and async dbt run/test/compile outcomes.
- `window/showDocument` (a server→client **request**) — used by
  `dbt.openSchema` and by `dbt compile` success to open the compiled SQL.
  The server ignores the response body, but the client should still reply.
- `textDocument/publishDiagnostics` — push diagnostics while Fusion mode is
  active (pull diagnostics are suppressed server-side in that mode).

### 1.6 workspace/executeCommand argument shape

Every command takes **one argument object**, not positional values:

```json
{ "command": "dbt.run",
  "arguments": [ { "uri": "file:///…/models/customers.sql",
                   "position": { "line": 0, "character": 0 } } ] }
```

`position` is only read by `dbt.goToSchema` and `dbt.relation`. Results:

| command | result | side effect |
|---|---|---|
| `dbt.goToSchema` | `Location` or `null` | none (client jumps) |
| `dbt.openSchema` | `null` | `window/showDocument` or warning |
| `dbt.columnLineage` | lineage JSON or `null` | none |
| `dbt.showLineage` | `null` | formatted `window/showMessage` |
| `dbt.run` / `dbt.test` / `dbt.compile` | `{token}` (immediately) | async showMessage; compile success opens the compiled SQL via showDocument; the token feeds `dbt.cancel` |
| `dbt.parse` | `{token}` (immediately) | async showMessage/$\/progress; project-wide (no `--select`), success refreshes the server context |
| `dbt.deps` | `{token}` (immediately) | async showMessage/$\/progress; project-wide `dbt deps`, success refreshes the server context (the packages.yml quickfix runs it) |
| `dbt.incrementality` | `{models, incremental}` | none; every table model's incrementality verdict, slowest last run first (specs/ir-pipeline/10) |
| `dbt.splitPlan` | `{model, sexp, part: {name, cte, text}}` or `null` | none; the split at the incrementality frontier as an S-expression (specs/ir-pipeline/10 §6) |
| `dbt.incrementalDiff` | `{model, fullRefresh, incremental, diff, identical}` or InvalidParams Guidance | none, or with `show: true` (the "Compare incremental / full refresh" lens) the diff written to `target/.cogitator/incremental/<model>.diff` and opened via showDocument |
| `dbt.modelGraph` | graph payload or `null` | none |
| `dbt.profiles` | redacted profiles inventory | warning when no profiles.yml |
| `dbt.setTarget` | `{profile, target}` or `null` | context refresh + confirmation message |
| `dbt.extractModelYaml` | `null` | workspace/applyEdit (CreateFile scaffold, or version TextEdits) + showDocument |
| `dbt.modelTests` | `{model, tests: [{name, column?, version?, uri, range}]}` or `null` | none |
| `dbt.goToTests` | `null` | showDocument to the first test + INFO list |
| `dbt.status` | server/project/profiles/refresh/caches/config snapshot | none |
| `dbt.projectGraph` | `{nodes: [{id, name, kind, layer, uri, tags, columns}], edges: [{from, to}]}` | none |
| `dbt.modelInfo` | `{model, uri, description, tags, upstream, downstream, versions, tests, compiledUri}` or `null` | none |
| `dbt.relation` | `{kind, name, source, database, schema, identifier, relation, origin}` or `null`; `origin` is `"manifest"` where target/manifest.json renders the model, else `"defaults"` | none |
| `dbt.compiledSql` | `{model, uri, content}` or `null` | WARNING when the file is over the wire cap (2 MiB default; `limits.compiled_sql_mib`) |
| `dbt.openModel` | `null` | showDocument to the model's file (`{uri}` or `{model}`), or a warning |
| `dbt.setProfilesDir` | `{dir}` or `null` | live profiles-directory switch: profile/target overrides reset, context refresh, confirmation message — no client restart |
| `dbt.cache` | `{profiles, lineage, results, artifacts, plans, highlights}` each `{entries, hits, misses}` | `{action: "clear"}` empties the derived caches and refreshes the context |
| `dbt.reloadConfig` | `null` | forces an immediate cogitator.yml reload pass (otherwise: 2 s stamp poll / didSave fast-path) |
| `dbt.sourceFreshness` | `{token}` (immediately) | async `dbt source freshness` (project-wide); the refreshed sources.json feeds the freshness lenses on source YAML entries |
| `dbt.runSelection` | `{token}` or InvalidParams | runs dbt with an arbitrary validated selector (`{selector, exclude?, kind?: run\|test\|build\|compile}`; `exclude` becomes `--exclude`) |
| `dbt.cancel` | `{token, label}` or InvalidParams | aborts the running invocation (the child process is killed) + confirmation message |
| `dbt.deadModels` | `{models: [{name, uri, reasons}]}` | none (conservative: seeds/package models never flagged; selector mentions keep a model alive) |
| `dbt.impact` | `{model, column, summary, models: [{name, uri, depth}], exposures, columnHits, truncated, reach?: {summary, models, tests: [{model, column, test}], semanticItems: [{semanticModel, model, column, list?, name?, uri, range}], semanticModels, metrics: [{name, uri, range, via}], exposures: [{name, level}], dashboards: [{name, url?, level, model, column?}], notes?}}` | none (column hits are an under-approximation: `*` and unresolved lineage stay out; lineage fan-out capped at 200 models). `reach` only with a `column`: tests on every hit, semantic-model items whose `expr:` (or name) reads a hit, the metrics over those measures, exposures at node level, dashboards of cogitator.yml `bi.manifest` (docs/column-reach.md) |
| `dbt.expandJinja` | `{sql}` or InvalidParams Guidance | renders the open buffer with minijinja — project macros, ref/source → relations, var(), config()/is_incremental()/target stubs; adapter calls need a real compile |
| `dbt.doctor` | `{serverVersion, projectRoot/Name, dialect, dbtBinary(+Found), profiles, artifacts (ages incl. catalogAgeS + driftCount), caches, scanCachePersisted, graph, lintEnabled, runningJobs, config}` | none — the one health report :checkhealth/doctor render |
| `dbt.queryPlan` | `{model, dialect, backend, schemaRevision, status: complete\|partial\|declined, guidance, origin, levels: {hir, mir, lir: {available, fidelity, text, unavailable, degradations, violations}}, findings, trace: {entries, incomplete}, needs: {available, missing}}` | none (read-only): runs the IR pipeline on the open buffer or file; a level dropped by a verifier violation is shown as unavailable; models with control flow other than a lone `is_incremental()` come back `declined` with a Guidance pointing at `dbt compile` until compiled fallback lands — :DbtPlan |
| `dbt.syntaxTree` | `{nodes: [{depth, kind, field, error, missing, range}], truncated}` or InvalidParams | none (read-only): the open document's tree-sitter-dbt tree, named + ERROR + MISSING nodes in pre-order, ranges in UTF-16, at most 20 000 nodes — :DbtSyntaxTree |
| `dbt.preview` | `{token}` (job started) | `dbt show --select <model> --limit N` (N from `limit`, default 50) through the job registry; on success the result table lands in `target/.cogitator/preview/<model>.txt` and opens via showDocument — :DbtPreview [limit] |
| `dbt.previewCte` | `{model, cte, columns, rows, truncated, limit, sql, source: compiled\|rendered, uri, table}` (answered when dbt finishes; `RequestCancelled` after dbt.cancel), or RequestFailed Guidance | `{uri, cte?, position?, limit?}`: the model's SQL — `target/compiled` when it is no older than the model and the buffer is saved, else the dbt.expandJinja rendering — cut at `cte` (or the CTE under `position`; neither: the final select) with sqlparser into `<with up to cte> select * from <cte> limit N+1`, refused unless one SELECT; `dbt show --inline <sql> --limit N+1 --output json` (active target, profiles dir/profile, `--vars`) as a job (token in the `$/progress` title, dbt.cancel); N from `limit`, else cogitator.yml `preview.rows` (20), capped at `preview.max_rows` (1000); the markdown table lands in `target/.cogitator/preview/<model>.<cte\|final>.md` and opens via showDocument; every CTE and the final select carry a "Preview rows" lens — :DbtPreviewCte [limit] |
| `dbt.highlight` | `{language, version?, spans: [{range, capture, layer}], truncated, layers}`, or `null` for compiled SQL that does not exist (an uncompiled model or test, a macro) | none (read-only): the grammars' highlights (specs/highlighting §2) of `{uri, language?}` (open text, else the file; the language from the path: `.yml` → yaml, `target/compiled\|run` → sql, else dbt), `{text, language?: dbt\|sql\|yaml}`, `{model?, compiled: true}` or `{node: {kind, name}, compiled?}`; `range: {start: {line}, end: {line}}` answers those lines' spans, `version` is echoed; YAML gets its base layer, Jinja and SQL inside values and dbt-aware captures (§3–§5); whole answers cached per document generation / file stamp; at most 50 000 spans, nothing past 2 MiB |
| `dbt.astGrep` | `{language, files, matches: [{uri, range, text, env, replacement}], truncated}` or InvalidParams Guidance | ast-grep (`cogitator-astgrep`) over tree-sitter-dbt (`dbt`: Jinja structure; the SQL is opaque text) or tree-sitter-sequel (`sql`, compiled SQL, whole statements): `{pattern, language?, rewrite?, uri?}` — `uri` searches that document, else the project's dbt files (`dbt`) or `target/compiled` (`sql`); at most 500 matches; `apply: true` with a `rewrite` sends workspace/applyEdit. The project's `sgconfig.yml` rules (`language: dbt\|sql`) are pull diagnostics (`source: ast-grep`) and their `fix` a quickfix |
| `dbt.nodeInfo` | `{kind, name, …}` per kind (specs/highlighting §6.1), or `null` + a WARNING with the closest names | none (read-only): `{node: {kind, name}}` — `model` (modelInfo's answer + kind), `macro` (package, description, params, uri, range, callers, calls, usedByTests), `test` / `analysis` / `source` / `exposure` / `metric` / `semantic_model` (uri, refs, calls, upstream, downstream, compiledUri for tests and analyses), `generic_test` (a `{% test %}` definition: params, uri, range, appliedTo [{model, column, uri, range}]) |
| `dbt.breakingChanges` | `{model, status, baseline, summary, diff: {removed, added, retyped}, impacts: [{column, change, severity, tests, downstream, semantic?, metrics?, range?, keepAlias?}], exposures, public, truncated, sinceSave?}` — the buffer's columns against `target/catalog.json`, else the manifest's documented columns, and (`sinceSave`) against the file as last saved | none (read-only): `{uri}`; `status: "noBaseline"` without target/ artifacts and without an unsaved edit; also `breaking-change` diagnostics (anchored at the column's item, related information, "Keep '<old>' as an alias") and an `N breaking changes` lens, computed on open/save and after the refresh debounce on change |
| `dbt.unresolved` | `{items: [{kind, name, uri, range, message, suggestions?}], count, byKind, truncated}` — every unresolved name of the root project, by file then position (at most 5 000 items) | none (read-only): `{kinds?: ["ref"\|"source"\|"source_table"\|"doc"\|"macro"\|"var"]}`; open documents' unsaved text counts |
| `dbt.select` | `{selector, count, counts: {resourceType: n}, nodes: [{uri, name, label, kind, resourceType, uniqueId, version, package, path, tags, range, graphId}], issues, error: {message, start, end} \| null, vocabulary?: {methods, values: {method: [value]}}}` — what `dbt ls --select … --exclude …` lists, by resource type then name; an error lists no node | none (read-only): `{select, exclude?, vocabulary?}` (`selector` for `select`, as the selectors.yml lenses send it); `select` may carry its own `--exclude …`; error offsets are bytes into `select` (or `exclude`); docs/selectors.md — `:DbtSelect`, the navigator's `f` and `/` |
| `dbt.lineageSite` | `{site, counts}` (`site`: format 1 — the `dbt.projectGraph` nodes/edges with each node's details and columns, each traced column's `inputs` one hop up and its `transform`); with `assets: true` also `assets: {script, style}`; with `write: true` `{written: {path, uri}, counts}` instead of `site`; `null` with no project | `{uri?, assets?, write?}`; `write` writes `<root>/target/cogitator-lineage/index.html` + `lineage.json` — :DbtLineageSite, `cogitator lineage-site` (docs/lineage-site.md) |
| `dbt.syncYaml` | `{changes, applied, files: [{uri, created, edits, diff}], models: [{model, uri?, createdEntry, added, inherited: [{column, from, via?}], typed: [{column, dataType}], stale, pruned, reordered, undocumented, skipped: [{column?, reason}], changes}]}` | `{uri?, models?, prune?, inheritTypes?, reorder?, newEntries?: "model"\|"folder", apply?}`: the schema-YAML sync plan (docs/yaml-sync.md); with `apply: true` the server sends it as workspace/applyEdit (label "Sync schema YAML") — :DbtSyncYaml |

On duckdb-feature builds with cogitator.yml `warehouse: {columns:
true}`, source tables whose YAML declares no columns get them from the
warehouse's information_schema during the context refresh (read-only,
never on a request path); completion/hover then show them with their
data type marked "(from warehouse)".

Two-argument `ref('package', 'model')` resolves like plain `ref()`
everywhere (hover, definition, completion, dangling checks): the first
string refines as the package, the second as the model. packages.yml /
dependencies.yml buffers warn per declared-but-not-installed package.

`dbt.relation` resolves the `source()` table or `ref()` under `position`
(otherwise the file's own model) to its warehouse coordinates. **dbt
defaults only**: a source without `schema:` uses the source name, a table
without `identifier:` its declared name, and model database/schema come
from the active profiles target (duckdb: the `path` file stem and `main`).
Custom `generate_schema_name`/`generate_database_name` macros and
`models:` config trees (`+schema`/`+database`/`+alias` in
dbt_project.yml) are **not** evaluated. Unresolvable parts are `null` and
left out of the dot-joined `relation` string.

`dbt.compiledSql` takes `{uri?, model?}` (`model` wins, like
`dbt.modelInfo`) and serves the compiled file's text so clients need no
filesystem path math; `uri`/`content` are `null` when dbt has not written
`target/compiled/…` yet, and `content` alone is `null` (with a WARNING
message) past the 2 MiB cap — open the `uri` directly then.

`workspace/symbol` additionally understands `tag:x` (models carrying tag
x — from YAML `tags:`/`config.tags` and `{{ config(tags=…) }}`) and
`selector:x` (selectors.yml entries) queries.

Deliberate non-goals of this surface: no `manifest.json` /
`run_results.json` fallback (the server's own analysis is always live;
manifest parsing is a specs/rust-port/00 non-goal), no full Jinja render
command (minijinja only tokenizes — 05 §4), no call hierarchy
(`dbt.columnLineage` covers column flows), no `dbt.syntaxTree`, no
custom `generate_schema_name`-style macro evaluation in `dbt.relation`,
no editor-side tree-sitter grammar builds (highlighting rides the
server's semantic tokens), and no dedicated `cache` workflow beyond
`dbt.status`'s cache counters and `dbt.cache` (stats/clear).

### 5.3b Project DAG view (`:DbtDag`) — **Neovim only**

`lua/cogitator/dag.lua` renders `dbt.projectGraph` as a dbt-docs-style
DAG in a scratch tab: layers left→right, box-drawn nodes with `#tags`,
orthogonal edges with corner glyphs (never overdrawing box content), and
docglow-style column information inside expanded nodes. Nodes are filled
in the dbt-docs resource colours (`src/app/services/graph.service.js`,
groups `DbtDag*` in `cogitator.highlight`, defined with `default = true`);
the selected node and its edges are recoloured in a separate extmark
namespace so moving the selection never rebuilds the grid.
Exactly one node is selected, and movement is node to node (`cogitator.dag_nav`,
pure): `h`/`l` go to a connected parent/child (nearest by row, or back
along the path just taken), `j`/`k` through the same layer, `gg`/`G`
first/last node (or node `[count]`), `{`/`}` first/last layer (or layer
`[count]` from that end), `/` live name search with `n`/`N`,
`<C-o>`/`<C-i>` selection history; counts repeat `hjkl`, `n`/`N` and the
history keys. Native motions and the mouse move the selection through a
`CursorMoved` hit test. Other keys: `<CR>` open, `c` toggle this node's
columns, `C` toggle all, `L` the column-lineage graph (of the model; on
an expanded column line, of that column; in a column graph, re-centred
on the column under the cursor), `m` back to the model graph, `f` focus
on the selection's neighbourhood (`cogitator.dag_data.focus`, relayered)
and `+`/`-` its depth, `F` follow the model of the buffer entered
elsewhere, `R` refresh (also after every save when
`dag.refresh_on_save`), `q` close; the winbar names the mode.
The column graph (`cogitator.column_lineage`) is walked from
`dbt.columnLineage`, one model per request: upstream through the
`model`/`source` refs of each column, downstream through the project
graph's children whose columns read it, bounded by `lineage.depth` hops
and `lineage.max_requests` requests; it reuses the DAG's layout, as a
payload of column nodes (`model:<m>::<col>`, `source:<s>.<t>::<col>`,
`unknown:<name>::<col>`) with layers from the longest path. The server command is editor-neutral; this UI is
deliberately implemented only in the Neovim plugin.

### 5.3c Model explorer (`:DbtBrowse`, `:DbtExplorer`) — **Neovim only**

One model at a time, in five floats:

```
┌ upstream ⯇ (n) ┐┌ model · a › b › c ──┐┌ ⯈ downstream (n) ┐
│ direct parents ││ the model's card    ││ direct children  │
└────────────────┘└─────────────────────┘└──────────────────┘
┌ dbt SQL · x.sql ───────────────┐┌ compiled SQL ───────────────────┐
│ the file (Jinja and all)       ││ target/compiled/<project>/…     │
└────────────────────────────────┘└─────────────────────────────────┘
```

- **Opens** on the current buffer's model; elsewhere it asks with the
  models picker (§5.3e), and closes if nothing is chosen.
  `open({ focus = name })` opens on a given model.
- **The card** (centre, `explorer/card.lua`) shows the model's name and
  kind with its file, the description (wrapped), tags, the `dbt.relation`
  name, versions (`<CR>` opens a version's file), the documented columns
  from the project graph and the tests (`<CR>` jumps to the test's YAML).
  `<CR>` on any other line opens the model.
- **The side panes** list the direct upstream and downstream from
  `dbt.modelInfo`, in the `DbtDag*Fg` resource colours. `<CR>` on an
  entry, or `h` (upstream) / `l` (downstream) pressed again in its pane,
  makes it the model and stays in the pane, so repeating walks the
  lineage (`[count]` walks that many; steps queue while `dbt.modelInfo`
  is in flight). From the card, `h`/`l` enter the panes.
- **History:** every re-centring (a walk, `f`/`/`, `<C-g>` from a picker)
  records the model left behind. `[`/`-` go back and `]` forward, and the
  card's title shows the trail (`a › b › c`).
- **The two SQL panes** show the dbt SQL (the loaded buffer when there is
  one, else the file read in the background) and the compiled SQL from
  `dbt.compiledSql`. When the model has not been compiled they say so;
  the explorer never runs dbt itself. Both are `sql` buffers; `J`/`K`
  scroll them together from any pane, `p` enters the dbt SQL, `h`/`l`
  move between the two, and `<Esc>` returns to the card.
- **Fetching:** changing the model fetches `dbt.modelInfo`,
  `dbt.compiledSql` and then `dbt.relation`, debounced; answers for an
  older model are dropped (generation counter). The project graph is
  cached per client (`cogitator.pick.graph`) and dropped on each
  `$/progress` end.
- **Keys** (on the card, or on a side entry):
  - `f` or `/` pick any model (the fuzzy picker, fzf syntax);
  - `.` lets the picker and the walks take every kind of node
    (`navigator.show_all_kinds`);
  - `<Space>` marks; `y` yanks and `r` dbt-runs the marked models, or the
    entry (one `dbt.runSelection`);
  - `o`/`s`/`v`/`t` open / split / vsplit / tab; `c` compile; `g` show in
    :DbtDag; `L` column lineage; `P` dbt parse;
  - `?` lists the keys; `q` closes, as does `<Esc>` on the card.

The implementation is the `cogitator.explorer` package (init/render/card/
preview/keys), with `cogitator.browse` as a facade; `explorer/filter.lua`
keeps only the substring match :DbtDag's search uses. The server commands
are editor-neutral; this UI is deliberately implemented only in the
Neovim plugin.

`dbt.modelInfo` takes `{uri?, model?}` (`model` wins) and answers
everything the browser's preview needs in one round trip:

```json
{ "model": "customer_orders", "uri": "file:///…/customer_orders.sql",
  "description": "", "tags": [],
  "upstream":   [{ "name": "orders", "kind": "model", "uri": "file:///…" }],
  "downstream": [],
  "versions": { "latestVersion": 2,
                "versions": [{ "v": 1, "uri": "file:///…_v1.sql" },
                             { "v": 2, "uri": "file:///…customer_orders.sql" }] },
  "tests": [{ "name": "not_null", "column": "order_count", "version": 1,
              "uri": "file:///…/customer_orders.yml", "range": { … } }],
  "compiledUri": null }
```

`upstream`/`downstream` entries are the `dbt.modelGraph` shape, `tests`
the `dbt.modelTests` shape. `versions` is `null` for an unversioned
model; a version's `uri` follows dbt's file convention
(`<model>_v<N>.sql`, the latest version may live in `<model>.sql`).
`compiledUri` is non-null only when
`target/compiled/<project>/<model path>` exists on disk (kept for
display/back-compat). The browser's compiled tab fetches the text
through `dbt.compiledSql` instead of reading that file itself.

When the client advertises `window.workDoneProgress`, context refreshes
and dbt run/test/compile report through `window/workDoneProgress/create`
+ `$/progress` instead of the start/success showMessages (failures keep
their Reason/Solution messages).

`dbt.profiles` answers `{ path, searched, activeProfile, activeTarget,
profiles: [{ name, defaultTarget, allowed, targets: [{ name, type,
auth: { method, issues: [string] } }] }] }` — auth methods and problems,
never credential values; profiles outside the allowlist appear by name
with `allowed: false`. `dbt.setTarget` takes
`{ "target": "prod", "profile"?: "name" }`.

`dbt.modelGraph` also accepts `{ "model": "<name>" }` in place of `uri`
(re-centering) and answers with direct neighbors only:

```json
{ "model": "customers", "uri": "file:///…/customers.sql",
  "upstream":   [ { "name": "stg_customers", "kind": "model",  "uri": "file:///…" } ],
  "downstream": [ { "name": "mart_x",        "kind": "model",  "uri": "file:///…" } ] }
```

`kind` is `model` or `source`; `uri` is `null` for dangling refs and for
sources without a defining YAML.

---

## 2. Plugin layout

```
cogitator.nvim/
├── plugin/cogitator.lua          -- guard + user commands (no setup needed)
├── lua/cogitator/
│   ├── init.lua                  -- setup(opts), vim.lsp.config registration
│   ├── config.lua                -- defaults + validation
│   ├── commands/                 -- :Dbt* user commands → executeCommand: a module
│   │                             --   per concern (jobs, lists, analysis, views, node,
│   │                             --   split, navigation), gathered by init.lua
│   ├── picker/                   -- the yazi-style dialogs: match, view, window, keys,
│   │                             --   dialogs (select/input), init (state, open)
│   ├── pick/sources/             -- what :DbtFind lists, a module per origin
│   ├── paths.lua, positions.lua  -- shared display paths and LSP → byte columns
│   ├── lens.lua                  -- codelens refresh autocmds
│   └── health.lua                -- :checkhealth cogitator
└── doc/cogitator.txt
```

**Installing with lazy.nvim.** The plugin is a subdirectory of the server's
repository and lazy.nvim has no subdirectory option, so the repository root
carries a `lazy.lua`, which lazy.nvim adds to the plugin's own spec: `init`
appends `editors/nvim` to the runtimepath and tags its `doc/` (lazy.nvim tags
and sources only the root's `doc/` and `plugin/`), `main = "cogitator"`, and
`config` sources `plugin/cogitator.lua` and calls `setup(opts)`. A user's
spec is then `{ "sirh3e/dbt-language-server", opts = {} }`;
`editors/nvim/examples/lazy.lua` adds lazy-loading on `BufReadPre` of
SQL/YAML (not `ft`, after the `FileType` the client is enabled on) and an
opt-in `build` running `install.sh`/`install.ps1`. `just nvim-lazy-test`
installs this checkout through lazy.nvim, both ways, in a sandbox (network:
it clones lazy.nvim).

Rules: `require("cogitator")` must be cheap (no I/O at require time); all
`vim.fn`/`vim.loop` probing happens in `setup()` or lazily. Every module
≤ ~200 lines.

---

## 3. Configuration surface

```lua
require("cogitator").setup({
  cmd = { "cogitator" },        -- or full path; extra args appended from opts below
  profiles_dir = nil,           -- string → sent as initializationOptions.profilesDir
  fusion = false,               -- false | true | "/path/to/dbt"
  tree_sitter = true,           -- false → --no-tree-sitter
  debug = false,                -- true → --debug
  filetypes = { "sql", "yaml" },
  root_markers = { "dbt_project.yml" },
  attach_outside_project = false, -- see §4.2
  lens = { enabled = true, events = { "BufEnter", "CursorHold", "InsertLeave" } },
  keymaps = true,               -- buffer-local defaults of §5.3
  lineage = { depth = 3, max_requests = 60 },  -- column-lineage graphs (§5.3b)
  dag = { focus_depth = 2, follow = false, refresh_on_save = true },
  grammar = { highlight = true, auto_install = false, url = nil, rev = nil }, -- §5.3d
  -- colours from the server (specs/highlighting §7.1)
  highlight = { server = "auto", layers = { yaml = true, sql = true, jinja = true, dbt = true } },
})
```

Validation with `vim.validate`; unknown keys are an error (fail loudly at
setup, never mid-session). `fusion = true` appends `--fusion`;
`fusion = "<path>"` appends `--fusion <path>`. `profiles_dir` goes through
`init_options`, **not** the CLI, so one Neovim instance can serve several
projects with different launch-time settings later.

---

## 4. Server registration and attach

### 4.1 vim.lsp.config (0.11+)

```lua
vim.lsp.config("cogitator", {
  cmd = opts.cmd_with_flags,
  filetypes = opts.filetypes,
  root_markers = opts.root_markers,
  init_options = opts.profiles_dir and { profilesDir = opts.profiles_dir } or nil,
  capabilities = vim.tbl_deep_extend("force", vim.lsp.protocol.make_client_capabilities(), {
    window = { showDocument = { support = true } },
    workspace = {
      workspaceEdit = {
        documentChanges = true,
        resourceOperations = { "rename" },   -- model rename renames the file
      },
    },
  }),
})
vim.lsp.enable("cogitator")
```

### 4.2 Attach discipline

- Attach **only** when a `dbt_project.yml` is found upward from the buffer
  (`root_markers` does this), unless `attach_outside_project = true`. A dbt
  LSP on arbitrary SQL files is noise.
- YAML buffers: attach under the same root; the server serves schema-YAML
  diagnostics and property completion for them.
- One client per root dir — `vim.lsp.enable` already reuses by
  `(name, root_dir)`; do not spawn manually with `vim.lsp.start` in
  addition.

### 4.3 Handlers

- `window/showDocument`: Neovim 0.11 ships a default handler; keep it.
  Verify in an integration test that the schema file opens and the selection
  is applied.
- `window/showMessage`: route through `vim.notify`, mapping
  Error→`vim.log.levels.ERROR`, Warning→`WARN`, Info→`INFO`. The server
  sends multi-line bodies; pass them through unchanged (notifier plugins
  render them fine; `:messages` keeps them greppable).

---

## 5. User commands and keymaps

### 5.1 Commands (`plugin/cogitator.lua`, buffer-aware)

| command | executeCommand | notes |
|---|---|---|
| `:DbtRun` | `dbt.run` | current buffer's uri |
| `:DbtTest` | `dbt.test` | |
| `:DbtCompile` | `dbt.compile` | compiled SQL opens via showDocument |
| `:DbtParse` | `dbt.parse` | project-wide; refreshes artifacts + server context |
| `:DbtRelation` | `dbt.relation` | yanks the resolved relation into `"` (and `+` when a clipboard exists), `<leader>dR` |
| `:DbtTarget` | `dbt.profiles` + `dbt.setTarget` | picker over allowed profile×target; the choice persists per project in the manager store, `<leader>dT` |
| `:DbtProfiles [dir]` | `dbt.setProfilesDir` | no arg: candidate report; with dir: live switch (persisted per root; restart fallback for servers without the command) |
| `:DbtProjects` | — (manager store) | picker over every project root ever attached; opening its dbt_project.yml attaches the per-root client, `<leader>dP` |
| `:DbtFind [source]` | `dbt.projectGraph`, `workspace/symbol`, `dbt.modelTests`, `textDocument/documentSymbol` | fuzzy pickers (§5.3e); no argument: pick the source |
| `:DbtReferences[!]` | `textDocument/references` | every use of what the cursor names (`!`: the file's own model), in the yazi-style dialog, a group per file and a line per use with its preview; the first entry renames it — **Neovim only** |
| `:DbtMacros` | `workspace/symbol`, `textDocument/references` | every macro in the yazi-style dialog; choosing one shows where it is called (asked at its header, which the server reads from disk when the file is not open) — **Neovim only** |
| `:DbtRename[!] [name]` | `textDocument/prepareRename`, `textDocument/rename` | rename what the cursor names, else (or with `!`) the file's model, asked past its last line; the WorkspaceEdit is shown (apply / cancel / a diff per file) and applied only on "apply" — **Neovim only** |
| `:DbtPlan` | `dbt.queryPlan` | each IR level, findings, the incrementality analysis and frontier as S-expressions, and the rule trace (including skip reasons) in a scratch split |
| `:DbtIncrementalDiff` | `dbt.incrementalDiff` | the model rendered with `is_incremental()` false and true, as a unified diff in a scratch split (`diff` filetype); `q` closes |
| `:DbtSplit` | `dbt.splitPlan` | the split at the incrementality frontier as an S-expression and the new model's file in a scratch split; `a` applies the split code action, `q` closes |
| `:DbtIncrementality` | `dbt.incrementality` | every table model with its verdict, strategy, last run time and why, as a quickfix list |
| `:DbtColumnLineage [column]` | `dbt.projectGraph` + `dbt.columnLineage` | the column-lineage graph of this model (or one column) in :DbtDag (§5.3b) |
| `:DbtDeps` | `dbt.deps` | project-wide; the job token is kept for `:DbtCancel` |
| `:DbtReloadConfig` | `dbt.reloadConfig` | re-read cogitator.yml now |
| `:DbtSyntaxTree` | `dbt.syntaxTree` | toggle the syntax-tree split (§5.3d) |
| `:DbtHighlight[!] [auto\|always\|off]` | `dbt.highlight` | toggle the buffer's colours from the server (`cogitator.tshl`: a namespace and priority per layer — yaml 120, sql 124, jinja 126, dbt 128 — captures → `@<capture>` groups, regex syntax off once painted, repainted 150 ms after edits, older answers dropped, the visible window first past 2 000 lines); `!` sets `highlight.server` for the session. With `highlight.server = "auto"` (default) SQL buffers without Neovim's tree-sitter highlighter and every YAML buffer are coloured on attach (specs/highlighting §7.1); the explorer's SQL panes and picker previews always are |
| `:DbtNode [kind:]name` | `dbt.nodeInfo` | a node's card in a float (macro, test, generic test, source…); kinds tried in order with `quiet` when none is given; `<CR>` opens the line's target |
| `:DbtBreakingChanges` | `dbt.breakingChanges` | this model's removed, possibly renamed and retyped columns against its last built schema, and what reads them downstream, into quickfix |
| `:DbtLineageSite` | `dbt.lineageSite` | `write: true`, then `vim.ui.open` on the written page; a message with the counts; never blocks |
| `:DbtSyncYaml [model…] [%] [prune] [types] [reorder] [folder]` | `dbt.syncYaml` | the plan (each model's changes, then every file's diff) in a split, then applied on confirm through `apply: true`; `%` narrows to this buffer's model or entries; nothing to change is a message |
| `:DbtUnresolved [kind…]` | `dbt.unresolved` | every unresolved ref, source, source table, doc, macro and var of the project into quickfix; `:DbtFind unresolved` shows the same list in the picker |
| `:DbtSelect [selection]` | `dbt.select` | the navigator's finder over a dbt selection (`tag:x`, `+model`, `resource_type:test`, `a,b`, `--exclude …`), live as it is typed; completion of methods and values |
| `:DbtAstGrep [sql:]pattern [=> rewrite]` | `dbt.astGrep` | matches over the project into quickfix; with a rewrite, the matches listed and applied after a confirmation (`apply: true`); `:DbtFind astgrep` / `<leader>da` asks for a pattern and shows the matches in the picker |
| `:DbtTreesitter [install\|status\|start]` | — | the tree-sitter-dbt grammar in Neovim (§5.3d) |
| `:DbtProjects!` | — (manager store) | manage one project: open, profiles directory, clear selections, rename (`label`), forget |
| `:Dbt <sub> [args]` | — | every command above as a subcommand, completed (subcommands, `cache clear`, profiles dirs, the buffer's column names) |
| `:DbtQuery` | `dbt.relation` | `select * from <relation> limit <query_limit>` through vim-dadbod when configured, else a scratch sql buffer, `<leader>dx` |

The manager store is a JSON file (default
`stdpath("data")/cogitator/manager.json`, override via setup
`manager_path`) holding `{projects = {[root] = {profiles_dir?, profile?,
target?, last_used}}}`. Stored selections are merged into
`initializationOptions` per root through `before_init` at attach, and
applied live through `dbt.setProfilesDir`/`dbt.setTarget` while a client
runs.
| `:DbtSchema` | `dbt.goToSchema` | uri + cursor position; jump to the returned `Location` with `vim.lsp.util.show_document` |
| `:DbtLineage` | `dbt.showLineage` | result arrives as a showMessage |

Implementation: `client:exec_cmd({ command = …, arguments = { { uri = vim.uri_from_bufnr(0), position = <cursor> } } }, { bufnr = 0 }, handler)`.
Nothing blocks the editor: no `request_sync`, `vim.wait` or `:wait()` in
the plugin (the headless suite scans for them). Other LSP methods go
through `actions.request` (a timeout, a staleness guard, cancel), and what
must answer synchronously — command-line completion of column names,
`:checkhealth` — reads a cache filled in the background (`dbt.columnLineage`
on attach/enter/save, `dbt.doctor` on attach and after each `$/progress`
end). The DAG paints large graphs' highlights in slices, the visible lines
first.
Only `:DbtSchema` needs a result handler; the rest are fire-and-forget
(`null` results). All five must error cleanly ("cogitator is not attached to
this buffer") when no client is attached.

### 5.2 Code lens

- When `lens.enabled`, create an augroup per attached buffer:
  `vim.lsp.codelens.refresh({ bufnr = bufnr })` on the configured events and
  once on attach.
- `vim.lsp.codelens.run()` executes the lens under the cursor; it sends the
  lens's `command.arguments` verbatim — this matches §1.6, no translation
  layer needed.
- **Macro call-site lenses carry `command = ""`** (informational only). The
  plugin must not offer them for `run` (filter in a small wrapper around
  `vim.lsp.codelens.run`, or accept Neovim's no-op).
- Model lenses in display order: `N downstream`, `N upstream`,
  `Go to schema`, `Column lineage`, `dbt run`, `dbt test`, `dbt compile`.

### 5.3 Model-graph browser (`:DbtGraph`, `lua/cogitator/graph.lua`)

A yazi-style three-pane float over `dbt.modelGraph`: direct upstream on the
left, the current model (name, path, counts) in the center, direct
downstream on the right. Keys: `h`/`l` move focus between panes, `j`/`k`
select in a side pane, `<CR>` re-centers the graph on the selected neighbor
(a `dbt.modelGraph` call by `model` name), `o` opens the selected entry's
file (closing the browser), `q`/`<Esc>` close. Sources are tagged
`[source]` and refuse re-centering with an INFO notice; entries without a
`uri` are tagged `(missing)`. One instance at a time; the fetch runs
through the originating buffer's client.

### 5.3e Fuzzy pickers (`:DbtFind [source]`) — **Neovim only**

`lua/cogitator/pick/`: a source (`title`, `items(done)`, `preview(item,
done)`, optional `choose`) shown by a backend — fzf-lua, snacks.picker or
telescope when installed (`picker = "auto"`, in that order), or a
named one (`picker = "fzf-lua" | "snacks" | "telescope" | "builtin"`);
one that is missing or fails falls back to the built-in picker (prompt,
ranked list with matches highlighted, preview; fzf's syntax via
`cogitator.pick.query` and `vim.fn.matchfuzzypos`). The same keys
everywhere: `<CR>` open, `<C-v>`/`<C-x>`/`<C-t>` vsplit/split/tab,
`<C-g>` the navigator (for a column: its column-lineage graph), `<C-y>`
yank, `<C-r>` dbt run, `<Tab>` mark. Items and previews arrive
asynchronously; nothing waits.

| source | items | default action |
|---|---|---|
| `models`, `sources`, `all` | `dbt.projectGraph` nodes | open the file |
| `definitions`, `macros`, `selectors` | `workspace/symbol` | jump to the definition |
| `columns` | documented columns as `model.column` | open the model on the column |
| `upstream`, `downstream`, `lineage` | BFS over the graph from the buffer's model, depth shown | open |
| `refs` | the buffer's `ref()`/`source()` calls, CTEs and macros | go to definition |
| `tests`, `tags` | `dbt.modelTests`; the graph's tags, then their nodes | jump / open |

`:DbtTests`, `:DbtTags` and `:DbtSelectors` open these sources, and every
other choice the plugin offers (`:Dbt`, `:DbtCancel`, targets, projects)
goes through `pick.select`, a `vim.ui.select` on the same backends.

### 5.3d Syntax tree and grammar — **Neovim only**

`:DbtSyntaxTree` (`lua/cogitator/syntax.lua`) draws `dbt.syntaxTree` in a
split, one node per line; the tree and the source cursor follow each
other, `<CR>` jumps, edits refetch after 250 ms. `:DbtTreesitter install`
(`lua/cogitator/treesitter.lua`) fetches the tree-sitter-dbt revision the
server pins (`M.REV`, held equal to Cargo.toml by the suite), compiles
`src/parser.c` + `src/scanner.c` with `$CC`/`cc`/`zig cc` into
`stdpath("data")/site/parser/dbt.so` and copies `queries/*.scm`; with
`grammar.highlight` the plugin starts it on attached `sql` buffers via
`vim.treesitter.start` (the filetype stays `sql`).

### 5.4 Default buffer-local keymaps (when `keymaps = true`)

A `<leader>d` prefix, deliberately clear of Neovim 0.11's built-in `gr*`
LSP maps: `<leader>ds` → `:DbtSchema`, `<leader>dl` → `:DbtLineage`,
`<leader>dr` → `:DbtRun`, `<leader>dt` → `:DbtTest`, `<leader>dc` →
`:DbtCompile`, `<leader>dg` → `:DbtGraph`, `<leader>dR` →
`:DbtRelation`, `<leader>dL` → `vim.lsp.codelens.run`, `<leader>dv`
(normal and visual) → expand along `textDocument/selectionRange`,
`<leader>dV` (visual) → shrink back, `<leader>df` → `:DbtFind models`,
`<leader>dF` → `:DbtFind`, `<leader>dd` → definitions, `<leader>dk` →
columns, `<leader>du`/`<leader>dU` → upstream/downstream, `<leader>dn` →
the navigator at this model. All with `desc` set, all buffer-local on
attach only.

---

## 6. Feature-specific client notes

### 6.1 Rename (`textDocument/rename`)

The server returns one `WorkspaceEdit` with `documentChanges`: text edits in
dependent files, the properties-YAML name edit, and a `RenameFile` **last**.
The plugin must advertise the capabilities in §4.1 or the file rename is
silently dropped by the server-agnostic client code. Caveat to document in
`:h cogitator`: dependents are read from disk, so unsaved changes in *other*
buffers are not seen — save before renaming.

### 6.2 Diagnostics

- Pull: Neovim 0.11 drives `textDocument/diagnostic` automatically from
  `diagnosticProvider`. Nothing to implement; dangling-`ref` and YAML schema
  errors arrive this way.
- Push: with `fusion` enabled the server publishes compile diagnostics and
  suppresses the static ones. Both paths land in `vim.diagnostic`; no merge
  logic in the plugin.

### 6.3 Completion

- The server advertises `triggerCharacters = [".", "'", "\"", " ", "("]`
  so nvim-cmp / blink.cmp / native `vim.lsp.completion` fire in
  member-access (`alias.`), `ref('`, and post-keyword positions with no
  client configuration. The plugin keeps a client-side fallback that sets
  the same characters **only when the server advertised none** (older
  server builds).
- Ranking arrives via `sortText` tiers (`0` < `a` < `b` < `c` < `m` < `z`);
  completion engines must be left on `sortText` ordering (document that
  `cmp.config.compare.sort_text` has to stay in the comparator list).
- `completionItem/resolve` fills documentation — enable
  `resolveSupport` (default in 0.11).
- `insertText` is plain text (no snippet syntax). Do not advertise
  `snippetSupport`-dependent behavior.

### 6.4 Semantic tokens

Automatic in 0.11. Provide optional highlight links for the legend's
non-standard entries, e.g. `@lsp.type.macro.sql → @function.macro`, and a
`semantic_tokens = false` escape hatch
(`vim.lsp.semantic_tokens.enable(false, { bufnr })`).

### 6.5 Signature help

Nothing custom: triggers `(` and `,` come from the server; structured macro
parameters render through the default handler.

---

## 7. Health check (`:checkhealth cogitator`)

Report, in order:

1. `cogitator` binary found on `PATH` (or configured `cmd[1]`) + its
   `--version` output.
2. Neovim version ≥ 0.11 (else point at §10 fallback).
3. For the current buffer/cwd: whether `dbt_project.yml` was found and where.
4. Profiles resolution preview: list the candidate paths in server order
   (§1.3 of the README: override → `$DBT_PROFILES_DIR` → `<root>/profiles.yml`
   → `<root>/.dbt/profiles.yml` → `~/.dbt/profiles.yml`) and mark the first
   that exists. (Duplicated client-side knowingly; keep in sync with
   `util::profiles_candidates`.)
5. If `fusion` is configured: the fusion binary exists and `--version`
   mentions `dbt-fusion` (mirror of the server's own validation).
6. The tree-sitter-dbt grammar in Neovim: installed, loadable, at the
   server's revision.
7. With a live client: every `dbt.*` command and LSP method the plugin
   uses that the server does not advertise, with the feature it breaks
   (`health.COMMANDS`, `health.METHODS`), then the `dbt.doctor` report.

---

## 8. Testing

- Layout: `tests/run.lua` is a thin orchestrator over `tests/helpers.lua`
  and five ordered step files (`tests/steps/01_unit.lua` … `05_profiles.lua`;
  the state-mutating profiles-dir switch stays last). The Emacs suite
  mirrors it with `tests/helpers.el` under a single `run.el`.
- Determinism rules (binding, mirroring specs/rust-port/05 §7.1):
  - the suites run on a per-run COPY of `testdata/jaffle_shop_duckdb`
    inside a sandboxed `HOME`/`XDG_*` — the shared fixture is never
    opened in place, mutated, or `target/`-wiped;
  - readiness is the `dbt.status` refresh barrier
    (`refresh.count >= n`), never a lens/modelGraph proxy;
  - every wait is labeled and FAILS at its deadline (`H.wait` /
    `cogitator-test-wait`) instead of surfacing later as a stale
    assertion;
  - the quickfix step rides Neovim's built-in pull diagnostics
    (`vim.diagnostic` + `user_data.lsp`): Neovim ≥ 0.12 cancels
    concurrent manual `textDocument/diagnostic` pulls on buffer
    changes, and a cancelled `buf_request_sync` only times out;
  - the server binary resolves via `COGITATOR_BIN`, then
    `CARGO_TARGET_DIR`, with `.exe` on Windows.
- Coverage (superset of the original five): attach + barrier, hover,
  `:DbtSchema`, lenses (`… downstream`, `dbt run`), rename →
  `RenameFile`, commands, code actions + quickfix, formatting, both
  browsers, the DAG, profiles.
- VS Code: `editors/vscode/src/test/` is the extension-host suite on
  the same contract — `runTest.ts` (@vscode/test-electron ≥ 3.1;
  downloads a cached VS Code under `.vscode-test/`) copies the fixture,
  points `cogitator.serverPath` at the cargo-built binary via workspace
  settings, and `suite/index.ts` (no mocha) prints the same `ok`/`FAIL`
  steps: activation, client Running, the dbt.status barrier,
  hover/definition through VS Code's provider pipeline, the dbt.*
  command surface. `activate()` returns `{ client() }` so the suite
  (and other extensions) can reach the live LanguageClient.
- One code-executed entry point: `just system-test` runs
  `crates/cogitator/tests/system_editors.rs` (`#[ignore]`d otherwise),
  which drives all three suites with `COGITATOR_BIN` set to the exact
  cargo-built binary and parses their `ok`/`FAIL` lines; a missing
  editor SKIPs locally and FAILS in CI (`COGITATOR_REQUIRE_EDITORS=1`).
  Dev loops: `just nvim-test` / `just emacs-test` / `just vscode-test`.
- CI: the `editors` job in ci.yml (ubuntu, neovim stable + emacs-nox +
  xvfb for the extension host, path-filtered on `editors/**` and the
  Rust side) runs exactly that under `xvfb-run`.

---

## 9. Non-goals (v1)

- No `$/progress` UI (server does not send it); dbt run feedback stays
  showMessage-based.
- No multi-root workspaces (server reads `workspaceFolders[0]` only).
- No file watcher: the server refreshes on `didSave`/`didOpen`, and
  `workspace/didChangeWatchedFiles` is accepted but never registered for.
- No install/download of the server binary — health check points at the
  release assets (`cogitator-{linux-amd64,darwin-amd64,darwin-arm64,windows-amd64.exe}`)
  and `cargo build`.

## 10. Neovim 0.10 fallback

Ship a one-paragraph recipe in `doc/cogitator.txt` using `nvim-lspconfig`'s
custom-config API with the same `cmd`/`root_dir`/`init_options`/capabilities
tables; the plugin's own modules must not hard-require 0.11 APIs outside the
`vim.lsp.config` call (guard with `vim.fn.has("nvim-0.11")`).
