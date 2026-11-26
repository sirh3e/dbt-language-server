# Type checks

cogitator infers the type of every expression a model computes (from SQL
inference over its upstream models, the catalog, enforced contracts and YAML
`data_type:`) and reports the three mistakes an implicit cast hides. They are
correctness problems, not performance advice, so they report under every
`perf.profile`, including the default `off`.

| Code | Rule id | What it flags |
|---|---|---|
| `type-text-temporal-compare` | `type.text_temporal_compare` | text compared with a date, time or timestamp in a WHERE, HAVING, ON or CASE (`=`, `<>`, `<`, `<=`, `>`, `>=`, and a simple `CASE x WHEN …`) |
| `type-aggregate-text` | `type.aggregate_text` | `sum`, `avg`, `stddev`, `variance`, `median` (and their variants) over a text column |
| `type-join-key-mismatch` | `join.key_type_mismatch` | join keys of different type families (a number against text, a date against a number, …) |

## Severity

- **Warning** when every type involved is *proven*: SQL inference over proven
  inputs, the catalog (`target/catalog.json`), an enforced contract, or an
  explicit `cast(…)`.
- **Info** when one side rests on a YAML `data_type:` nothing enforces, or on
  a column whose type cogitator could only guess: the claim may be wrong.
- Nothing at all when a type is unknown.

A string *literal* against a date is the idiom (`order_date >= '2024-01-01'`)
and is never reported.

## How it runs

The project schema job, which already builds every model's plan to infer its
columns, runs these checks too; a pulled diagnostic reads them off its
published result and never plans the model. They therefore follow the
**saved** file: while a model has unsaved edits its type diagnostics are not
shown (they would point at the wrong text), and they come back once it is
saved. With a `perf.profile` other than `off` (or a `perf_rules` entry for a
rule outside this page) the model is planned on pull anyway and the checks
follow the open text.

Measured on `testdata/jaffle_shop_duckdb` (`customers.sql`, debug build, 40
pulls after an edit): planning per pull to run these took the median pull from
3.7 ms to 8.2 ms; read off the snapshot it is 3.9 ms against 4.3 ms before.

## Configuration

```yaml
types:
  checks: false          # leave them to perf.profile (default true)
perf_rules:
  type.aggregate_text: off   # or hint | info | warning
inlay:
  types: true            # the type after each select item (default false)
```

Suppress one occurrence with `-- cogitator: ignore type.aggregate_text`
(the rule id, not the `type-…` code).

## Type inlay hints

With `inlay.types: true` each output column of a model's final SELECT shows
its inferred type after the select item: `: bigint`, `: ≈ varchar` when the
type is not proven, nothing when it is unknown. The tooltip says why the type
is what it is. A `UNION` model shows none (its columns are its arms'), and a
`select *` only on aliased items.
