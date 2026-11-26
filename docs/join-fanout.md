# Join fan-out (`join.fanout`)

A join duplicates the rows of one side whenever the key on the other side is
not unique. That is often meant (orders joined to their payments) and often a
bug (a lookup table with a duplicate key, silently doubling a sum).
`join.fanout` says so where cogitator cannot tell:

> this join may duplicate rows of `orders`: `customer_id` is not known to be
> unique in `customers`

## When it reports

Any equi-join (`INNER`, `LEFT`, `RIGHT`, `FULL`) whose key on the joined side
(the right side; the left one for a `RIGHT JOIN`) is not *known* to be unique,
unless the grain is restored above it. Known-unique is:

- a strict `unique` test on the column (no `where:`, not `severity: warn`);
- a column constraint `unique` or `primary_key`;
- a model-level `constraints:` entry (`primary_key` / `unique` with
  `columns:`) or a strict `dbt_utils.unique_combination_of_columns` test,
  when the join equates all of its columns;
- an incremental model's `unique_key` (one column, or all of a composite one);
- a joined subquery or CTE that is one row per key: a `GROUP BY` on it, a
  `DISTINCT`, or `qualify row_number() over (partition by key …) = 1`.

The grain counts as restored by a `GROUP BY` on a join key above the join,
or a `DISTINCT` over it (that case is `join.distinct_fanout`'s).

## Severity and profile

A **Hint**, on by default under every `perf.profile`, including `off`. It is a
correctness hint, not performance advice: it never claims the rows *are*
duplicated, only that nothing proves they are not, so it is not a Warning.
Turn it off with `perf_rules: {join.fanout: off}`, or state the intent on the
line with `-- cogitator: ignore join.fanout -- one row per payment`.

Like the type checks it runs in the project schema job, so it follows the
saved file (see [type-checks.md](type-checks.md), "How it runs").

## Quick fixes

- **Add a unique test on `<relation>.<key>`** edits the YAML that documents
  the column: it appends `- unique` to the column's `data_tests:` (or
  `tests:`), or starts that list. Offered only when the column is listed in
  YAML; a flow list (`tests: [not_null]`) is left to you.
- **Deduplicate `<relation>` on `<key>`** (changes results) adds a CTE
  `<relation>_dedup` that keeps the first row per key and joins it instead:
  `qualify row_number() over (partition by key order by key) = 1` on
  Snowflake, BigQuery and DuckDB, `distinct on (key)` on Postgres, a filtered
  subquery elsewhere (which adds a `dedup_row` column). Edit the `order by` to
  choose which duplicate stays.
