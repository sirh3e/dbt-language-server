# Previewing the rows of a CTE

Every CTE of a model carries a **Preview rows** code lens, and so does its
final select. It shows what the model holds at that point: the model runs up
to that CTE and no further, and the first rows come back as a table.

## What runs

1. **The SQL.** When dbt has compiled the model (`target/compiled/…`) and the
   compiled file is no older than the model and the buffer has no unsaved
   edits, that compiled SQL is used (`source: compiled`). Otherwise cogitator
   renders the model's Jinja itself, with the renderer of `dbt.expandJinja`
   (`source: rendered`): `ref()` and `source()` become the active target's
   relations, `var()` the project's values. Models that call adapter macros or
   package macros cogitator cannot stub need a `dbt compile` first.
2. **The cut.** The SQL is parsed with sqlparser. The WITH clause is kept up to
   the chosen CTE and the final select is replaced by
   `select * from <cte> limit N`; for the final select the whole query is
   wrapped as `select * from (<query>) limit N`. Anything that is not exactly
   one SELECT — DML, DDL, `SELECT … INTO`, `FOR UPDATE` — is refused, so a
   preview can only read. dbt's hooks never run: `dbt show --inline` runs the
   query alone.
3. **The run.** `dbt show --inline "<sql>" --limit N --output json` in the
   project, with the active target, profiles directory and profile and the
   `vars:` overrides, as a job of the registry: it reports through
   `$/progress` (the title ends in `[job <token>]`) and `dbt.cancel {token}`
   stops it. cogitator asks for one row more than it shows, so `truncated`
   says whether there were more.

The answer is `{model, cte, columns, rows, truncated, limit, sql, source, uri,
table}`, and the table is also written as markdown to
`target/.cogitator/preview/<model>.<cte>.md` (`<model>.final.md` for the final
select) and opened through `window/showDocument`, so editors that only show
documents see it too.

## Rows

```yaml
# cogitator.yml
preview:
  rows: 20        # when the call names no limit
  max_rows: 1000  # larger limits are capped
```

## Editors

| Editor | How |
|---|---|
| Any LSP client | the **Preview rows** lenses |
| Neovim | `:DbtPreviewCte [limit]` — the CTE under the cursor, in a split (`q` closes) |
| Emacs | `M-x cogitator-preview-cte` (a prefix argument sets the rows) — `*cogitator-preview*` |
| VS Code | `dbt: Preview rows of the CTE under the cursor` — the markdown table beside the model |

Past every CTE the cursor names the final select.

## Not (yet)

Previews always go through dbt. Running the query in-process against the
duckdb file is not offered: the only DuckDB connection the server has lives
behind the `duckdb-parser` build feature (a bundled C++ build), and dbt holds
the warehouse file while it runs, so `dbt show` is the one path that works
for every adapter.
