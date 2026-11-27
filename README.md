# cogitator — a dbt language server in Rust

`cogitator` is a language server for [dbt](https://www.getdbt.com/)
projects: hover, completion, navigation, lineage, refactoring and dbt
execution for your models, sources, macros and schema YAML.

Guides live in [`docs/`](docs/README.md): [Neovim](docs/neovim.md),
[incrementality](docs/incrementality.md), [ast-grep](docs/ast-grep.md), the
[command reference](docs/commands.md) and, for contributors,
[editor parity](docs/editor-parity.md).

## Origin

This project is a **complete Rust rewrite** of
[j-clemons/dbt-language-server](https://github.com/j-clemons/dbt-language-server),
the original Go implementation — all credit for the idea, the initial
feature set and the wire behavior goes to that project. The rewrite was
done for speed (parsing, project scans and completion answer in
milliseconds on large projects) and as a base for features the Go server
did not have.

- The Go behavior was captured first as binding specs in
  [`specs/rust-port/`](specs/rust-port/) (00–09) and the port was written
  against them; wire shapes are Go-exact, extended only additively. The
  one deliberate deviation: the `shutdown` response carries `"result": null`
  as JSON-RPC requires (Neovim rejects the bare Go shape).
- The last Go state lives in git history at commit `724b0f2`.
- The binary is named `cogitator` (Adeptus Mechanicus naming); the library
  crates use the brand plus their technical role:
  `cogitator-{error,lsp,dialects,schemas,parser,analysis,duckdb,treesitter}`.

## Features inherited from the Go server

| Resource | Go to Definition | Hover | Completion |
| --- | --- | --- | --- |
| Model References | x | x | x |
| Sources | x | x | x |
| Seeds | x | x | x |
| Macros | x | x | x |
| Variables | x | x | x |
| Functions |   | x | x |

Plus: go to schema, bundled function documentation (Snowflake and
BigQuery), dbt Fusion static analysis, document symbols, and the
`DBT_PROFILES_DIR` dialect lookup.

## Features added in this project

**Language intelligence**

- Grammar-aware **completion engine**: classifies the cursor's grammatical
  position (member access `alias.`, relation position after FROM/JOIN,
  column position after SELECT/WHERE/ON, keyword continuations like
  `group ` → `by`) and ranks suggestions accordingly
- **Write assist**: local CTE/alias scope index, model and source columns
  from schema YAML as completion with documentation
- **Column lineage** (`dbt.columnLineage` / `dbt.showLineage`):
  CTE-transitive with `select *` substitution, down to staging columns
- **Column hover and go-to-definition**: YAML description plus lineage on
  hover, jump from a column to its schema-YAML entry
- Structured **signature help** for macros (real parameter objects with
  defaults, active-parameter tracking)
- **Dispatched macros**: go to implementation lists every
  `<adapter>__x` / `default__x` in dbt's dispatch search order, and hover
  names the one the active adapter picks; **macro call checks** warn on
  wrong argument counts, unknown or duplicate keywords and dispatches
  with no default (on by default; [docs/macros.md](docs/macros.md))
- **Semantic tokens**, folding ranges, selection ranges, document
  highlights, workspace symbols, find references
- **Diagnostics**: dangling `ref()` detection, dbt YAML schema validation
  (typify-generated types from [dbt-jsonschema]), pull diagnostics
  (`textDocument/diagnostic`)
- **CTEs without the warehouse**: the query that selects a CTE
  (`dbt.cteSql`, an "SQL" lens beside "Preview rows"), and on hover a CTE's
  columns with the lineage of each; in Neovim the rows in a float
  (`:DbtPreviewCte!`) and every CTE in a picker (`:DbtCtes`)
- **The SQL of a model's tests** (`dbt.testSql`): every YAML data test
  (dbt's own, the project's and packages' generic tests, with their
  arguments and `where:`), singular test and unit test of the model,
  rendered as the query dbt executes — no `dbt compile` first
- **Incremental strategies** (`dbt.incrementalSql`): what an incremental
  run executes for `append`, `merge`, `delete+insert`, `insert_overwrite`
  and `microbatch` (the config's, the adapter's default, or one picked),
  and code actions that make a model incremental with a strategy's
  template or switch its strategy
- **Invalid models as you type**: a misspelt materialization, config key
  or value, a model that refs itself or closes a ref cycle, `is_incremental()`
  in a model that is not incremental, an empty model — each with its
  did-you-mean as a quickfix ([docs/lint-and-format.md](docs/lint-and-format.md))
- **Syntax errors as you type**, no `dbt compile` first: Jinja errors
  (unclosed `{% if %}`, stray `{% endif %}`, unbalanced `{{ }}`) and SQL
  errors in the masked SQL — and incremental models, `{% for %}` loops and
  macros checked as dbt renders them (`is_incremental()` both ways),
  declining only what needs a warehouse
  ([docs/lint-and-format.md](docs/lint-and-format.md))
- **`cogitator lint`** and **`cogitator format`** for CI: every
  editor-free diagnostic over the whole project (text, JSON or GitHub
  annotations, exit 1 on errors), and the formatter in place or as
  `--check` ([docs/lint-and-format.md](docs/lint-and-format.md))
- dbt **YAML intelligence**: property-name completion and schema errors in
  `schema.yml` / `dbt_project.yml`

**Refactoring and navigation**

- **Model rename** (`textDocument/rename`): rewrites every dependent's
  `ref()`, updates the schema-YAML entry and renames the model file — one
  WorkspaceEdit over the project graph
- **File operations** (`workspace.fileOperations`): renaming a model file
  in the editor's tree rewrites its refs and YAML entry, deleting one
  warns about its dependents, and created files join the context
  ([docs/file-operations.md](docs/file-operations.md))
- **Declaration** (`textDocument/declaration`): a model's properties-YAML
  entry, beside definition's SQL; a YAML column's definition is the
  select item producing it
- **Code actions** (`textDocument/codeAction`): the executable lens set
  at the cursor as command-only actions, so every stock LSP editor —
  eglot, lsp-mode, Zed, plain Neovim — gets dbt workflows through its
  native code-action UI with zero client wiring; on a `ref('x')` an
  "Open schema of 'x'" action, and in schema YAML "Open model" / "Go to
  tests" for the entry under the cursor
- **Quickfixes** that define or repair what a diagnostic names:
  did-you-mean rewrites; "Create model 'x'" for a `ref()` to no model
  (a new `x.sql` beside the referencing one); "Define var 'x' in
  dbt_project.yml" for an unknown `var()`; "Add N missing column(s)" to
  the schema YAML for produced but undocumented columns; "Remove the
  trailing comma" and "Qualify with 'alias'" for the lint findings —
  preferred, and bundled into `source.fixAll`, where they change no result;
  "Declare source", "Add table to source" and "Create docs block" for
  undeclared `source()`/`doc()` names; for an enforced contract that
  drifted, add the undeclared columns (with `data_type`) or drop the
  contracted ones the model never produces; for a ref into another
  group's private model, "Make 'x' protected" or "Move into group";
  "Run dbt deps" in a packages.yml naming uninstalled packages
- **Incrementality** (DBSP, specs/ir-pipeline/10): a `table` model that
  could be built incrementally gets a hint naming the dbt strategy
  (`append`, `delete+insert` on a time bucket or group keys, `merge` for a
  DISTINCT) and a fix rewriting it into an incremental model;
  `dbt.queryPlan` says why the others must stay tables, and
  `dbt.incrementality` lists every table model, slowest first. A model kept
  a table by one blocker (a global `rank()`) over an aggregating CTE can be
  **split**: the CTE becomes its own incremental model and the rest a table
  over it (code action and `dbt.splitPlan`); the analysis, the frontier and
  the split are also emitted as S-expressions. In the editors: `:DbtSplit`
  and `:DbtIncrementality` (Neovim), `M-x cogitator-split-plan` and
  `cogitator-incrementality` (Emacs), the `dbt: Split model…` and `dbt:
  Table models that could be incremental` palette commands (VS Code); Zed
  gets the code action. An incremental model gets a "Compare incremental /
  full refresh" lens: `dbt.incrementalDiff` renders it with
  `is_incremental()` false and true and opens the unified diff
  (`:DbtIncrementalDiff`, `M-x cogitator-incremental-diff`, `dbt: Compare
  incremental / full refresh`). With catalog.json statistics, each CTE shows
  the row count they prove as an inlay hint (`1,200 rows`, `≤ 1,200 rows`;
  `perf.row_hints`); `perf.inline_findings: true` shows the plan findings
  inline too (`⚠ join.cartesian`, the whole finding as the tooltip)
- **Refactors**: "Expand `c.*`" to the relation's known columns,
  "Extract CTE to a model" (the CTE becomes its own model and is
  `ref()`'d), and "Extract schema YAML" as an edit for editors that run
  no custom commands
- **Code lenses** per model: `N downstream` / `N upstream`, go to schema,
  column lineage, dbt run/test/compile, `N tests` (documented) or
  `Extract schema YAML` (undocumented); per macro: `N call sites`
- **Extract model → schema YAML** (`dbt.extractModelYaml`): scaffolds
  `<model>.yml` with the model's real columns (from lineage) when no entry
  exists, or adds the next `versions:` entry (and bumps `latest_version`)
  when one does — applied in the editor via workspace/applyEdit
- **Sync schema YAML** (`dbt.syncYaml`, `cogitator yaml-sync`): the
  project-wide version of the YAML quickfixes, what dbt-osmosis's `yaml
  refactor` does — models without an entry get one, missing columns are
  added in output order, undocumented columns inherit the description of
  the column they pass through unchanged (through CTEs and `select *`, from
  the nearest documented model or source column; never across a rename or
  an expression), stale columns are reported (`--prune` removes them),
  optionally proven `data_type`s are filled and columns reordered. Edits
  keep the YAML's formatting and comments; models whose lineage is unclear
  are skipped with the reason. `cogitator yaml-sync --check` exits 1 when
  anything would change, for CI ([docs/yaml-sync.md](docs/yaml-sync.md))
- **Go to tests** (`dbt.modelTests` / `dbt.goToTests`): jump to every
  `data_tests`/`tests` entry defined for a model — model-level, per
  column, and per version (a `<model>_v2.sql` buffer resolves to its base)
- **Model-graph browser** (`dbt.modelGraph`): direct upstream/downstream
  neighbors for the yazi-style three-pane UI in the Neovim and Emacs
  integrations
- **Tag and selector search**: model tags from YAML and
  `{{ config(tags=…) }}` plus `selectors.yml` entries are indexed;
  `workspace/symbol` understands `tag:x` and `selector:x` queries in any
  editor, and Neovim adds `:DbtTags` / `:DbtSelectors` pickers
- **Node selection** ([docs/selectors.md](docs/selectors.md)): dbt's
  selector language evaluated over the project — `dbt.select` lists what
  `tag:nightly +orders --exclude resource_type:test` or `selector:nightly`
  picks, and
  selectors.yml gets completion, diagnostics, a hover and "N nodes match"
  lenses
- **Project DAG view in the terminal — Neovim only**: `:DbtDag` renders
  the whole project dbt-docs style (layered left→right, box-drawn nodes
  with tags, edges), and docglow-style column information inside the nodes
  (`c`/`C` expand, `L` column lineage, `<CR>` opens the model). This
  text-mode UI ships only in the Neovim plugin.
- **Lineage site** ([docs/lineage-site.md](docs/lineage-site.md)): the
  project DAG with column-level lineage as one self-contained HTML page —
  `cogitator lineage-site [--project-dir DIR] [--out DIR] [--open]`, from
  static analysis alone (no dbt run, no warehouse, nothing fetched). Click
  a column to see every column upstream and downstream of it across
  models, with how each is made (passed through, renamed, derived); models
  cogitator cannot trace are marked and say why. The same page is a live
  panel in VS Code (`dbt: Show lineage view`, following the active model),
  and `:DbtLineageSite` / `M-x cogitator-lineage-site` write and open it
  from Neovim and Emacs (`dbt.lineageSite`).
- **Fuzzy pickers — Neovim only**: `:DbtFind` finds models, sources,
  macros and selectors (jump to the definition), documented columns,
  a model's upstream/downstream with depth, the refs of the current
  file, tests and tags, with a preview — shown in fzf-lua, snacks.picker
  or telescope when installed, else a built-in fzf-style picker (fzf's
  query syntax). Every other choice the plugin offers uses it too.
- **Model explorer — Neovim only**: `:DbtBrowse` shows one model at a
  time: its upstream models on the left, the model's card in the
  middle (description, tags, relation, versions, documented columns,
  tests with jumps), its downstream models on the right, and below
  them its dbt SQL beside the compiled SQL (served by
  `dbt.compiledSql`). Walk the lineage from the side panes (with a
  history and a breadcrumb), pick any model with the fuzzy picker, mark
  models to yank or `dbt run` together. Backed by the editor-neutral
  `dbt.modelInfo` command (neighbors, versions, tests and the
  compiled-SQL location in one round trip); the UI ships only in the
  Neovim plugin.

**dbt workflow**

- **dbt execution** from the editor (`dbt.run` / `dbt.test` /
  `dbt.compile` with `--select` on the current model, plus project-wide
  `dbt.parse`, `:DbtParse`); a successful compile opens the compiled
  SQL, a successful parse refreshes the server's project context
- **Compiled SQL over the wire** (`dbt.compiledSql`): the server reads
  `target/compiled/…` itself and answers the text (2 MiB cap), so
  editors need no filesystem path math
- **Query plan** (`dbt.queryPlan`, `:DbtPlan`): each IR level (HIR,
  MIR, LIR) of one model with its fidelity and any gaps, the optimiser
  findings, and which rules ran or were skipped and why; models with
  Jinja control flow other than `is_incremental()` report what to run
  instead (`dbt compile`)
- **Relation resolution** (`dbt.relation`): the
  `database.schema.identifier` of the `source()` table, `ref()` target
  or model under the cursor — source YAML coordinates with dbt's
  defaults, plus database/schema from the active profiles target — the
  building block for query-this-relation editor workflows
  (`:DbtRelation` yanks it in Neovim, `M-x cogitator-relation` copies
  it in Emacs)
- **dbt profiles from anywhere**: `--profiles-dir`,
  `initializationOptions.profilesDir`, `$DBT_PROFILES_DIR`,
  `<root>/profiles.yml`, `<root>/.dbt/profiles.yml`, `~/.dbt/profiles.yml`
  — first file whose profile resolves a target wins (details below)
- **Session configuration manager** (Neovim): a persisted JSON store of
  every dbt project ever opened with per-project profiles-dir /
  profile / target selections — injected at attach, switched live via
  `dbt.setProfilesDir` (`:DbtProfiles <dir>`, `M-x
  cogitator-set-profiles-dir`) with no server restart, browsed with
  `:DbtProjects`
- **Connection auth manager**: Snowflake auth classification (password,
  key pair, SSO, OAuth, MFA, JWT) with per-method validation, a redacted
  `dbt.profiles` inventory behind a profile allowlist, runtime
  profile/target switching (`dbt.setTarget`, `:DbtTarget`), and auth
  diagnostics inside profiles.yml
- **Actionable errors**: every failure surfaces as an editor message with
  a Reason and a Solution (missing project, unresolved dialect, missing
  dbt binary, failed runs with the exact retry command)
- **Status visibility**: context refreshes and dbt runs report through
  standard `$/progress` (visible in `vim.lsp.status()`, fidget, eglot's
  mode line), and `dbt.status` (`:DbtStatus`, `M-x cogitator-status`)
  answers with the full picture — backend, fusion state, project counts,
  dialect, active profile/target, refresh timings and cache stats

**Engine and infrastructure**

- Parsing by **minijinja** (Jinja layer) + **sqlparser** (SQL layer) — no
  hand-written lexer; zero-copy tokens over the document text
- **tree-sitter** backend ([`tree-sitter-dbt`](https://github.com/sirh3e/tree-sitter-dbt)) for
  error-tolerant segmentation of broken buffers (`--no-tree-sitter` falls
  back to the plain pipeline)
- Optional **DuckDB 2.0 parser** diagnostics (`--features duckdb-parser`,
  C/C++ built with the Zig toolchain)
- **Project scan cache** keyed by mtime/size: unchanged files are never
  re-parsed across refreshes; petgraph project graph
- `rootUri`/`workspaceFolders` support (modern clients omit the deprecated
  `rootPath`)
- **Windows support**: drive-letter `file:///c%3A/…` URIs,
  `%USERPROFILE%` lookup, a `windows-amd64.exe` release artifact
- **Observability**: `tracing` with optional OTLP export (`--otlp`) and a
  tokio-console feature
- Tokio task architecture (reader → dispatcher → writer, debounced
  background context refresh), cross-platform releases via cargo-zigbuild,
  CI tests on Linux/macOS/Windows

**Editor integrations (first-party, in this repository)**

- **Neovim** plugin ([`editors/nvim`](editors/nvim)): `:Dbt*` commands,
  model-graph browser, code-lens wiring, `:DbtProfiles`,
  `:checkhealth cogitator`, headless test suite
- **Emacs** package ([`editors/emacs`](editors/emacs)): eglot + lsp-mode,
  the same commands and graph browser
- **Zed** extension ([`editors/zed`](editors/zed)): server registration
  with settings passthrough, and the dbt menu as the server's dialogs

[dbt-jsonschema]: https://github.com/dbt-labs/dbt-jsonschema

## What reaches which editor

| Capability | Neovim | Emacs | Zed | any stock LSP editor |
|---|---|---|---|---|
| Diagnostics, completion, hover, definitions/references, symbols, folding, selection ranges, semantic tokens, signature help, rename | ✓ | ✓ | ✓ | ✓ |
| Code actions: the dbt command set at the cursor, YAML-entry jumps, profiles.yml target switching | ✓ | ✓ | ✓ | ✓ |
| Code lenses | ✓ | ✓ | ✓ | editor-dependent |
| The dbt menu: every dbt command by group | ✓ `:Dbt` | ✓ commands | ✓ the "dbt…" code action (server dialogs) | — |
| dbt run / test / compile / parse, schema & tests navigation, extract-to-YAML | ✓ commands + keymaps | ✓ commands | ✓ the dbt menu | via code actions |
| Pickers: targets, tags, selectors, CTEs, strategies, columns, jobs | ✓ | ✓ | ✓ prompts of the dbt menu | — |
| Query workflow (bounded SELECT on the relation) | ✓ `:DbtQuery` (dadbod/scratch) | ✓ `cogitator-query` (sql buffer) | rows of a model or CTE as a document | — |
| Model-graph browser | ✓ `:DbtGraph` | ✓ `cogitator-graph` | the model graph as a document | — |
| Project explorer / text-mode DAG | ✓ `:DbtBrowse` / `:DbtDag` (deliberately Neovim-only) | — | — | — |
| Lineage site (DAG + column lineage, HTML) | ✓ `:DbtLineageSite` (opens in the browser) | ✓ `cogitator-lineage-site` (opens in the browser) | ✓ the dbt menu (opens in the browser) | `cogitator lineage-site` from a shell; VS Code: a live panel |
| Fuzzy pickers | ✓ `:DbtFind` (fzf-lua / snacks / telescope / built-in) | — | — | — |
| Profiles-dir live switch | ✓ persisted per project | ✓ command (dir-locals for persistence) | `initialization_options.profilesDir` | — |
| Health report | ✓ `:checkhealth cogitator` | ✓ `cogitator-doctor` | ✓ the dbt menu | — |

Zed's extension API cannot add commands, pickers or panels, so the server
draws the dialogs itself: the extension asks for them
(`initializationOptions.dialogs`), the code action "dbt…" opens the dbt
menu as `window/showMessageRequest` prompts, and each command's answer
opens as a document (`dbt.menu`, [docs/commands.md](docs/commands.md)).
Any client that answers showMessageRequest can ask for the same.

## Dialects

Function hover/completion documentation is bundled for **Snowflake** and
**BigQuery**. The dialect itself (from `profiles.yml`) also steers the SQL
parser and keyword set — DuckDB, Postgres and the other adapter types are
recognized, with generic function docs.

## dbt Fusion static analysis

With dbt Fusion installed, compilation results come back as diagnostics in
the editor; artifacts are written outside your project.

```
-f, --fusion=[path]
```

Without a path, `dbt` from `$PATH` is used. The binary is validated at
startup (its `--version` must report `dbt-fusion`), with a clear message
if not.

## dbt profiles

The SQL dialect comes from your dbt `profiles.yml`. It can live anywhere;
the first location that contains the project's profile with a resolvable
target wins:

1. `--profiles-dir <dir>` (CLI, `~` is expanded) or
   `initializationOptions.profilesDir` sent by your editor at initialize
   (the client option replaces the CLI flag)
2. `$DBT_PROFILES_DIR`
3. `<workspace root>/profiles.yml`
4. `<workspace root>/.dbt/profiles.yml`
5. `~/.dbt/profiles.yml` (`%USERPROFILE%\.dbt\profiles.yml` on Windows)

If no candidate resolves, the server sends a warning listing every path it
searched. In the Neovim plugin, `profiles_dir` in `setup()` or
`:DbtProfiles <dir>` at runtime configures this; in Emacs it is
`cogitator-profiles-dir`; in Zed it is
`lsp.cogitator.initialization_options.profilesDir`.

### Connection auth manager (Snowflake first-class)

Snowflake alone has half a dozen ways to connect — password, key pair,
SSO (`authenticator: externalbrowser`), OAuth, MFA, JWT. The server parses
profiles.yml into a **redacted inventory** (auth methods and what is
missing, never a credential value) and validates each target:

- unresolved `{{ env_var('…') }}` references (no value, no default)
- password auth without `user`/`password`, key-pair auth whose
  `private_key_path` does not exist, OAuth without `token`, missing
  `account`, unknown `authenticator` values

Problems surface three ways: a warning when the *active* target is broken,
diagnostics inside profiles.yml when you open it, and the
`dbt.profiles` report.

**Read scope**: by default only the profile named in `dbt_project.yml` is
parsed and exposed; widen it with `--profiles-allow name1,name2` or
`initializationOptions.profilesAllow`.

**Active target**: the profile's default `target:` unless overridden by
`--target` / `initializationOptions.target` — or switched at runtime with
the `dbt.setTarget` command, which re-resolves the SQL dialect and passes
`--target` to dbt run/test/compile. In Neovim: `:DbtTarget` (a picker
showing each target's auth method and issues); in Emacs:
`M-x cogitator-target`.

## Configuration file

`cogitator.yml` (or `.cogitator.yml`) at the workspace root is read at
initialize and **hot-reloaded** while the server runs — a stamp poll
every 2 s, a fast-path when you save the file through the editor, and
an explicit `dbt.reloadConfig` command. Every key is optional:

```yaml
profiles:
  dir: ~/.dbt            # ↔ --profiles-dir
  profile: jaffle_shop
  target: dev            # ↔ --target
  allow: [other_profile] # ↔ --profiles-allow
limits:
  compiled_sql_mib: 2    # dbt.compiledSql wire cap
refresh:
  debounce_ms: 200       # context-refresh coalescing window
startup:                 # honored only via --config at launch
  debug: false
  otlp: ""
  tree_sitter: true
  fusion: dbt
perf:
  profile: off             # off | recommended | strict: which plan rules
  catalog_stats: true      # report as diagnostics (dbt.queryPlan always works)
  row_hints: true          # CTE row counts proven by catalog stats, as inlay hints
  inline_findings: false   # the reported findings as inlay hints too
perf_rules:                # per-rule severity or off; wins over the profile
  join.cartesian: warning  # off | hint | info | warning
  filter.non_sargable: off
lint:                      # style checks default off; macro_calls,
  select_star: false       # dispatch (docs/macros.md) and syntax
  macro_calls: true        # (docs/lint-and-format.md) default on
  dispatch: true
  syntax: true
  model: true              # invalid models (docs/lint-and-format.md), default on
inlay:
  vars: true               # the value after each var() (docs/variables.md)
  env_vars: true           # the value after each env_var(), secrets masked
  jinja: false             # what short {{ … }} expressions render to
preview:                   # dbt.previewCte, the "Preview rows" lenses (docs/preview.md)
  rows: 20                 # rows when the call names no limit
  max_rows: 1000           # larger limits are capped
vars:                      # overrides, like dbt's --vars; passed to dbt runs
  since: '2025-06-01'
```

Hot semantics are **changed-keys-only, last writer wins**: a reload
applies only keys whose file value changed, so editing `target:`
updates the live target even after a `dbt.setTarget`, while untouched
keys never clobber runtime switches. Keys removed from the file (or
the whole file, when deleted) revert to their launch values. Each
applied change shows one INFO message naming the keys; a file that
stops parsing shows the syntax error and keeps the previous values.
Unknown keys warn with a did-you-mean, never fail.

Launch precedence: defaults < config file < CLI flags <
`initializationOptions`. The `startup:` keys (install-once subsystems)
only apply when the file is passed at launch via `--config <path>`;
found in the root file, they warn to restart instead. `dbt.status`
reports the file's path, whether it loaded, and the last parse error.

## Performance advice

Off by default. Set `perf.profile` in `cogitator.yml` to `recommended` (the
rules whose findings are claims) or `strict` (everything), or name single
rules under `perf_rules`; `dbt.queryPlan` shows the plan whichever you pick.

The advice comes from reading your SQL statically: nothing here runs a query
or connects to a warehouse, and SQL changes only through a quick fix you
accept. Findings report through pull diagnostics, and the next save
republishes them for push clients (the push path reuses a computed plan and
never plans). A model with Jinja control flow is planned from
`target/compiled` when that exists.

Suppress a finding with a `{# cogitator: ignore join.cartesian -- reason #}`
comment, or `-- cogitator: ignore …`, on the same line or the line above;
`ignore-file` covers the whole file. Suppressed findings stay visible in
`dbt.queryPlan`.

## Observability

Off by default. `--otlp[=endpoint]` (or `OTEL_EXPORTER_OTLP_ENDPOINT`)
exports **traces** (one span per LSP message with method/id, context
refreshes with per-builder children, dbt invocations with exit codes,
fusion compiles with diagnostic counts) and **metrics**
(`lsp.requests`, `lsp.request.duration` ms histogram by method — the
spec's latency-budget measurement — `context.refreshes` +
`context.refresh.duration`, `scan.cache` and `derived.cache` hit/miss
counters, `dbt.invocations` by kind and outcome) over gRPC/tonic as
`service.name = dbt-language-server`. `--debug` appends a plain-text
log to `log.txt` beside the executable (falls back to a warning when
that directory is read-only). Nothing but LSP frames ever touches
stdout. **Privacy note:** spans and logs carry file paths and model
names — point `--otlp` only at collectors you trust with them.

## Installation

On **Linux and macOS**, [install.sh](install.sh) downloads the latest
release into `~/.local/bin`:

```bash
curl -fsSL https://raw.githubusercontent.com/sirh3e/dbt-language-server/main/install.sh | sh
```

On **Windows**, [install.ps1](install.ps1) does the same into
`%LOCALAPPDATA%\Programs\cogitator` and adds it to your user `PATH`:

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://raw.githubusercontent.com/sirh3e/dbt-language-server/main/install.ps1 | iex"
```

Both scripts point at **this** repository (the upstream Go project's
installer no longer applies) and honor `COGITATOR_REPO`,
`COGITATOR_VERSION` and `COGITATOR_BIN_DIR` environment variables.
Alternatively grab a binary from the
[releases page](https://github.com/sirh3e/dbt-language-server/releases)
or build from source (below).

Release assets cover Linux (`cogitator-linux-amd64.tar.gz`), macOS
(`cogitator-darwin-amd64.tar.gz`, `cogitator-darwin-arm64.tar.gz`) and
Windows (`cogitator-windows-amd64.zip`) — each archive holds the
binary, LICENSE and README — plus a `SHA256SUMS` file to verify them
against and the packaged VS Code extension
(`cogitator-<version>.vsix`, installable via *Extensions → … →
Install from VSIX*). On Windows, put `cogitator.exe` somewhere on
`%PATH%` and configure your editor the same way as below.

Every editor pairs the same way: install the server (above), then
the editor's own plugin — each integration has a step-by-step
install guide in its directory.

### VS Code

First-party extension at [`editors/vscode`](editors/vscode)
([install guide](editors/vscode/README.md)): download
`cogitator-<version>.vsix` from the releases page and install it via
*Extensions → ⋯ → Install from VSIX* (not on the marketplace yet).
Palette commands for dbt run/test/compile/preview, lineage and server
status; `cogitator.serverPath` points at the binary when it is not on
PATH.

### Neovim

First-party plugin at [`editors/nvim`](editors/nvim)
([install guide](editors/nvim/README.md), spec:
`specs/neovim-plugin/00-plugin.md`): `:DbtRun/Test/Compile/Schema/
Lineage/Graph/Profiles`, the yazi-style model-graph browser, code-lens
wiring, `<leader>d` keymaps and `:checkhealth cogitator`. With lazy.nvim
it is one line (the repository's [`lazy.lua`](lazy.lua) does the rest):

```lua
{ "sirh3e/dbt-language-server", opts = {} }
```

A fuller spec — lazy-loading, and an opt-in `build` that installs the
server from the latest release — is
[editors/nvim/examples/lazy.lua](editors/nvim/examples/lazy.lua).

Or configure any LSP client manually (binary on `$PATH`):

```lua
vim.lsp.config("cogitator", {
  cmd = { "cogitator" },
  filetypes = { "sql", "yaml" },
  root_markers = { "dbt_project.yml" },
})
vim.lsp.enable("cogitator")
```

### Emacs

First-party integration at [`editors/emacs`](editors/emacs)
([install guide](editors/emacs/README.md)) for **eglot** (built-in,
Emacs 29+) and **lsp-mode**:
`M-x cogitator-run/test/compile/schema/lineage` and the same model-graph
browser (`M-x cogitator-graph`):

```elisp
(add-to-list 'load-path "/path/to/dbt-language-server/editors/emacs")
(require 'cogitator)
(require 'cogitator-graph)
(cogitator-setup-eglot)
(add-hook 'sql-mode-hook #'eglot-ensure)
```

### Zed

First-party extension at [`editors/zed`](editors/zed)
([install guide](editors/zed/README.md)): install it via
`zed: install dev extension`, configure through the standard
`lsp.cogitator` settings (binary path, arguments,
`initialization_options`). The third-party
[zed-dbt](https://github.com/kylejbrk/zed-dbt) extension also works.

### Helix

Add the executable to `$PATH` and to `languages.toml`:

```toml
[language-server.cogitator]
command = "cogitator"

[[language]]
name = "dbt"
scope = "dbt_project.yml"
file-types = ["sql","yml","yaml"]
language-servers = ["cogitator"]
```

## Building and developing

The server is a Cargo workspace:

```
cargo build --release -p cogitator
```

Development goes through the [justfile](justfile): `just check` (format,
clippy with `-D warnings`, all test suites, schema freshness),
`just nvim-test` and `just emacs-test` (headless editor integration tests
against `testdata/jaffle_shop_duckdb`), `just zed-build`,
`just duckdb-check`, `just gen-schemas`. CI runs the Rust suite on Linux,
Windows and macOS; releases cross-compile with cargo-zigbuild.

The whole toolchain is declared in Nix, derived from one source
(`nix/tooling.nix` — add tools there, nowhere else). Three ways in:

```bash
nix-shell          # classic, via shell.nix
```

```bash
devenv shell       # devenv.nix (devenv.sh); `check` runs the CI set
```

```bash
direnv allow       # .envrc picks devenv when installed, else nix-shell
```

Git hooks run through [prek](https://prek.j178.dev)
(`prek install`, config in `.pre-commit-config.yaml`); the changelog is
generated from the conventional-commit history with
[git-cliff](https://git-cliff.org) (`just changelog`, config in
`cliff.toml`).

The feature workflow (trunk-based, conventional commits, squash-merged
PRs) and the release cycle (`just release-prep <version>`, tags start at
v0.5.0) are defined in [CONTRIBUTING.md](CONTRIBUTING.md).

See `AGENTS.md` for the development command reference and
[`specs/rust-port/`](specs/rust-port/) for the implementation specs the
port was written against.
