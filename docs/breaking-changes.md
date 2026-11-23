# Breaking-change preview

Before you run `dbt build`, cogitator tells you what your edit to a model
breaks: which columns the model no longer produces (or produces with
another type) compared with its **last built schema**, and who reads them.

## What it compares

- **Now**: the columns the model's SQL produces, from its typed column
  lineage — the same answer `dbt.columnLineage` gives. A `select *` the
  project's schemas cannot expand makes the columns unknown, and the
  preview says nothing rather than guess.
- **Then**: `target/catalog.json` (from `dbt docs generate`: the
  warehouse's real columns and types) when it has the model, else the
  columns `target/manifest.json` recorded from your properties YAML at the
  last parse. Without either, the preview is silent; `dbt.status` says so
  under `breaking_changes`.

Types are compared only when both sides know them. The current side's
types are the inferred output-column types (see the "Column types" section
of [properties-yaml.md](properties-yaml.md)) of the files as saved, and
only a proven type with an exact spelling in the project's dialect counts:
`count(*)` (an integer of unproven width) or a column typed only by an
unenforced YAML `data_type` is compared by name alone.

## What it reports

- **Removed** columns, and, when one removed and one added column have
  similar names (`customer_id` → `customer_key`), a *possibly renamed*
  hint. It is only a hint: the removal still counts.
- **Added** columns, for information — they break nothing.
- **Retyped** columns.

For every removed or retyped column: the tests on it, the downstream
columns whose lineage reads it (with their tests, whether their model has
an enforced contract and whether it is `access: public`), the model's
exposures, and whether the model itself is public (consumers outside the
project).

## Where you see it

- A diagnostic per removed or retyped column, code `breaking-change`:
  an **error** when a test or a contracted downstream model relies on a
  removed column, a **warning** when anything else reads it, otherwise
  information.
- A code lens, `N breaking changes`, that opens the full report.
- The command `dbt.breakingChanges` (`:DbtBreakingChanges` in Neovim,
  `M-x cogitator-breaking-changes` in Emacs, "dbt: Breaking changes
  against the last build" in VS Code). It uses the open buffer, saved or
  not.

The diagnostics and the lens are computed when you open or save a model,
never as you type: the file on disk is compared, and the downstream
models are read from disk too (at most 200, as for `dbt.impact`).
Clients that pull diagnostics see a save's findings on their next pull.
