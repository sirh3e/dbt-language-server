# Syntax errors, `cogitator lint` and `cogitator format`

Three things that need no `dbt compile`, no dbt Fusion and no warehouse:

1. **Syntax errors as you type** in every `.sql` file of the project
   (models, macros, tests, analyses, snapshots).
2. **`cogitator lint`**: every diagnostic the editor would show, for every
   file of the project, from the command line — for CI.
3. **`cogitator format`**: the editor's formatter over every model, in
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

### When it says nothing

A false syntax error is worse than a missed one, so the SQL layer
**declines** whenever it cannot be sure (the confidence rule of
`specs/rust-port/07` §3.2):

- the template has control blocks (`{% if %}`, `{% for %}`, `{% call %}`…)
  other than `config` and `set`: the SQL's shape depends on them;
- it has a `{% set x %}…{% endset %}` block, whose body is not SQL;
- the error sits inside, or right after, a masked `{{ … }}` — a macro can
  render anything, not just the identifier that stands in for it;
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

- the syntax checks above;
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
