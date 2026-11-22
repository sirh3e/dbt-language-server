# 10 — Incrementality

Which `materialized='table'` models could be maintained incrementally
instead of rebuilt in full, by which dbt strategy, and why the others
cannot. Binding for `canon::incrementality`, `incr.could_be_incremental`,
the `incrementality` section of `dbt.queryPlan` and `dbt.incrementality`.

## 1. Theory: DBSP

Budiu, Chajed, McSherry, Ryzhyk, Tannen, *DBSP: Incremental Computation on
Streams and Its Applications to Databases*, SIGMOD Record 53(1):87–95,
2024; the full development is *DBSP: Automatic Incremental View
Maintenance for Rich Query Languages*, PVLDB 16(7):1601–1614, 2023
(arXiv 2203.16684).

A database is a stream of snapshots. With integration `I` (running sum),
differentiation `D` (difference with the previous value) and Z-sets
(rows with integer weights, a deletion weighing −1), a query `Q` has the
incremental version `Q^Δ = D ∘ Q ∘ I`, and the chain rule
`(Q₁ ∘ Q₂)^Δ = Q₁^Δ ∘ Q₂^Δ` lets every operator be incrementalized on its
own and the results composed. The analysis is that composition, bottom-up
over the canonical MIR.

| MIR operator | DBSP class | State | dbt strategy |
|---|---|---|---|
| `Filter`, `Project` (deterministic), `SetOp` UNION ALL | linear: `Q^Δ = Q` | none | `append` |
| `Join` inner/cross with one side static; LEFT with the growing side preserved; semi/anti with the subquery side static | bilinear, one side constant | the static side (already a table) | `append` |
| `Join` inner/cross, both sides growing | bilinear: `Δa⋈Δb + I(a)⋈Δb + Δa⋈I(b)` | both histories | `append`, noted `Bilinear`: filter both inputs or allow a lookback |
| `Aggregate` grouped by a bucket of the event column | linear per group (SUM, COUNT); MIN/MAX monotone under append | the newest buckets | `delete+insert` on the group keys |
| `Aggregate` grouped by other keys | per group | the touched groups, recomputed from the source | `delete+insert` on the keys; needs a high-water column (`max(event)`) to find new rows |
| `Distinct`, UNION (set) | non-linear (`H` over `I`) | the output set | `merge` on every column |
| `Window` partitioned by a key | per partition | touched partitions | `delete+insert` on the partition keys |
| `Limit`, a window over the whole table, an aggregate with no GROUP BY | non-linear, unbounded | the whole input | keep `table` |
| EXCEPT/INTERSECT, FULL join, an outer join preserving the static side, a semi-join or subquery over a growing input, GROUPING SETS, an operator over re-aggregated rows | non-linear | — | keep `table` |
| `now()`, `random()`, `uuid()`… in the output, or a filter on the clock | not time-invariant | — | keep `table` |
| `Opaque`, `{{ this }}`, Jinja relations | unknown | — | no verdict |

Aggregates are recomputed per touched group rather than updated from their
old value, so every aggregate is exact under `delete+insert` — notes say
which ones (`count(distinct)`, medians) could not be updated in place.

## 2. What must hold, and where it comes from

The SQL cannot say how an input changes between runs. The analysis never
assumes it:

1. **Growth of each input** (`IncrementalInputs`, built by
   `features/plan/incremental.rs`), strongest first:
   - declared: a snapshot changes; an incremental upstream built by `append`
     or `microbatch` only grows, by `merge`, `delete+insert` or
     `insert_overwrite` changes; `event_time` (manifest or YAML, entry or
     `config:`) and a source's `loaded_at_field` name the growing column —
     evidence `Exact`, severity Info;
   - inferred: a temporal column named like an event time (`ts`,
     `event_time`, `*_at`, `*_ts`, `*_time`, `created*`; never `updated*`
     or `modified*`) — evidence `Heuristic`, severity Hint, and the message
     says the assumption and how to declare it;
   - neither: the input is assumed static and the verdict says so
     (`AssumesStatic`); with a partially known schema it gives no verdict.
2. **The event column is non-null and later rows carry later values.** The
   append filter is strict (`>`), so a row arriving later with the newest
   value already built is missed; the bucket filter (`>=`) recomputes the
   newest bucket whole and has no such gap.
3. **The growing column reaches the output** (itself, a cast to a temporal
   type, a bucket `date_trunc`/`date`/`time_bucket`, or a constant shift),
   so the high-water mark is read back from `{{ this }}`.
4. **Keys** for `delete+insert`/`merge` are output columns.
5. **Exact arithmetic**: SUM/AVG over a float is noted, as rounding can
   differ from a full rebuild.

## 3. Surfaces

- `incr.could_be_incremental` (MIR, Advice, `Needs::Mat`, tranche three):
  only on `materialized='table'` with an incremental verdict, anchored on
  the `config()` call. Off in the recommended profile until proven.
