# Incrementality: which table models could be incremental

A `materialized='table'` model is rebuilt in full on every `dbt run`. Many
could be maintained incrementally — reading only the rows that arrived
since the last run — and cogitator says which, by which dbt strategy, and
why the others cannot. For a model kept a table by one part of its SQL, it
proposes **splitting** that model so the rest becomes incremental.

The binding rules are in
[`specs/ir-pipeline/10-incrementality.md`](../specs/ir-pipeline/10-incrementality.md);
this page is the guide.

## Turning it on

The rule `incr.could_be_incremental` is off in the recommended profile until
it has seen more projects. Turn it on in `cogitator.yml` at the project root
(hot-reloaded on save):

```yaml
perf_rules:
  incr.could_be_incremental: info
```

## How it decides

cogitator turns each model into a query plan and works through it bottom-up,
the way DBSP (Budiu et al., *DBSP: Incremental Computation on Streams and
Its Applications to Databases*) incrementalizes a query: every operator gets
a class, and the classes compose.

| In the SQL | What it means for an incremental run |
|---|---|
| filters, projections, `UNION ALL` | linear: new rows in, new rows out — `append` |
| a join with a table that does not grow | the new rows join the whole other side — `append` |
| `GROUP BY` a bucket of the event time (`date_trunc('day', ts)`) | only the newest buckets change — `delete+insert` on the bucket |
| `GROUP BY` another key | the groups new rows touch are recomputed — `delete+insert` on the key, needs each group's newest event time in the output |
| `DISTINCT` | the output set — `merge` on every column |
| a window `PARTITION BY` a key | the touched partitions — `delete+insert` on the partition keys |
| `LIMIT`, a window or aggregate over the whole table, `EXCEPT`, a `FULL JOIN`, `now()` | needs the whole input every time — stays a table |

### Which inputs grow

The SQL cannot say how an input changes between runs, so cogitator looks for
evidence, strongest first:

1. **Declared:** an input built incrementally by `append`/`microbatch` only
   grows (one built by `merge`/`delete+insert` changes); a snapshot changes;
   `event_time` (in YAML or `config:`) or a source's `loaded_at_field` names
   the column that grows.
2. **Inferred:** a timestamp column named like an event time (`ts`,
   `event_time`, `*_at`, `created*` — never `updated*`). The finding says it
   assumed this, and how to declare it instead.
3. **Neither:** the input is assumed static, and the finding says so.

## What it tells you

On the model's `config()` call, a finding such as:

> This table could be built incrementally with `incremental_strategy='append'`:
> it is linear in `events` (filters, projections and joins to unchanging
> inputs), so each run needs only the rows added since the last, those with
> `ts` after the newest `ts` already in the model.

Or, for a model that must stay a table, every reason at its place in the
SQL — "LIMIT keeps only some rows of the whole input", "a window function
over the whole table (no PARTITION BY)", "a window over re-aggregated rows,
which change rather than arrive", "`now()` in the output is not a function
of the rows".

### The fix it offers

A code action rewrites the model (it changes what the model is, so it is
never applied by "fix all"). Three shapes, from the test suite:

**Append** — a filter that keeps an `OR` whole:

```sql
-- before
{{ config(materialized='table') }}
select id, ts from {{ ref('events') }} where a > 0 or b > 0

-- after
{{ config(materialized='incremental', incremental_strategy='append') }}
select id, ts from {{ ref('events') }} where (a > 0 or b > 0)
{% if is_incremental() %}
and ((select max(ts) from {{ this }}) is null or ts > (select max(ts) from {{ this }}))
{% endif %}
```

The `is null` arm is required: a model built empty has no newest value, and
a comparison with `NULL` would let nothing in, ever.

**By bucket** — a daily rollup recomputes its newest day:

```sql
{{ config(materialized='incremental', incremental_strategy='delete+insert', unique_key='day') }}
select date_trunc('day', e.ts) as day, count(*) as n from {{ ref('events') }} e
{% if is_incremental() %}
where ((select max(day) from {{ this }}) is null or e.ts >= (select max(day) from {{ this }}))
{% endif %} group by 1
```

