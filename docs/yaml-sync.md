# Syncing schema YAML with the SQL

`dbt.syncYaml` (and `cogitator yaml-sync` on the command line) brings the
properties YAML of every model of the root project in line with what the
model's SQL produces — what dbt-osmosis's `yaml refactor` does — on the
same pieces the single-entry code actions use: typed column lineage, the
inferred column types and the line-wise YAML editing of the quickfixes.

## What a sync does

For each model (or the ones you name):

1. **A model without an entry gets one**, where `dbt.extractModelYaml`
   would put it — `<model>.yml` beside the SQL — or, with the
   `newEntries: "folder"` option (`--per-folder`), in the folder's
   `_<folder>__models.yml`, shared by the folder's models. An existing
   file is appended to; a file with `models: []` written inline is left
   alone and said so.
2. **Produced columns the entry lacks are added**, in the model's output
   order, after the columns already written.
3. **A column without a description inherits one** — the description of
   the column it passes through unchanged, from the nearest documented
   ancestor: a model column or a source column. The walk follows column
   lineage model by model, through CTEs, derived tables and `select *`,
   and records where the text came from and which undocumented models it
   went through. A description someone wrote (anything non-empty) is never
   touched; `description: ""`, `''`, `~` and a missing key count as empty.
   With `inheritTypes` (`--inherit-types`), a missing `data_type:` is
   filled where the type inference proves one (the "Fill data_type"
   quickfix's rule).
4. **Documented columns the model no longer produces are reported** as
   stale, and removed only with `prune` (`--prune`).
5. With `reorder` (`--reorder`, off by default) the column items follow
   the SQL's order; each item moves whole, with the comment lines above it.

## Same column, or not

A description is only inherited by a column that *is* the upstream
column: a bare reference under the same name at every hop.

| SQL | inherits from `a.amount`? |
|---|---|
| `select amount from {{ ref('a') }}` | yes |
| `select a.amount from {{ ref('a') }} as a` | yes |
| `select * from {{ ref('a') }}` (a's columns known) | yes |
| `with s as (select * from {{ ref('a') }}) select amount from s` | yes |
| `select amount + 1 as amount from {{ ref('a') }}` | no — an expression |
| `select price as amount from …` | no — a rename |
| `with s(amount) as (select price from …) …` | no — a positional rename |
| `select * replace (amount * 2 as amount) …` | no |
| `… union all …` | no — a set operation mixes two columns |
| `select amount from a join b using (amount)` | no — two candidates |

A seed or a snapshot ends the walk: its own documented column is used,
nothing beyond it.

## When the sync stays out

It never guesses; what depends on something unclear is skipped, with the
reason in the summary:

- **a model whose lineage is unclear** — Jinja control flow without fresh
  compiled SQL (`dbt compile` fixes that), or SQL that does not parse: the
  whole model is skipped;
- **a `*` whose columns are not known** (its upstream has no closed
  schema, or a `{{ macro() }}` select item outputs columns nobody can
  list): the columns that are known are still added and documented, but
  stale columns are not reported, and nothing is pruned or reordered;
- **a column whose walk meets an unclear model** upstream: no description,
  and the reason names that model;
- **a versioned model** (`versions:` in its entry) and its `<model>_v<N>`
  files: columns live per version, which the sync does not edit;
- **a flow-style column** (`- {name: x}`), an empty description written as
  a block scalar, a `columns: []` written inline: line edits would break
  them.

## What stays as it was

The YAML is never re-serialized. Each phase (prune, fill, add, reorder)
edits lines of the file and reads the entry back before the next, so
comments, quoting, key order, blank lines and other entries stay byte for
byte. An inherited description is written as a double-quoted scalar; a
trailing comment on an emptied `description: ""` line is kept. The edits
an editor receives are line hunks of the file, not a rewrite of it, and
open buffers' unsaved text is what gets edited.

A second sync after applying the first changes nothing.

## Running it

| Where | How |
|---|---|
| Any LSP client | the code action "Sync schema YAML of 'm' (N changes)" on a model's SQL file and on its YAML entry |
| Neovim | `:DbtSyncYaml [model…] [%] [prune] [types] [reorder] [folder]` — the plan in a split, then applied on confirm (`%` is this buffer's model) |
| Emacs | `M-x cogitator-sync-yaml` (`C-u` for this buffer only; `cogitator-sync-yaml-prune`, `cogitator-sync-yaml-inherit-types`) |
| VS Code | `dbt: Sync schema YAML` — scope, options, a summary, the plan as a diff on request, then apply |
| CLI / CI | `cogitator yaml-sync [--project-dir DIR] [--check] [--prune] [--inherit-types] [--reorder] [--per-folder] [--json] [models…]` |

`cogitator yaml-sync --check` writes nothing, prints what would change and
exits 1 when anything would (0 when in sync, 2 when the project cannot be
read) — a CI gate. Without `--check` it writes the files. `--json` prints
the same plan `dbt.syncYaml` answers. The top-level `--profiles-dir` and
`--target` pick the dialect as they do for the server.

The wire contract is in [commands.md](commands.md#dbtsyncyaml).
