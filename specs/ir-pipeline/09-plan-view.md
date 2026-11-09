# 09 — Plan view

`dbt.queryPlan` (one argument object: `{uri, position?, …}`) shows each IR
level and which rules fired, for one model. Always available, independent of
`perf.profile`. Neovim wrapper `:DbtPlan`.

## Content

Per level: the deterministic text from `print_hir`, `print_mir`, `print_lir`,
or a level-unavailable note naming the violations or the degradation that
declined it. Plus the trace (`TraceEntry` per rule visit, including `Skipped`
with reasons) and any `incomplete` budget overrun.

Stale compiled plans show a banner and produce no diagnostics (01). `Unmapped`
spans appear only here.

## Compiled plans

A model whose control flow the template cannot express is planned from
`target/compiled` when the shell found a file (`PlanInput.compiled`, read off
the dispatcher thread and capped by `limits.compiled_sql_mib`). The result
reports:

| Field | Meaning |
|---|---|
| `origin` | `{kind: "compiled", staleness, coverage}`; `coverage` is the basis points of compiled text anchored to the template |
| `banner` | Present when the plan is not known fresh: older than the model, or the template's SQL no longer matches (unsaved edits count), or timestamps unavailable. Absent on template plans |
| `guidance` | Set on `declined`: distinguishes no compiled file, a file over the cap, and an unusable file; each points at `dbt compile` |

A stale file still produces a plan with a banner; only a missing, oversized or
unparsable file declines.

Each finding carries:

| Field | Values |
|---|---|
| `span` | Template offsets, except `placement: unmapped`, which keeps compiled offsets |
| `placement` | `template`, `exact`, `inJinja`, `unmapped` |
| `diagnosticsEligible` | The one field a consumer needs. False on a stale or unknown-age compiled plan and on `unmapped` spans; true for template plans and for fresh `exact`/`inJinja` spans |

`fixes` is non-empty only for `Fresh` + `exact`; a fix with any edit that is not
`exact` is withheld whole. Degradation spans and the printed levels stay in
compiled offsets. `model.unused_cte` and other `AllBranches` rules record
`Skipped` on compiled plans.

## Compute

Offloaded (`offload_blocking`), cached in a new `DerivedCache` slot keyed by
(uri, generation, schema revision). The push-diagnostics path reuses a cached
plan and never computes one.

## Lineage

`lineage/from_hir.rs` migrates column lineage to HIR, keeping the
`column_lineage` signature and `ModelLineage` JSON so all consumers stay green.
Legacy code moves under `cfg(test)` for a parity test over all fixtures,
comparing sources as sets with an explicit allowlist of accepted differences,
and is removed once parity holds.
