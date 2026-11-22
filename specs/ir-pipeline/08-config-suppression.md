# 08 — Config, suppression and fixes

`cogitator.yml`:

```yaml
perf: { profile: off, catalog_stats: true, row_hints: true, inline_findings: false }   # off | recommended | strict
perf_rules: { join.cartesian: warning, filter.non_sargable: off }
```

Default `off` for the first release, matching the existing opt-in lints;
`dbt.queryPlan` is always available. Flip to `recommended` once false positives
are measured.

A rule named in `perf_rules` reports even under `profile: off`: naming a rule
is the opt-in. `lint.select_star: true/false` aliases
`cross.select_star_chain` (on at its own severity, or off) unless `perf_rules`
names that rule itself; the existing style lint keeps working beside it.

`catalog_stats: false` makes catalog row counts unusable as a fact
(`Need::Stats` stays unmet); clustering keys are physical layout and stay.
No rule reorders by statistics yet.

`row_hints` (default true) shows each CTE's row count as an inlay hint when
catalog statistics prove it (06): an exact count through row-preserving
operators, an upper bound through filters, groups, DISTINCT, LIMIT and
semi/anti joins, and nothing through other joins, unnest, opaque holes or
without statistics. It needs `catalog_stats` and a catalog with row counts;
the `profile` does not gate it, as hints are no rule findings.

`inline_findings` (default false: the diagnostics already show them) also
shows every finding the diagnostics report as an inlay hint
(`features/plan/finding_inlay.rs`) — the same eligibility, suppression,
profile and `perf_rules` severity, so a rule that is off or below the
profile shows none. The hint sits at the end of the first line of the
finding's span and reads `<icon> <rule id>` (`⚠` warning, `ℹ` info, `💡`
hint); its tooltip is the whole finding (message with the evidence tag,
`rule: <id> (<severity>)`, a `suggestion:` line per fix), and its label part
carries a `dbt.queryPlan {uri}` command. A row-count hint on the finding's
first line, within its span (the CTE's name), absorbs the finding into ONE
hint at the row count's position, the count first: `≤ 1,200 rows · ⚠
model.unused_cte`; several findings at one position share a hint in plan
order. Both kinds come from the one cached plan (`document_plan`), never a
second planning; a document that wants neither does not plan for hints.
`dbt.status` and `dbt.doctor` report it as `perf.inlineFindings`.

## Profiles

| Rule | `recommended` | `strict` |
|---|---|---|
| `join.left_nullified`, `join.cartesian`, `model.unused_cte`, `incr.missing_guard`, `incr.subquery_watermark`, `filter.non_sargable` | on | on |
| `join.distinct_fanout`, `set.union_distinct`, `cross.select_star_chain` | off | on |

`off` enables nothing. `recommended` is the rules whose findings are claims;
the rest wait for measured false positives. A rule added to the registry
appears only under `strict` until listed here (`RECOMMENDED_RULES`).

A finding reports at the severity it carries, dialect included (05), capped at
the rule's declared ceiling, so evidence (catalog statistics above all) can
never raise it. A `perf_rules` severity sets it outright, in either direction;
`off` silences the rule.

Suppression: `{# cogitator: ignore join.cartesian -- reason #}` or
`-- cogitator: ignore join.cartesian -- reason`, same line or line above, ids
separated by commas or spaces; `ignore-file` with ids, or bare for every rule.
A plain `ignore` with no id suppresses nothing. Unknown ids warn with
did-you-mean over the registry ids (`closest_matches`, rust-port 05 §2).
Directives are read from Jinja comment spans and sqlparser's own comment tokens;
if the text cannot be tokenized mid-edit, only the Jinja-comment directives
apply. Suppressed findings stay in `dbt.queryPlan` with `suppressed: true`; they
are dropped only on the way to the editor.

## Fix wire format

A finding's fixes travel in `Diagnostic.data` (`DiagnosticData`) as
`FixKind::MultiEdit { fixes: Vec<MultiEditFix> }`, where each `MultiEditFix` is
a title and its complete `Vec<TextEdit>`; one `CodeAction` is built per fix.
`token_range` still holds the finding's range; `suggestions` is empty.

`FixKind`'s unit variants keep their wire format (`"replace_token"`), so
existing diagnostics still decode. `MultiEdit` serializes as
`{"multi_edit": {...}}`. A payload that fails to decode becomes `None` through
`lenient_data` and contributes no actions.

Only `Fresh` + `Exact` spans carry fixes (01). P fixes may be offered as
preferred (`MultiEditFix.preserving`, the first one becomes `isPreferred`) and
are listed first; N fixes never are, and their titles say they change results.
`source.fixAll.cogitator` bundles the first P fix of each context diagnostic
(an overlapping one is skipped), and only when the client asks for a
`source.fixAll` kind.
