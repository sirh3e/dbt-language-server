# 00 — IR pipeline overview

Binding for the HIR → MIR → LIR work. This directory states the contract
the crates implement.

## Goal

A compiler-style pipeline for dbt SQL+Jinja that gives more information
(better lineage, typed columns, a plan view) and optimisation advice
grounded in DuckDB's and DataFusion's optimisers. Output is editor
diagnostics, code actions and a `dbt.queryPlan` command. SQL changes only
through an accepted fix; no whole-query rewriting.

```
template ─ mask+segment ─► HIR ──verify──► MIR ──verify──► canonicalise ──verify──► detect
target/compiled ──────────▲ (bound, spans)   (typed algebra)   (fixpoint, traced)       │
                                                    └─ backend ─► LIR ──verify──► detect ─► findings + plan view
```

## Crates

| Crate | Depends on | Owns |
|---|---|---|
| `cogitator-ir` | serde, petgraph only | Arenas, HIR/MIR/LIR types, `SqlType`, verifiers, printers, `testkit` feature |
| `cogitator-sql-front` | ir, parser, dialects, error, sqlparser, minijinja | HIR construction. The only crate here that sees sqlparser or `minijinja::machinery` |
| `cogitator-lower` | ir | HIR→MIR, MIR→LIR, inference, `backends/` per dialect |
| `cogitator-rules` | ir | Pass manager, trace, registry, rules |

`cogitator-ir` must never depend on `cogitator-error` (it would pull in
sqlparser). `cogitator-analysis` depends on all four and owns the feature
layer (`features/plan/`, project schemas). The default build gains no
heavy dependency; oracles are separate (07).

## Lowering contract

```rust
pub struct Lowered<T> { value: Option<T>, fidelity: Fidelity, degraded: Vec<Degradation>, violations: Vec<Violation> }
```

`Fidelity` is `Complete | Partial | Declined`. A `Degradation` is an
expected gap in the input; a `Violation` is our own bug. A violation drops
that level and everything below it (`Lowered::with_violations` clears
`value`), is logged, and marks the level unavailable in the plan view. No
violation may panic the server (rust-port 05 §2).

## Invariants

Every invariant has a variant in `Invariant` and a test that shows it can
fail. Verification runs `VerifyMode::EveryRewrite` in debug builds and
`Boundaries` in release (`VerifyMode::for_build`). Tests and fuzz targets
call `assert_clean()`.

| Level | Invariant | Variant |
|---|---|---|
| HIR | Ids in range, including `Bound.col` on a closed FROM item | `HirIdRange` |
| HIR | Scopes form a tree | `HirScopeTree` |
| HIR | Every `ColRef` has a `Resolution` (`Unresolved` counts; never bound does not) | `HirColRefResolved` |
| HIR | CTE references point backward, or to itself when recursive | `HirCteBackward` |
| HIR | Each Jinja region consumed at most once; Relation/Scalar/Composite exactly once | `HirJinjaConsumed` |
| HIR | Spans on char boundaries of the supplied text | `HirSpanBoundary` |
| MIR | Ids in range; each operand and subquery precedes its user | `MirIdRange` |
| MIR | Each ColId defined once in the reachable plan | `MirColDefinedOnce` |
| MIR | Each ColId visible where read; correlation declared and bound | `MirColVisible` |
| MIR | Declared output equals computed, including CTE arity | `MirDeclaredOutput` |
| MIR | CTE reference graph acyclic | `MirCteAcyclic` |
| MIR | Provenance total: every reachable node maps through `Origin` to HIR | `MirProvenanceTotal` |
| MIR | Rewrites preserve root width, order and names | `MirRootColumns` |
| MIR | Types never lose specificity (references and rewrites) | `MirTypeSpecificity` |
| LIR | Ids in range; operands precede users | `LirIdRange` |
| LIR | Every node maps to a node of the current MIR plan | `LirMapsToMir` |
| LIR | Root columns equal MIR's | `LirRootColumns` |
| LIR | Pruning keys belong to the scan | `LirPruningKeyOwned` |
| LIR | Only hints and CTE policies the backend supports | `LirHintSupported` |

Entry points: `verify_hir(&Hir, text)`, `verify_mir(&Mir, Option<&Hir>)`,
`verify_rewrite(&Mir, before: RelId)` (run each time `root` moves),
`verify_lir(&Lir, &Mir, &Capabilities)`. Id range runs first and ends the
run, because the other checks follow ids.

## Determinism

No HashMap iteration in any output path: findings sort by (span, id),
printers and the trace use arena or `BTreeMap` order. Budgets are
count-based, not clocks (04). `plan_hash` (FNV-1a over the printed MIR,
which omits node ids) is the fixpoint test.

## Caps

Each degrades explicitly. Front end: 512 KiB text, 40k tokens, expression
depth 256, 200k IR nodes. The pass manager has its own smaller budget (04).

## Performance targets (proposed, not measured)

Full plan on a cache miss p95 ≤ 15 ms, offloaded only; cached plan ≤ 1 ms.
The push-diagnostics path reuses a cached plan and never computes.
`DerivedCache` slot keyed by (uri, generation, schema revision).
