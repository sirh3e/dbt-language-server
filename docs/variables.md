# Project variables

cogitator reads `var('name')` the way dbt resolves it, and shows you the
result where you write it:

```sql
where created_at > '{{ var("since") }}'        -- = '2024-01-01'
  and region = '{{ var("region", "emea") }}'   -- = 'emea' (default)
  and tier = {{ var("tier") }}                 -- = ? undefined
```

- [Where a value comes from](#where-a-value-comes-from)
- [Overrides](#overrides)
- [In the editor](#in-the-editor)
- [Turning the hints off](#turning-the-hints-off)

## Where a value comes from

A file sees the vars of the project it belongs to. A file under an
installed package (`dbt_packages/<package>/`) sees that package's vars; every
other file sees the root project's. Within a scope dbt's precedence holds,
from lowest to highest:

| Source | Written as | Seen by |
|---|---|---|
| a package's own `dbt_project.yml` | `vars: {own: 1}` or `vars: {<package>: {own: 1}}` | that package's files |
| the root project's top level | `vars: {since: '2024-01-01'}` | every file |
| the root project's block for a scope | `vars: {<project or package>: {since: …}}` | that scope's files |
| an override | cogitator.yml `vars:`, `initializationOptions.vars` | every file |

- **Scope blocks.** A top-level key that names the root project or an
  installed package, with a mapping as its value, is a scope block, not a
  var.
- **Null values.** A `null` inside a scope block is skipped, as dbt skips it.
- **Defaults.** `var('name', default)` falls back to its default when no
  source defines the name.

## Overrides

Overrides are what `dbt run --vars '{…}'` would set. They win over every
`dbt_project.yml`. There are two places to write them:

```yaml
# cogitator.yml — hot-reloaded
vars:
  since: '2025-06-01'
  region: amer
```

```json
// initializationOptions
{ "vars": { "since": "2025-06-01" } }
```

- **Precedence.** The file wins over the init options.
- **dbt commands.** Every dbt command the server runs (run, test, compile,
  preview…, but not deps) is given the overrides as `--vars`, so what runs
  is what the editor shows.

## In the editor

| Where | What you get |
|---|---|
| after `var('x')` | an inlay hint with the value: `= 'jaffle'`, `= 7 (default)`, `= ? undefined`. `(override)` marks an override and `(<package>)` a package's value. A long value is shortened; the tooltip has all of it and the file and line that set it |
| hover | the value and where it is defined; for an undefined var, its default |
| go to definition | the key that decides the value: a line of `dbt_project.yml`, a package's `dbt_project.yml`, or `cogitator.yml` for an override. An undefined var goes to its default argument. On a `vars:` key it goes to what overrides that key |
| references | every `var('x')` in the project's and packages' SQL and YAML files that resolves to the same definition, and the definition itself. From a `vars:` key you get every use of the name |
| completion | inside `var('`, the names this file sees |
| diagnostics | `unknown-var` for a `var('x')` nothing defines and no default covers |
| quick fix | "Define var 'x' in dbt_project.yml". In a package's file it adds the var to the root project's `vars: {<package>: …}` block |
| YAML colouring | `var('x')` in a value is `variable.dbt.var`, or `variable.dbt.var.unresolved` when nothing defines it ([highlighting.md](highlighting.md)) |

Inlay hints have to be turned on in the editor:
- **Neovim:** `inlay_hints = true` in `setup()`.
- **VS Code:** `editor.inlayHints.enabled`.
- **Emacs:** eglot's `eglot-inlay-hints-mode`, or lsp-mode's `lsp-inlay-hint-enable`.

## Turning the hints off

The var hints alone can be turned off in cogitator.yml; relation and
parameter hints stay:

```yaml
inlay:
  vars: false
```
