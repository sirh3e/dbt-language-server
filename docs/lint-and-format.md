# Syntax errors, invalid models, `cogitator lint` and `cogitator format`

Four things that need no `dbt compile`, no dbt Fusion and no warehouse:

1. **Syntax errors as you type** in every `.sql` file of the project
   (models, macros, tests, analyses, snapshots).
2. **Invalid models as you type**: what makes dbt refuse a model, or
   build something other than what it says, with the fix as a quickfix.
3. **`cogitator lint`**: every diagnostic the editor would show, for every
   file of the project, from the command line — for CI.
4. **`cogitator format`**: the editor's formatter over every model, in
   place or as a check.

## Syntax errors as you type

On every `didOpen`, `didChange` and `didSave` (and in pull diagnostics)
cogitator checks the buffer's text in two layers:

| Code | Layer | What it finds |
|---|---|---|
| `jinja-syntax` | the Jinja, through minijinja's own parser | an unclosed `{% if %}` / `{% for %}` / `{% macro %}` (shown at the opener), a closing tag no block opened (`{% endif %}` alone), an unclosed `{{`, `{%`, `{#` or string, a malformed expression (`{{ ref('a' }}`) |
| `sql-syntax` | the SQL, with the Jinja masked byte for byte | what sqlparser rejects in the project's dialect: `select a,, b`, `where` with nothing after it, a misspelled keyword |

Each is an Error with source `cogitator`, at the offending range, and a
message with a Reason and a Solution. A file has at most one: the first
Jinja error, else the first SQL error. Compiled SQL under
`target/compiled` and `target/run` is not checked.

### Incremental models, loops and macros: checked as dbt renders them

A model whose Jinja is only `{{ … }}` is checked with the Jinja masked,
at the exact place. A model with control flow — `{% if is_incremental()
%}`, `{% for %}`, any other `{% if %}`, a `{% set x %}…{% endset %}`
block — or with an error right next to a `{{ … }}`, is **rendered the
way dbt renders it** and the result checked: the project's macros,
`ref()` and `source()` (relations with `.identifier`, `.schema`,
`.database`; `ref('m', v=2)` too), `this`, `var()`, `env_var()`,
`config()`, `target`, `execute` (true, as while dbt compiles),
`invocation_id` and `run_started_at`.

- An incremental model is rendered **twice**, `is_incremental()` false
  (a full refresh, the first run) and true (every later run), and each
  rendering is checked; the message says which one fails.
- The error is pointed at the model's own text where the rendered text
  around it is found there (a loop body, a `set` block), else at the
  model's first line, and the message quotes the rendered line.
- What needs a warehouse is not rendered, so not checked: `run_query`,
  `adapter.*`, `return(…)` from a macro, a package's namespace
  (`dbt_utils.…`) that is not among the macros read, a var with no value.
- In an editor, the first pull of diagnostics (`textDocument/diagnostic`)
  reads the project's macro files once per project state and builds the
  renderer; from then on every push (didOpen/didChange/didSave) reuses it,
  so a model with control flow is checked as you type. Before that first
  pull such a model's SQL is not checked. `cogitator lint` renders always.
- `RUST_LOG=cogitator::syntax=trace cogitator lint` says per file what the
  check did and why it declined.

### When it says nothing

A false syntax error is worse than a missed one, so the SQL layer
**declines** whenever it cannot be sure (the confidence rule of
`specs/rust-port/07` §3.2):

- a model with control flow that cannot be rendered (above);
- any other dialect sqlparser knows accepts the SQL;
- the error is at or right after a warehouse extension sqlparser parses
  only in part (`ASOF LEFT JOIN`, `POSITIONAL JOIN`, `ANTI`/`SEMI` joins,
  `lambda`, `PIVOT`, `COLUMNS(…)`, `EXCLUDE`/`REPLACE`/`RENAME`, `SAMPLE`…);
- the file is over 512 KiB.

The Jinja layer declines on what minijinja rejects but dbt's Jinja may
accept: statements minijinja does not know (`{% break %}`, an extension's
tag) and names it reserves. dbt's own block tags (`test`, `snapshot`,
`materialization`, `docs`) are understood.

### Turning it off, and other engines

```yaml
# cogitator.yml
lint:
  syntax: false   # default true
```

- With `--fusion`, dbt Fusion's compile owns diagnostics: the push path
  publishes Fusion's alone, and pull diagnostics leave the syntax check out.
- In a build with the `duckdb-parser` feature and a duckdb project,
  DuckDB's own parser reports SQL errors (source `duckdb 2.0`); cogitator
  then checks only the Jinja layer, so nothing is reported twice.

