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
