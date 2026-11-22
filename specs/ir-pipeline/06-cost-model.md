# 06 — Cost honesty

The pipeline reports structure and evidence, not predicted cost.

## Evidence tags

Each finding is tagged `Evidence::{Exact, Catalog { generated_at }, Heuristic,
Unknown}`:

| Tag | Meaning |
|---|---|
| `Exact` | Derived from the query structure alone, on a `Fresh` plan |
| `Catalog(generated_at)` | Backed by `catalog.json` stats, with their age |
| `Heuristic` | A structural guess; say so in the message |
| `Unknown` | The rule could not establish it |

## Rules

- Never dollars, and never selectivity percentages.
- Statistics may reorder and enrich findings but never raise severity (05).
- `Estimate::Heuristic(Bound)` is a bound, not a point estimate.
- `perf.catalog_stats` (08) switches off catalog enrichment entirely.

## Row hints

The plan reports `rowHints` — per CTE, what the scanned tables' catalog
`row_count` proves about its rows (`features/plan/row_hints.rs`, computed over
the MIR before the passes, template plans only): `exact` through Project,
Sort, Window and `UNION ALL` of exact inputs; `atMost` through Filter,
Distinct, grouped Aggregate, Limit (capped by a literal), semi/anti joins,
UNION, INTERSECT and EXCEPT; one row exactly for a global aggregate. Other
joins, Unnest, Opaque holes and grouping sets prove nothing, and a CTE that
reaches no statistic gets no hint — a guess is never shown. The editor sees
them as inlay hints after the CTE name (`1,200 rows`, `≤ 1,200 rows`).
