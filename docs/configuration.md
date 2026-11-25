# Configuration

How cogitator finds your dbt profiles and connects, its own configuration
file `cogitator.yml`, dbt Fusion, the performance advice and telemetry.
Everything here is optional: with a `dbt_project.yml` and a `profiles.yml`
where dbt looks for one, the server needs no setup.

## dbt profiles

The SQL dialect comes from your dbt `profiles.yml`. It can live anywhere;
the first location that contains the project's profile with a resolvable
target wins:

1. `--profiles-dir <dir>` (CLI, `~` is expanded) or
   `initializationOptions.profilesDir` sent by your editor at initialize
   (the client option replaces the CLI flag)
2. `$DBT_PROFILES_DIR`
3. `<workspace root>/profiles.yml`
4. `<workspace root>/.dbt/profiles.yml`
5. `~/.dbt/profiles.yml` (`%USERPROFILE%\.dbt\profiles.yml` on Windows)

If no candidate resolves, the server sends a warning listing every path it
searched. In the Neovim plugin, `profiles_dir` in `setup()` or
`:DbtProfiles <dir>` at runtime configures this; in Emacs it is
`cogitator-profiles-dir`; in Zed it is
`lsp.cogitator.initialization_options.profilesDir`.

## Connection auth manager (Snowflake first-class)

Snowflake alone has half a dozen ways to connect — password, key pair,
SSO (`authenticator: externalbrowser`), OAuth, MFA, JWT. The server parses
profiles.yml into a **redacted inventory** (auth methods and what is
missing, never a credential value) and validates each target:

- unresolved `{{ env_var('…') }}` references (no value, no default)
- password auth without `user`/`password`, key-pair auth whose
  `private_key_path` does not exist, OAuth without `token`, missing
  `account`, unknown `authenticator` values

Problems surface three ways: a warning when the *active* target is broken,
diagnostics inside profiles.yml when you open it, and the
`dbt.profiles` report.

**Read scope**: by default only the profile named in `dbt_project.yml` is
parsed and exposed; widen it with `--profiles-allow name1,name2` or
`initializationOptions.profilesAllow`.

**Active target**: the profile's default `target:` unless overridden by
`--target` / `initializationOptions.target` — or switched at runtime with
the `dbt.setTarget` command, which re-resolves the SQL dialect and passes
`--target` to dbt run/test/compile. In Neovim: `:DbtTarget` (a picker
showing each target's auth method and issues); in Emacs:
`M-x cogitator-target`.

## Configuration file

`cogitator.yml` (or `.cogitator.yml`) at the workspace root is read at
initialize and **hot-reloaded** while the server runs — a stamp poll
every 2 s, a fast-path when you save the file through the editor, and
an explicit `dbt.reloadConfig` command. Every key is optional:

```yaml
profiles:
  dir: ~/.dbt            # ↔ --profiles-dir
  profile: jaffle_shop
  target: dev            # ↔ --target
  allow: [other_profile] # ↔ --profiles-allow
limits:
  compiled_sql_mib: 2    # dbt.compiledSql wire cap
refresh:
  debounce_ms: 200       # context-refresh coalescing window
startup:                 # honored only via --config at launch
  debug: false
  otlp: ""
  tree_sitter: true
  fusion: dbt
perf:
  profile: off             # off | recommended | strict: which plan rules
  catalog_stats: true      # report as diagnostics (dbt.queryPlan always works)
  row_hints: true          # CTE row counts proven by catalog stats, as inlay hints
  inline_findings: false   # the reported findings as inlay hints too
perf_rules:                # per-rule severity or off; wins over the profile
  join.cartesian: warning  # off | hint | info | warning
  filter.non_sargable: off
lint:                      # style checks default off; macro_calls,
  select_star: false       # dispatch (docs/macros.md) and syntax
  macro_calls: true        # (docs/lint-and-format.md) default on
  dispatch: true
  syntax: true
inlay:
  vars: true               # the value after each var() (docs/variables.md)
  env_vars: true           # the value after each env_var(), secrets masked
  jinja: false             # what short {{ … }} expressions render to
preview:                   # dbt.previewCte, the "Preview rows" lenses (docs/preview.md)
  rows: 20                 # rows when the call names no limit
  max_rows: 1000           # larger limits are capped
vars:                      # overrides, like dbt's --vars; passed to dbt runs
  since: '2025-06-01'
```

Hot semantics are **changed-keys-only, last writer wins**: a reload
applies only keys whose file value changed, so editing `target:`
updates the live target even after a `dbt.setTarget`, while untouched
keys never clobber runtime switches. Keys removed from the file (or
the whole file, when deleted) revert to their launch values. Each
applied change shows one INFO message naming the keys; a file that
stops parsing shows the syntax error and keeps the previous values.
Unknown keys warn with a did-you-mean, never fail.

Launch precedence: defaults < config file < CLI flags <
`initializationOptions`. The `startup:` keys (install-once subsystems)
only apply when the file is passed at launch via `--config <path>`;
found in the root file, they warn to restart instead. `dbt.status`
reports the file's path, whether it loaded, and the last parse error.

## dbt Fusion static analysis

With dbt Fusion installed, compilation results come back as diagnostics in
the editor; artifacts are written outside your project.

```
-f, --fusion=[path]
```

Without a path, `dbt` from `$PATH` is used. The binary is validated at
startup (its `--version` must report `dbt-fusion`), with a clear message
if not.

## Performance advice

Off by default. Set `perf.profile` in `cogitator.yml` to `recommended` (the
rules whose findings are claims) or `strict` (everything), or name single
rules under `perf_rules`; `dbt.queryPlan` shows the plan whichever you pick.

The advice comes from reading your SQL statically: nothing here runs a query
or connects to a warehouse, and SQL changes only through a quick fix you
accept. Findings report through pull diagnostics, and the next save
republishes them for push clients (the push path reuses a computed plan and
never plans). A model with Jinja control flow is planned from
`target/compiled` when that exists.

Suppress a finding with a `{# cogitator: ignore join.cartesian -- reason #}`
comment, or `-- cogitator: ignore …`, on the same line or the line above;
`ignore-file` covers the whole file. Suppressed findings stay visible in
`dbt.queryPlan`.

## Observability

Off by default. `--otlp[=endpoint]` (or `OTEL_EXPORTER_OTLP_ENDPOINT`)
exports **traces** (one span per LSP message with method/id, context
refreshes with per-builder children, dbt invocations with exit codes,
fusion compiles with diagnostic counts) and **metrics**
(`lsp.requests`, `lsp.request.duration` ms histogram by method — the
spec's latency-budget measurement — `context.refreshes` +
`context.refresh.duration`, `scan.cache` and `derived.cache` hit/miss
counters, `dbt.invocations` by kind and outcome) over gRPC/tonic as
`service.name = dbt-language-server`. `--debug` appends a plain-text
log to `log.txt` beside the executable (falls back to a warning when
that directory is read-only). Nothing but LSP frames ever touches
stdout. **Privacy note:** spans and logs carry file paths and model
names — point `--otlp` only at collectors you trust with them.