## Invalid models as you type

A model can parse and still be one dbt refuses, or one that builds
something else than it says. cogitator checks every model of the project
(a new one too, before it is saved) as it is written:

| Code | Severity | What it finds | The fix it offers |
|---|---|---|---|
| `invalid-materialization` | Error | `materialized='incrementl'`: a near miss of dbt's materializations (`view`, `table`, `incremental`, `ephemeral`, `materialized_view`) or of a `{% materialization %}` of the project or its packages | the closest names |
| `invalid-materialization` | Warning | a materialization that is no near miss of any of those (an adapter may still provide it; `dynamic_table`, `external` and `clone` are known) | — |
| `unknown-config-key` | Warning | a config key that is a near miss of one dbt or a common adapter reads (`materialised`, `unique_keys`) | the closest keys |
| `invalid-config-value` | Error / Warning | `access` outside `private`, `protected`, `public` (dbt refuses the project); `on_schema_change` outside `ignore`, `fail`, `append_new_columns`, `sync_all_columns` (dbt falls back to `ignore`); an `incremental_strategy` that is a near miss of dbt's and that no `get_incremental_<name>_sql` macro defines; `enabled='false'` and `full_refresh='false'`, strings that read as true | the closest values; the boolean |
| `ref-self` | Error | a model that refs itself | rewrite `ref('me')` to `this` |
| `ref-cycle` | Error | a ref whose model leads back to this one; the message names the cycle (`orders → payments → customers → orders`) | — |
| `is-incremental-never` | Warning | `is_incremental()` in a model whose own config materializes it as a `view`, `table`, `ephemeral` or `materialized_view`: the block never runs | — |
| `empty-model` | Warning | a model with nothing but comments and its `config(...)` | — |

The confidence rule of the syntax checks holds here too: what the text
cannot tell is never reported. A key outside the lists is reported only
when it is a near miss of one in them (adapters add keys of their own); a
value computed by a macro or a `var()` is not checked; a materialization
set in `dbt_project.yml` or the properties YAML is not used for
`is-incremental-never`. A cycle is found in the project graph as it was
last scanned, through the refs of the buffer as it is now.

Every fix rides on the diagnostic as typed data, so any LSP client offers
it as a quickfix (Neovim: `<leader>di` / `:DbtFix`, see the plugin's
help).

```yaml
# cogitator.yml
lint:
  model: false   # default true
```

## `cogitator lint`

```sh
cogitator lint                                  # the project in the working directory
cogitator lint --project-dir path/to/project models/staging
cogitator lint --format json                    # one JSON array
cogitator lint --format github                  # GitHub Actions annotations
cogitator lint --max-warnings 0                 # warnings fail the run too
```

It builds the project context the way the server does (`--profiles-dir`
and `--target` before the subcommand, and `cogitator.yml`'s `profiles:`)
and runs, over every model, macro, test, analysis and snapshot and the
YAML that describes them (plus `selectors.yml` and `cogitator.yml`):

- the syntax checks and the invalid-model checks above;
- the `lint:` checks cogitator.yml enables (`select_star`, …, and the
  default-on `macro_calls` and `dispatch`);
- dangling `ref()` / `source()` / `doc()`, unknown vars and env vars,
  private refs;
- dbt YAML schema validation, packages and exposure targets, selectors
  that match nothing;
- the project's ast-grep rules (`docs/ast-grep.md`).

Not in the CLI: what needs the editor's caches — the lineage-backed
`undocumented_columns` lint and contract drift, dbt artifacts, plan
findings and the breaking-change report.

Output is `path:line:column: severity code: message` on stdout, the
Reason and Solution indented below; a summary line goes to stderr.

| Exit | Meaning |
|---|---|
| 0 | no error (and no more warnings than `--max-warnings`) |
| 1 | at least one error, or too many warnings |
| 2 | a usage or project error (no `dbt_project.yml`, a bad `cogitator.yml`, a path that does not exist) |

In GitHub Actions:

```yaml
- run: cogitator lint --format github
```

## `cogitator format`

```sh
cogitator format                 # rewrite every model in place
cogitator format --check         # list the files that would change; exit 1 if any
cogitator format --check --diff  # and show the unified diff
cogitator format models/marts --indent 2
```

It runs `textDocument/formatting` (sqlformat over Jinja-segmented SQL)
on each model of the root project, or on the given files and
directories. A file the formatter declines — inline Jinja statements, a
sentinel miscount, any doubt — is reported on stderr as `skipped` and
never changed. Exit 0, or 1 with `--check` when a file would change.
