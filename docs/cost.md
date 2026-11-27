# What a build costs: the cost lens and `dbt.costReport`

cogitator reads `target/run_results.json`: each node's execution time and
its `adapter_response`, whose shape is the adapter's. It reads, where they
are present (a number or a numeric string, a few aliases),
`bytes_processed` (`total_bytes_processed`, `bytes_scanned`),
`bytes_billed` (`total_bytes_billed`), `slot_ms` (`slot_millis`,
`total_slot_ms`), `rows_affected` (`rows`) and `query_id` (`job_id`).
BigQuery reports bytes and slots; most adapters report only rows.

## The lens

The "last run" lens on a model grows its cost:

    last run: OK in 12.4 s · 3.2 GB scanned · slowest 5%

The bytes appear when the adapter reported them; "slowest N%" when the run
timed at least four models and this one is among the slowest quarter.

## `dbt.costReport {}`

Every model of the last run, slowest first, with its figures, plus:

- **the critical path** — the longest chain of `ref`s weighted by execution
  time: the models a build waits for one after the other, however many
  threads it has;
- **"make it incremental"** — for a model that is among the slowest quarter
  (or takes a minute or more) and is a `table` that the query plan says
  could be built incrementally (`incr.could_be_incremental`, see
  [incrementality.md](incrementality.md));
- `text`, the same as a plain-text report.

## Query history (optional)

dbt does not always record bytes. A warehouse's query-history export can
fill the gaps — never override what dbt recorded:

```yaml
# cogitator.yml
cost:
  query_history: target/query_history.json
```

The file is a JSON array of objects or JSON lines. A row matches a model by
`query_id` / `job_id` (the run result's `adapter_response.query_id`) or by
`unique_id` / `node_id` / `model`; its figures use the keys above. cogitator
never connects to the warehouse for this. **CSV is not read** (there is no
CSV parser in the build): export JSON. A broken file is logged and ignored.

## In the editors

Neovim `:DbtCostReport`, Emacs `M-x cogitator-cost-report`, VS Code "dbt:
Cost report of the last run". The Neovim DAG view (`:DbtDag`) does not mark
slow nodes: it has no hook for node annotations yet.