**By key** — a per-user rollup recomputes the users new rows touch (it
needs a column holding each group's newest event time, here `last_ts`):

```sql
{{ config(materialized='incremental', incremental_strategy='delete+insert', unique_key='id') }}
select id, count(*) as n, max(ts) as last_ts from {{ ref('events') }}
{% if is_incremental() %}
where ((select max(last_ts) from {{ this }}) is null or id in
  (select id from {{ ref('events') }} where ts > (select max(last_ts) from {{ this }})))
{% endif %} group by id
```

## Splitting a model at its frontier

One blocker keeps a whole model a table. When most of the work is
incremental — a CTE that scans and aggregates a large, growing input, under a
global `rank()` — materializing that CTE as its own incremental model lets
each run read only the new rows, while the rest still rebuilds, but over the
much smaller part. That only pays when the part **shrinks** the data
(an aggregate, a `DISTINCT`, a filter); cogitator refuses a cut that saves
nothing, one with no CTE boundary to cut at, one that reads another CTE, and
one that is not incremental itself — and says why.

The model:

```sql
{{ config(materialized='table') }}
with by_user as (
  select user_id, sum(amount) as total, max(ts) as last_ts
  from {{ ref('events') }}
  where status = 'ok'
  group by user_id
)
select user_id, total, rank() over (order by total desc) as rnk
from by_user
```

The code action "Split 'by_user' into the incremental model
'orders_ranked__by_user' and a table over it" creates
`orders_ranked__by_user.sql`:

```sql
{{ config(materialized='incremental', incremental_strategy='delete+insert', unique_key='user_id') }}
select user_id, sum(amount) as total, max(ts) as last_ts
  from {{ ref('events') }}
  where (status = 'ok')
{% if is_incremental() %}
and ((select max(last_ts) from {{ this }}) is null or user_id in (select user_id from {{ ref('events') }} where ts > (select max(last_ts) from {{ this }})))
{% endif %}
  group by user_id
```

and leaves the model reading it:

```sql
{{ config(materialized='table') }}
select user_id, total, rank() over (order by total desc) as rnk
from {{ ref('orders_ranked__by_user') }} as by_user
```

## Reading the S-expressions

The analysis, the frontier and the split are also given as S-expressions —
nested lists that read top-down, one operator per list, with names in place
of ids.

**The analysis** (`dbt.queryPlan` → `incrementality.sexp`): the verdict, then
the plan with each operator's class appended.

```lisp
(incrementality orders_ranked
  (verdict keep (blocker over-changing-rows (span 147 174)))
  (note min-max)
  (plan
    (project
      (items user_id total rnk)
      (window
        (fn rank (partition) (order (desc total)))
        (cte-ref by_user
          (project
            (items user_id total last_ts)
            (aggregate
              (keys user_id)
              (aggs (sum amount) (max ts))
              (filter (= status "ok") (scan events (class grows)) (class linear))
              (class per-group))
            (class per-group))
          (class per-group))
        (class blocked over-changing-rows))
      (class kept))))
```

`events` grows, the filter is linear, the aggregate is per group — and the
`rank()` over the aggregated rows blocks (`(span 147 174)` is where, in
bytes of the model), so everything above it is kept.

**The frontier** (`incrementality.frontier`, for a model that stays a
table): the parts that would not.

```lisp
(frontier orders_ranked
  (cut by_user
    (boundary cte (span 20 64))
    (verdict by-key
      (strategy delete+insert)
      (unique-key user_id)
      (high-water last_ts)
      (delta (input events ts inferred)))
    (reduces aggregate)))
```

**The split** (`dbt.splitPlan` → `sexp`): the new part, the model over it,
and what each run reads before and after.

```lisp
(split orders_ranked
  (model orders_ranked__by_user
    (materialized incremental)
    (strategy delete+insert)
    (unique-key user_id)
    (from-cte by_user)
    (reduces aggregate)
    (increment
      (filter
        (or
          (is-null (max-of this last_ts))
          (in user_id (select user_id (scan events) (> ts (max-of this last_ts))))))))
  (model orders_ranked (materialized table) (reads orders_ranked__by_user))
  (cost (before (full-scan events)) (after (delta-scan events) (full-scan orders_ranked__by_user))))
```

**A refusal** names the reason: `(split m (refused no-boundary (because "the
growing work shares one SELECT with what blocks it; move it into a CTE")))`.

## Where to find it in your editor

| | Neovim | Emacs | VS Code | Zed |
|---|---|---|---|---|
| The finding and its fix | diagnostic + code action | ✓ | ✓ | ✓ |
| The split | code action, `:DbtSplit` (shows the plan; `a` applies) | code action, `M-x cogitator-split-plan` | code action, "dbt: Split model at its incrementality frontier" | code action |
| Every table model, slowest first | `:DbtIncrementality` (quickfix) | `M-x cogitator-incrementality` | "dbt: Table models that could be incremental" | — |
| The analysis as S-expressions | `:DbtPlan` | — | — | — |

The commands behind them: `dbt.incrementality`, `dbt.splitPlan` and
`dbt.queryPlan` ([commands.md](commands.md)).

## How it is checked

Every rewrite is held to a full rebuild by a differential oracle that runs
in DuckDB and DataFusion: the model is built on a seed, the input grows by a
delta, the increment is applied by its strategy, and the result must equal
the model rebuilt from scratch — with duplicates, `NULL`s and an empty model
among the cases. A split is checked the same way: the part is maintained,
the rest computed over it, and the whole must equal the original model.

## Limits

- One cut per model, at a CTE: a model written as one `SELECT` is refused
  ("move it into a CTE").
- A per-key part needs a single key that keeps its name from input to output,
  and a `max(event time)` column in its output; otherwise it gets the advice,
  not the fix.
- Materialization comes from `config()` and the manifest: a project-level
  `+materialized: table` is seen once `manifest.json` exists.
