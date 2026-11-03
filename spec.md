# Feature Brainstorm — candidate LSP features

Ideas for future server-side features, written against what the
codebase already has (building blocks named per idea, same style as
FEATURE_REQUESTS.md). Nothing here is committed work; when an idea is
picked up it should graduate into `specs/rust-port/` the usual way.

## A — dbt artifacts as a data source (highest leverage)

dbt leaves `target/manifest.json`, `run_results.json` and
`sources.json` behind after every invocation. The server never reads
them today — they unlock features no amount of static analysis can.

- **Last-run status inline**: parse `run_results.json` (stamp-cached
  like profiles.yml in `DerivedCache`) and decorate models — codelens
  "last run: OK in 3.2 s" / a WARNING diagnostic on the model whose
  last run or test failed, with the adapter error text as Guidance.
  Building block: `derived_cache::stamp`, the codelens batch.
- **Test failures as diagnostics**: failed tests from `run_results.json`
  mapped onto the model's YAML entry (positions already ride the
  saphyr side-tables) — the editor shows *which* test failed without
  leaving the buffer.
- **Per-model timing lens**: "N downstream · last run 12.4 s" — makes
  expensive models visible right where they are edited.
- **Source freshness surface**: `dbt.sourceFreshness` command running
  `dbt source freshness` (the `spawn_dbt` plumbing exists) + a lens on
  source YAML entries showing the last freshness verdict.
- **Manifest cross-check**: when `manifest.json` disagrees with the
  server's own scan (model present in one, missing in the other), a
  hint diagnostic — catches stale `target/` and parse drift both.

## B — editing intelligence (pure additions to existing passes)

- **Quickfix code actions from did-you-mean**: the dangling-ref/source/
  var diagnostics already carry `closest_matches` suggestions; ship
  them as `codeAction` quickfixes (needs `Diagnostic.data` + an
  edit-bearing action path — today's actions are command-only).
  Already sketched in FEATURE_REQUESTS P4; cheapest real win.
- **Convert literal table → ref()/source()**: detect `schema.table` /
  `database.schema.table` identifiers in FROM/JOIN that match a known
  model or declared source (the relation resolver in
  `handlers/profiles.rs` already computes those coordinates) and offer
  a code action rewriting to `{{ ref('…') }}` / `{{ source('…','…') }}`.
- **Scaffold missing tests**: code action on a model's YAML entry —
  insert `data_tests: [unique, not_null]` for columns that look like
  keys (`*_id`, `id`), reusing the extractModelYaml edit machinery.
- **Inherit upstream descriptions**: hover already resolves columns
  through lineage; add a code action that copies the upstream column's
  YAML `description:` onto an undocumented downstream column (and a
  hint diagnostic listing undocumented columns, off by default).
- **Inlay hints** (`textDocument/inlayHint`): the resolved relation
  after `ref()`/`source()` calls (`→ jaffle.main.customers`), macro
  default values at call sites (signatureHelp already parses params).
- **Document links** (`textDocument/documentLink`): make every
  `ref('x')`/`source('a','b')` a clickable link — definition exists,
  links let editors underline without a keypress.
- **Linked editing for CTE aliases**: rename-in-place for a CTE name
  and its uses inside one file (`linkedEditingRange`); the TokenIndex
  already knows same-literal occurrences (documentHighlight).

## C — project understanding

- **Workspace diagnostics** (`workspace/diagnostic`): project-wide
  dangling refs/sources without opening files — the graph knows them
  (`ProjectGraph::dangling_refs`); needs result streaming + the
  previousResultId dance.
- **Dead-model detection**: models with no downstream edges, no
  exposure and no selector referencing them → opt-in hint diagnostic
  or a `dbt.deadModels` report command. Building block: petgraph
  `dependents_of`.
- **Exposures and metrics as graph citizens**: parse `exposures:` and
  semantic-layer blocks (`metrics:`, `semantic_models:`) from the
  properties YAML (schemas crate already deserializes them), add them
  to the graph/DAG, give them hover + go-to + dangling-ref checks.
- **Impact analysis command** (`dbt.impact`): transitive downstream
  closure of a model *with column granularity* where lineage resolves
  — "customer_id feeds 9 models, 2 exposures". Building blocks:
  project graph + column lineage, composed.
- **Package awareness**: parse `packages.yml` + `dbt_packages/` so
  package macros get hover/definition/completion like local ones, and
  `ref('package', 'model')` two-arg form resolves.
- **Multi-root / workspace folders**: today only the first root wins;
  monorepos with several dbt projects want one server with per-root
  contexts (the context builder is already root-keyed).

## D — SQL depth (adapter/dialect work)

- **Warehouse-backed column completion**: for duckdb targets the file
  is local — `information_schema` queries (feature-gated, like the
  parse diagnostics) can complete columns of *sources* that have no
  YAML docs. Explicitly opt-in; never on the request path (offload).
- **Formatting** (`textDocument/formatting`): embed a SQL formatter
  behind a feature flag (candidates: sqruff's formatter, sqlformat-rs)
  with Jinja regions masked the way `cogitator-duckdb::mask_jinja`
  already does — format around the Jinja, never through it.
- **Style diagnostics (lint-lite)**: a handful of dialect-aware,
  zero-config checks on the existing token stream/AST: `SELECT *` in a
  non-staging model, ambiguous unaliased columns in multi-relation
  FROMs, trailing commas the dialect rejects. Each one a Guidance
  diagnostic with a fix.
- **Jinja render preview** (`dbt.expandJinja`): render the current
  model's Jinja with minijinja (macros + vars from the context, `ref`/
  `source` stubbed to relations) and show the SQL — a poor man's
  compile without dbt, for quick "what does this loop emit".
- **Incremental tree-sitter reparse**: `tree.edit()` on didChange
  instead of full reparse (06 §3.4 leftover) — latency insurance for
  very large models.

## E — operations / DX

- **dbt.runSelection**: run dbt with an arbitrary selector typed in
  the editor (`state:modified+`, `tag:nightly`) — the spawn/progress
  plumbing exists; add selector-syntax completion from known tags and
  selectors (workspace/symbol already indexes them).
- **Cancel running dbt jobs**: `dbt.cancel {token}` — fusion compiles
  are already cancellable per URI; extend the same AbortHandle pattern
  to run/test/compile invocations, surfaced as a progress-cancel.
- **Config schema completion for cogitator.yml**: the server knows its
  own config schema (`config_file.rs` key lists) — completion +
  hover + unknown-key diagnostics when the file is open in the editor,
  eating our own YAML-intelligence dogfood.
- **Health/self-check command** (`dbt.doctor`): one JSON report the
  editors render — binary paths, profiles resolution trace, dialect,
  artifact freshness, cache stats — merging what nvim health.lua and
  cogitator-doctor each half-compute client-side today.
- **Persisted cross-session cache**: serialize the scan cache's
  (mtime, size) index to `target/.cogitator-cache` so a cold start on
  a large project skips unchanged file parsing entirely.
