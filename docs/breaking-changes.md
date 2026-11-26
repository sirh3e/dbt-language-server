# Breaking-change preview

Before you run `dbt build`, cogitator tells you what your edit to a model
breaks: which columns the model no longer produces (or produces with
another type) compared with its **last built schema**, and who reads them.

## What it compares

- **Now**: the columns the model's SQL produces, from its typed column
  lineage — the same answer `dbt.columnLineage` gives. A `select *` the
  project's schemas cannot expand makes the columns unknown, and the
  preview says nothing rather than guess.
- **Then**, twice:
  - **the last build**: `target/catalog.json` (from `dbt docs generate`: the
  warehouse's real columns and types) when it has the model, else the
    columns `target/manifest.json` recorded from your properties YAML at the
    last parse. Without either there is no build comparison; `dbt.status`
    says so under `breaking_changes`;
  - **the file as last saved**, while the open buffer differs from it:
    what *this edit* breaks, whether or not anything was ever built. The
    report carries it as `sinceSave`, its baseline `saved`.

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
an enforced contract and whether it is `access: public`), the semantic
models and metrics that read it at column level (the walk of
[column-reach.md](column-reach.md)), the model's exposures (node level),
and whether the model itself is public (consumers outside the project).

## Where you see it

- A diagnostic per removed or retyped column, code `breaking-change`:
  an **error** when a test or a contracted downstream model relies on a
  removed column, a **warning** when anything else reads it, otherwise
  information. The message is short — "column 'amount' was removed in
  this edit (against the saved file): breaks 3 models, 2 tests, 1
  exposure" — and the related information lists each test, downstream
  column, semantic model, metric and exposure at its place. A column the
  edit breaks is reported once, as the edit's.
- **Where it sits**: a retyped column at the select item that produces
  it (the root's, or the CTE's when the root passes it on by `*`). A
  removed column has no item any more: when an added column looks like
  its rename, the diagnostic sits on that column's item; otherwise on the
  root query's `select` keyword, the list it is missing from. When the
  SQL gives no exact place (Jinja control blocks, a parse hole) it stays
  at the top of the file.
- **"Keep '<old>' as an alias"**: on a likely rename whose new column is
  a bare column or literal (`id as customer_key`), a quick fix adds
  `, id as customer_id` after it, so downstream readers keep working. No
  fix is offered when the replacement is not obvious.
- A code lens, `N breaking changes`, that opens the full report.
- The command `dbt.breakingChanges` (`:DbtBreakingChanges` in Neovim,
  `M-x cogitator-breaking-changes` in Emacs, "dbt: Breaking changes
  against the last build" in VS Code). It uses the open buffer, saved or
  not.

The diagnostics and the lens are computed off the request path: when you
open or save a model (the file on disk), and as you type, after the
refresh debounce (`refresh.debounce_ms`, the open buffer against the build
and the saved file). The downstream models are read from disk (at most
200, as for `dbt.impact`). Clients that pull diagnostics see the findings
on their next pull.
