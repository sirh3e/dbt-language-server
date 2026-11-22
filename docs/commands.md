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

- **Argument**: `{uri, selector, kind?}` — any dbt selector; `kind` is `run` (default), `test`, `build` or `compile`
- **Result**: `{token}` or InvalidParams
- **Side effects**: runs dbt with an arbitrary validated selector (`{selector, kind?: run\|test\|build\|compile}`)
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
- **Result**: `{model, uri, description, tags, upstream, downstream, versions, tests, compiledUri}` or `null`
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
- **Result**: `{model, column, summary, models: [{name, uri, depth}], exposures, columnHits, truncated}`
- **Side effects**: none (column hits are an under-approximation: `*` and unresolved lineage stay out; lineage fan-out capped at 200 models)
- **Editors**: Neovim ✓ · Emacs — · VS Code —

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

## Analysis

### `dbt.queryPlan`

- **Argument**: `{uri}` — the document the command is about
- **Result**: `{model, dialect, backend, schemaRevision, status: complete\|partial\|declined, guidance, origin, levels: {hir, mir, lir: {available, fidelity, text, unavailable, degradations, violations}}, findings, trace: {entries, incomplete}, needs: {available, missing}}`
- **Side effects**: none (read-only): runs the IR pipeline on the open buffer or file; a level dropped by a verifier violation is shown as unavailable; models with control flow other than a lone `is_incremental()` come back `declined` with a Guidance pointing at `dbt compile` until compiled fallback lands — :DbtPlan
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

- **Argument**: `{node: {kind, name}}` — `kind` one of `model`, `macro`, `test`, `generic_test`, `source`, `exposure`, `metric`, `semantic_model`, `analysis`
- **Result**: the card data of the node, by kind:
  - `model`: `dbt.modelInfo`'s answer with `kind`;
  - `macro`: `{package, description, params, uri, range, callers, calls, usedByTests}`;
  - `generic_test` (a `{% test %}` definition): `{params, uri, range, appliedTo: [{model, column, uri, range}]}`;
  - any other kind: `{uri, refs, calls, upstream, downstream, compiledUri, description}`.

  Unknown nodes answer `null`.
- **Side effects**: a WARNING with the closest names for an unknown node
- **Editors**: Neovim ✓ · Emacs ✓ · VS Code ✓

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
- **Result**: server/project/profiles/refresh/caches/config snapshot
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
