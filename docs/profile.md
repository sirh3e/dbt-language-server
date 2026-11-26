# Profiling a column: `dbt.profileColumn` and the hover

A column's profile — rows, distinct values, null rate, min and max, the ten
most common values with a sparkline (`▁▂▃▅▇`) — in the hover of the column
in the model's SQL.

**Off by default: it runs a query against the warehouse.** Turn it on:

```yaml
# cogitator.yml
warehouse:
  profile: true
  profile_sample_rows: 10000   # the default
```

## How it runs

The hover never queries. `dbt.profileColumn {uri, position}` (or
`{uri, column}`), the code action "Profile column 'x'" or the editor
commands run a background job; its answer fills a cache, and from then on
hovering the column shows the profile. A profile belongs to the last run:
after another dbt run (a new `run_results.json` `generated_at`) it is not
shown until the column is profiled again.

The column must be an output column of the model (from its column lineage,
or documented in its YAML); the query reads the model's own relation
(`dbt.relation`, with the manifest's `relation_name` where target/ has
one). Ephemeral models have no table and are refused.

One query, sampled per adapter:

| Adapter | Sample |
|---|---|
| duckdb | `using sample N rows` |
| snowflake | `sample (N rows)` |
| databricks, spark | `tablesample (N rows)` |
| others (postgres, redshift, bigquery, …) | a `limit N` subquery: the first N rows, not a random sample. **On BigQuery a `limit` does not reduce the bytes scanned**: the column is read, and billed, in full |

Engines: with a duckdb adapter in a build with the `duckdb-parser` feature,
the query runs on a **read-only** DuckDB connection to the target's file;
otherwise through `dbt show --inline <query> --output json` as a job
(`dbt.cancel` token, `$/progress`).

## In the editors

Neovim `:DbtProfileColumn` (a float), Emacs `M-x cogitator-profile-column`,
VS Code "dbt: Profile the column under the cursor"; every editor gets the
code action and the hover.
