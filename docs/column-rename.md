# Renaming a column

Rename (F2 in most editors) works on a model's output column: on its
`- name:` entry under `columns:` in the properties YAML, or on the word in
the model's SQL (the alias, or a bare selected column). The edit reaches the
whole project in one `WorkspaceEdit`, or is refused as a whole — it is never
partial. Find references on a column shows the same places
(`features/rename_column/walk.rs` is the one walk both use).

## What changes

In the model itself:

- the select item that names the column. An alias is renamed
  (`id as customer_id` → `id as cust_id`); a bare column of an upstream
  relation gets an alias (`select status from {{ ref('x') }}` →
  `status as state`), so the upstream model is never touched;
- the item is found from the root query down through `select * from cte`
  and bare `cte.col` pass-throughs; the CTEs between that item and the root
  carry the new name, every reference to it inside the model follows, and
  nothing below the item changes;
- its properties YAML: the `columns:` entry (column tests move with it),
  and in the model's entry the column lists — `constraints:` → `columns:`,
  `primary_key:`, `combination_of_columns:` — and `column_name:`
  arguments; `description:` text and `docs` blocks are left alone.

Then every consumer, at any depth, **while the column keeps its name**:

| Consumer | Changes |
|---|---|
| a downstream model | its references to the column: qualified (`stg.old`), bare where only one relation can provide it, in every clause |
| one that outputs it under the same name (`select a.old`, `select *`) | renamed too, with its YAML entry, and the walk goes on from it |
| one that re-aliases it (`old as other`, `sum(old) as total`) | only the reference (`new as other`); the walk stops there |
| snapshots | read like models (their `{% snapshot %}` tags aside); consumers `ref()` them by name |
| singular tests, analyses | their references |
| `relationships` tests | `field:` when the sibling `to:` is `ref('<renamed model>')` |
| unit tests | `expect` row keys of a test of a renamed model; `given` row keys of an `input: ref('<renamed>')` |
| semantic models | the column in an `expr:`; an entity, dimension or measure that reads it by its `name:` alone gets `expr: <new>`, so its semantic name stays |
| metrics, exposures | nothing: they name measures, dimensions and nodes, not columns |

## When it is refused

The rename answers `RequestFailed`, naming each file and why, when any
affected file cannot be followed with certainty:

- Jinja control blocks (`{% if %}`, `{% for %}`) in a file that spells the
  column or has a `*`: the HIR declines them, as column lineage does;
- SQL that does not parse completely;
- a bare name two relations may provide (qualify it), or one the binder
  cannot place;
- `using (old)`, a star's `exclude`/`replace` naming it, `union by name`;
- the name inside a `{{ … }}` string (`{{ full_name('first_name') }}`):
  macros read columns by name, which nothing here can follow;
- the column coming into the model through `select *` from upstream
  (rename it there), a set operation, or more than once;
- a csv or sql unit-test fixture that names it;
- a new name that is not a plain identifier, or that is already a column of
  any query or model the walk reaches.

References still list the words of a file that rename refuses.
