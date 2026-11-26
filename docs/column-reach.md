# Who reads this column? (column reach)

`dbt.impact` with a `column` answers more than "which models are
downstream": it follows the column through every downstream model's
lineage (renames included) and then past the models, to everything that
reads one of those columns.

| Consumer | How it is found | Level |
|---|---|---|
| Downstream models | the taint walk over each downstream model's typed lineage (renames, CTEs, `select *` that expands); capped at 200 models | column |
| Column tests | the `tests:`/`data_tests:` of the column itself and of every downstream column it feeds | column |
| Semantic models | an entity, dimension or measure over a reached model whose `expr:` names a reached column, or which has no `expr:` and whose `name:` is one — the same walk the column rename uses | column |
| Metrics | simple and cumulative metrics over a reached measure (`type_params.measure`), conversion metrics (`base_measure`, `conversion_measure`), then ratio (`numerator`, `denominator`) and derived (`metrics:`) metrics over those, to a fixpoint | column (via a measure) |
| Exposures | the exposures downstream of the model | **node** only: an exposure lists nodes, never columns |
| Dashboards | the BI manifest (below) | column when a read names the column, node when it names only the model |

Where it shows:

- `dbt.impact {uri, model?, column}` — the `reach` field and a summary
  ending in "read by 3 models, 2 tests, 1 semantic model, 1 dashboard"
  (Neovim `:DbtImpact <column>`, Emacs `M-x cogitator-impact`, VS Code
  `dbt: Who reads this column?`).
- The hover on a model's output column (in its SQL) or on a column entry
  of its properties YAML: "`orders.amount` is read by …". The hover never
  computes: on open and save the server works out every output column's
  reach of the model (or of the models a properties file documents, up to
  20) in the background, and the hover shows the last answer. Before the
  first answer, nothing is shown.

## The BI manifest

BI tools are the end of the lineage, but cogitator cannot ask them. Write
(or export) a JSON file and point `cogitator.yml` at it, relative to the
project root:

```yaml
bi:
  manifest: bi/dashboards.json
```

```json
{
  "dashboards": [
    {"name": "Revenue", "url": "https://bi.example/d/1",
     "reads": [{"model": "orders", "column": "amount"}, {"model": "customers"}]}
  ]
}
```

`name` is required, `url` optional; each read names a dbt model and,
where known, a column. Unknown keys are refused. A manifest that cannot be
read or parsed is named in the report's `notes` (with the reason and the
fix), never dropped silently; the hover leaves dashboards out then.

## What it does not do

- Metric `filter:` Jinja (`{{ Dimension('customer__region') }}`) is not
  followed: a dimension used only in a metric's filter does not reach
  that metric.
- Exposures are never column-level.
- Column hits under-approximate: an unexpanded `*` or unresolved lineage
  stays out, as in `dbt.impact`.
- The hover's cache is refreshed on open and save of the model (or its
  properties file), not when a downstream file changes.