- The fix (`FixSafety::ChangesIntent`: never preferred, never in fix-all),
  offered for `append` and bucket verdicts where the places are certain
  (template origin, one SELECT reading the driving input, a qualifier where
  it reads several relations):
  - `config(materialized='incremental', incremental_strategy=…[, unique_key=…])`,
    editing the call or adding one;
  - `{% if is_incremental() %} where|and ((select max(out) from {{ this }})
    is null or col > (select max(out) from {{ this }})) {% endif %}` (`>=`
    and the bucket for buckets) on that SELECT, an existing WHERE
    parenthesized first. The `is null` arm is required: a model built empty
    has no newest value, and a comparison with NULL would let nothing in,
    ever (the oracle's empty fixtures hold this).
- `dbt.queryPlan` → `incrementality` for table models: `verdict`
  (`append | byBucket | byKey | keep | static | unknown`), `strategy`,
  `uniqueKey`, `inputs` (`relation`, `column`, `declared`), `reasons`
  (`reason`, `span`) and `summary`.
- `dbt.incrementality`: the same for every table model, slowest last run
  first (`run_results.json`), with `incremental` counting the candidates.

## 4. Oracle

`testdata/oracle/incr_*` with `[incremental]` in `expect.toml` and a
`delta` per fixture (07): the model is built on the seed as `this_model`,
the inputs replaced by the delta, the increment (the fix's branch,
`{{ this }}` rendered `this_model`) applied by its strategy — append as
`old ∪ increment`, delete+insert as `old rows whose key the increment
lacks ∪ increment` — and the result must be bag-equal to the model rebuilt
in full. Only `CREATE TABLE … AS`, so both engines run it. A P case proves
an increment; an N case shows why a blocker blocks (a witness that
differs, a precondition that agrees).

## 5. Limits

- Materialization is read from `config()` and the manifest; a project-level
  `+materialized: table` in `dbt_project.yml` is seen only once
  `manifest.json` exists.
- A `ByKey` verdict gets a fix only with one key, passed through unrenamed
  from the input, and a high-water column (`max(event)`) in the output: the
  touched-groups filter reads the key on the input side. Otherwise it is
  advice alone.
- Splitting is the subject of §6.

## 6. Splitting at the frontier, and S-expressions

### When a split pays

One blocker keeps a whole model a table. When most of its work is
incremental — a CTE that scans and aggregates a large growing input under a
global `rank()` — materializing that part changes `Q = Q_keep ∘ Q_inc` into
`Q_keep ∘ T`, with `T` maintained by `Q_inc^Δ`: `Q_inc` then costs the new
rows only, and `Q_keep` still rebuilds, but over `T`. That pays only when `T`
is smaller than what `Q_inc` reads, so a cut must **reduce**: aggregate,
deduplicate or filter. Cuts are text boundaries (CTEs) whose body reads no
other CTE, so the SQL is the one written.

`canon::incrementality::frontier` finds the cuts of a model whose verdict is
`keep`; each CTE body is classified as a model of its own (`classify_at`).
Refused, each with its reason: `no-reduction` (the part only projects),
`no-boundary` (the growing work shares a SELECT with its blocker),
`reads-cte`, `not-incremental`. `features::plan::split_plan` takes the first
cut: the CTE becomes `<model>__<cte>`, planned as a table model and rewritten
by `incr.could_be_incremental`'s own fix — for a per-key aggregate with a
column of each group's newest event time, the touched-groups form
`key in (select key from <input> where col > max(high_water))` with
`delete+insert` on the key — and the model reads it.

### Format

One printer, `cogitator_ir::sexp` (flat where it fits, one child a line under
its head otherwise), and `mir::sexp` for plans: relations by operator,
columns and tables by name, no ids, CTE references with their body inline.

```
plan   := (scan NAME) | (filter PRED PLAN) | (project (items …) PLAN)
        | (join KIND (on PRED) PLAN PLAN) | (aggregate (keys …) (aggs …) PLAN)
        | (window (fn F (partition …) (order …) NAME) PLAN) | (distinct PLAN)
        | (union-all PLAN…) | (sort … PLAN) | (limit N PLAN) | (cte-ref NAME PLAN) | (opaque)
class  := (class grows | static | linear | bilinear | per-group | per-bucket
                | dedupe | per-partition | kept | (blocked REASON))
```

1. **Analysis** (`dbt.queryPlan` → `incrementality.sexp`): the verdict, the
   notes, and the plan with each node's class appended.
2. **Frontier** (`incrementality.frontier`, for a kept model): each cut with
   its boundary span, its own verdict and what it reduces by; each refusal
   with `(because "…")`.
3. **Split** (`dbt.splitPlan` → `sexp`): the part (materialization, strategy,
   unique key, CTE, reduction, increment filter), the model over it, and the
   cost before and after.
4. **Refusal**: `(split MODEL (refused REASON (cte NAME) (because "…")))`.

The worked example, a global rank over a per-user rollup:

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

### Surfaces and proof

A `refactor.extract` code action carries the split out (the part created, the
model edited); `dbt.splitPlan {uri}` returns the S-expression and the part's
file. The oracle's split mode (`part` and `outer` under `[incremental]`)
maintains the part through the delta, computes `outer` over it as `__next`,
and requires it to equal `model.sql` rebuilt in full
(`testdata/oracle/split_rank_over_users`).

### Limits

- One cut per plan, CTEs only; a model with everything in one SELECT is
  refused `no-boundary` (cutting anywhere needs a MIR→SQL printer).
- A per-key part needs one key and a column with each group's newest event
  time (`max(ts)`) in its output; otherwise it is refused `not-incremental`.
