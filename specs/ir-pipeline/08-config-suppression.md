# 08 — Config, suppression and fixes

`cogitator.yml`:

```yaml
perf: { profile: off, catalog_stats: true }   # off | recommended | strict
perf_rules: { join.cartesian: warning, filter.non_sargable: off }
```

Default `off` for the first release, matching the existing opt-in lints;
`dbt.queryPlan` is always available. Flip to `recommended` once false positives
are measured.

Suppression: `{# cogitator: ignore join.cartesian -- reason #}` or
`-- cogitator: ignore …`, plus `ignore-file`. Unknown ids warn with
did-you-mean over the registry ids (`closest_matches`, rust-port 05 §2).

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
preferred; N fixes never are, and neither enters `source.fixAll`. `CodeAction`
has no `isPreferred` member yet; the diagnostics work adds it.
