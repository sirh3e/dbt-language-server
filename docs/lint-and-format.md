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
cogitator format --config ci/cogitator-format.toml
```

It runs `textDocument/formatting` on each model of the root project, or
on the given files and directories. A file the formatter declines is
reported on stderr as `skipped`, with why, and never changed. Exit 0, or
1 with `--check` when a file would change.

### How it formats

The formatter is the crate `cogitator-format`
([crates/cogitator-format](../crates/cogitator-format)), which the server
embeds and which is also a program of its own (below).

- **SQL** is laid out on the parse tree of
  [sqlfluff](https://github.com/sqlfluff/sqlfluff)'s Rust parser
  (sqlfluffrs, vendored in [vendor/sqlfluffrs](../vendor/sqlfluffrs)), in
  any of sqlfluff's 28 dialects. Lines break where sqlfluff's own reflow
  breaks them: every clause (`from`, `where`, `group by`, a `join`, a
  `union`) starts a line; a bracket, a `select`'s targets, a `case`, an
  expression's operators and a join's condition break only when they do
  not fit `max_line_length`; a bracket closes at the indent of the line
  that opened it.
- **Jinja** is read with minijinja's own tokenizer. Each tag is laid out
  again from its tokens (`{{ref( 'x' )}}` → `{{ ref('x') }}`, `{%set
  x=-1%}` → `{% set x = -1 %}`, `a|b` → `a | b`); a tag written over
  several lines is kept as it is. The lines inside `{% if %}`, `{% for %}`,
  `{% macro %}`, `{% set x %}` … go one level deeper, `{% else %}` back.
  A tag written inside a line stays inside it; one on a line of its own
  keeps its line.
- **Nothing changes what a model means.** Only whitespace moves, and the
  case of keywords and names where a rule says so: the output is lexed
  again and must hold the same tokens in the same order, every tag must
  come back exactly once, or the model is left as it is.

### The rules

Every rule has a default; the names follow sqlfluff's `.sqlfluff`.

| Key | Default | sqlfluff | What |
|---|---|---|---|
| `dialect` | the dbt adapter's | `dialect` | `duckdb`, `snowflake`, `bigquery`, `postgres`, `sparksql`, `tsql`, … |
| `max_line_length` | `80` | LT05 | the longest line the layout aims for |
| `indent_unit` | `space` | LT02 | `space` or `tab` |
| `tab_space_size` | `4` | LT02 | spaces per indent |
| `max_blank_lines` | `1` | | blank lines kept where the source had more |
| `trailing_newline` | `true` | LT12 | end the file with exactly one newline |
| `case.keywords` | `lower` | CP01 | `upper`, `lower`, `capitalise` or `preserve` |
| `case.identifiers` | `preserve` | CP02 | unquoted names; quoted ones never change |
| `case.functions` | `lower` | CP03 | |
| `case.literals` | `lower` | CP04 | `null`, `true`, `false` |
| `case.types` | `lower` | CP05 | |
| `layout.commas` | `trailing` | LT04 | `trailing` or `leading` |
| `layout.select_targets` | `one_per_line` | LT09 | more than one target: a line each; `fit`: while they fit |
| `layout.indented_joins` | `false` | `indented_joins` | `join` one level under `from` |
| `layout.blank_line_after_cte` | `true` | LT08 | a blank line after each CTE |
| `jinja.delimiter_spacing` | `true` | | one space inside `{{ }}` and `{% %}` |
| `jinja.normalize_tags` | `true` | | a tag's inside laid out from its tokens |
| `jinja.filter_spacing` | `true` | | spaces around a filter's `\|` |
| `jinja.indent_blocks` | `true` | `template_blocks_indent` | the lines inside a block one level deeper |

They are read from, the first that has them:

1. `format:` in `cogitator.yml` (the server, hot: an edit applies to the
   next format):

   ```yaml
   format:
     dialect: duckdb
     max_line_length: 100
     case:
       keywords: upper
     layout:
       commas: leading
   ```

2. the nearest `cogitator-format.toml` (or `.cogitator-format.toml`) at or
   above the file — what the standalone program reads too:

   ```toml
   dialect = "duckdb"
   max_line_length = 100

   [case]
   keywords = "upper"

   [layout]
   commas = "leading"
   ```

3. the defaults, with the editor's indent (`tabSize`, `insertSpaces`).

Where the rules name no dialect, the dbt adapter of the active target
names it (`spark` and `glue` → `sparksql`, `sqlserver` → `tsql`, an
adapter sqlfluff has no dialect for → `ansi`). A key the formatter does
not know is refused by name, in either file.

### When it leaves a model as it is

- **Jinja minijinja cannot read** (an unclosed `{{`).
- **SQL the dialect's grammar cannot parse.** Jinja is masked for the
  parser, and the masks are tried in turn, each looser than the last, so a
  model is masked the same way every run:
  1. a `{{ … }}` as a name, and one alone on its line before any SQL
     (`config()`, `set` calls) as a comment, as any other tag is;
  2. every `{{ … }}` as a name;
  3. every `{{ … }}` alone on its line as a comment;
  4. a `{{ … }}` alone where a statement may stand (a CTE's body, a
     subquery, a `union`'s branch) as one, `select <name>`: a call that
     expands to a whole query;
  5. the SQL between tags on their own lines parsed piece by piece, which
     `{% if %} select 1 {% else %} select 2 {% endif %}` needs. Inside a
     block (`{% macro %}`, `{% if %}` …) a piece that is still no SQL — a
     macro's fragment, a regular expression — is kept byte for byte, as
     sqlfluff leaves unparsable sections; the model's own SQL must parse.

  A call in a place no stand-in fills (the tail of a CTE list, `with {{
  ctes() }}, final as (…)`) declines the model.
- **A layout that would change a token** — a formatter bug; please report
  it.

`cogitator-format --explain <file>` says, for each way of masking the
Jinja, what it came to.

### The formatter on its own

`cogitator-format` is the same formatter without the server, for a
pre-commit hook or a project without cogitator:

```sh
cogitator-format models/                       # in place
cogitator-format --check --diff models/        # exit 1 if a file would change
cogitator-format --check --strict models/      # a declined file fails too
cogitator-format - --stdin-filename models/m.sql < m.sql   # stdin to stdout
cogitator-format --print-config models/m.sql   # the rules a file is formatted by
cogitator-format --explain models/m.sql        # why a file was declined
```

It reads the nearest `cogitator-format.toml` (or `--config`), and
`--dialect` sets the dialect over it; with neither it formats as `ansi`.
