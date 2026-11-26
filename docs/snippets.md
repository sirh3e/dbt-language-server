# Model-aware snippets

In a model, completion offers two snippets filled from what cogitator knows
of the model rather than a template:

| Type | Snippet |
|---|---|
| `is_inc…` | an `{% if is_incremental() %}` block filtering on the model's growing column |
| `{% if ` (or `{% if is_`) | `is_incremental() %}` and the same filter, closing the tag (only `is_incremental()` when `%}` already follows) |
| `conf…` | `{{ config(materialized='incremental', unique_key=…) }}` with the model's merge key |

The filter is written as the incrementality rewrite writes it
(`rules/incr/watermark.rs`, shared):

```sql
{% if is_incremental() %}
    where ordered_on > (select max(ordered_on) from {{ this }})
{% endif %}
```

`and` instead of `where` when the statement before the cursor already
filters.

## Which column

1. the model's `event_time` (its YAML entry or `config:`, or the manifest's
   `config.event_time`);
2. an upstream's declared `event_time` / `loaded_at_field` that the model
   passes through (the incrementality analysis's declared delta input);
3. an output column whose name reads like an event time (`created_at`,
   `loaded_at`, `event_ts`, …; never `updated_at`, whose rows change) and
   whose type is a date or timestamp where known.

Only 3 is a guess, and a guess is a tab stop (`${1:created_at}`) to confirm;
with nothing known the column is a tab stop `${1:updated_at}`. The output
columns are the schema job's inferred ones, else the open text's own select
list, else the YAML's.

## Which merge key

A column with a `primary_key` constraint; else a single-column key
(`constraints: [{type: unique, columns: [x]}]`, a one-column
`unique_combination_of_columns`); else a column with a strict `unique` test;
else a composite key, written as a list (`['created_at', 'event_id']`);
else a tab stop `'${1:id}'`.

## Clients without snippets

The server reads `textDocument.completion.completionItem.snippetSupport` at
initialize. A client without it gets plain text: every tab stop replaced by
its default, no `insertTextFormat`. Likewise `labelDetailsSupport` and
`documentationFormat` decide whether `labelDetails` and Markdown
documentation are sent (`cogitator_lsp::adapt`).
