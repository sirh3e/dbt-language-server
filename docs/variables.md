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
- [Renaming a var](#renaming-a-var)
- [Unused vars](#unused-vars)
- [Environment variables](#environment-variables)
- [Jinja preview](#jinja-preview)
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
| rename | on a `var('x')` or a `vars:` key: the calls and the keys at once ([below](#renaming-a-var)) |
| diagnostics | `unknown-var` for a `var('x')` nothing defines and no default covers; `unused-var` (a hint) for a key nothing reads |
| quick fix | "Define var 'x' in dbt_project.yml". In a package's file it adds the var to the root project's `vars: {<package>: …}` block. "Remove unused var 'x'" on an unused key |
| YAML colouring | `var('x')` in a value is `variable.dbt.var`, or `variable.dbt.var.unresolved` when nothing defines it ([highlighting.md](highlighting.md)) |

Inlay hints have to be turned on in the editor:
- **Neovim:** on by default (`inlay_hints = true` in `setup()`).
- **VS Code:** `editor.inlayHints.enabled`.
- **Emacs:** eglot's `eglot-inlay-hints-mode`, or lsp-mode's `lsp-inlay-hint-enable`.

## Renaming a var

Rename (F2 in most editors) works on a `var('x')` call and on a `vars:`
key — in the root `dbt_project.yml` (top level or a scope block), in an
installed package's `dbt_project.yml`, or in cogitator.yml's `vars:`. One
edit renames the calls in every SQL and YAML file of the project and its
packages (and in `dbt_project.yml` itself) together with the keys.

The rename follows the **definition** the starting point resolves to:

- **Calls** are renamed in every scope (the root project, each installed
  package) whose `var('x')` resolves to that definition, and nowhere else.
- **A key of the same name** moves along only when every scope that can see
  it resolves to that definition. A key nobody else reads is safe to move;
  one another scope still reads is left alone.
- **From a key** that decides some scope, that key is the definition. From
  a key that is shadowed everywhere (an override wins), the rename follows
  the definition that shadows it.

Examples, with a package `utils`:

| Starting point | Renamed |
|---|---|
| `var('x')` in a root model, decided by `vars: {shop: {x: …}}` | the root's calls and `shop.x`; the top-level `x`, which `utils` reads, stays |
| the top-level key `x`, read by root and `utils` | both scopes' calls and the key |
| `vars: {utils: {x: …}}` | `utils`' calls, that key, and `utils`' own `x` (only `utils` sees it) |
| an override `x` in cogitator.yml | every call and every key named `x` (the override wins everywhere) |

A rename is refused when the new name is not a plain identifier, or when a
renamed scope already sees a var of that name (the two would merge). An
override from `initializationOptions` has no file, so it cannot be renamed.

## Unused vars

On the root `dbt_project.yml`, a `vars:` key that nothing reads is greyed out
(an `unused-var` hint, tagged unnecessary), with a quick fix that deletes the
entry and its nested lines:

- **A top-level key** is read when any file of the project or of an
  installed package reads it with `var()`.
- **`vars: {<project>: {x: …}}`** is read when the project's own files read
  `x`; **`vars: {<package>: {x: …}}`** when that package's files do.
- **What counts as reading** is a `var('x')` with a literal name in the SQL
  and YAML of the resource paths and in `dbt_project.yml` itself. A
  `var(name)` with a computed name cannot be followed.
- **Not checked:** a package's own `dbt_project.yml` (installed code),
  cogitator.yml overrides (they also feed the dbt commands the server runs),
  and a top-level mapping named like a declared but not installed package,
  which is that package's block once `dbt deps` has run.

The reads are scanned with the project, so the hints follow saved files.

## Environment variables

`env_var('NAME')` and `env_var('NAME', default)`, in SQL, properties YAML,
`dbt_project.yml` and `profiles.yml`, get the same treatment as `var()`:

| Where | What you get |
|---|---|
| after `env_var('NAME')` | `= 'value'`, `= 'dev' (default)` or `= ? unset` |
| hover | the value and where it comes from; that `.env` has it but dbt does not read `.env` |
| go to definition | the line in `<root>/.env` that sets it, else its use in the active `profiles.yml`, else the default argument |
| diagnostics | `env-var-unset` (a warning) for an `env_var('NAME')` without a default whose variable is unset — dbt fails to compile it |

- **What counts as set** is the server's own process environment: dbt, run
  by the server or from the shell that started the editor, inherits it.
- **`.env` is not read by dbt.** A value only in `<root>/.env` is shown in
  the tooltip and hover and is a definition target, but the hint says unset
  and the warning stays: export it (direnv, `set -a; . ./.env`) before
  starting the editor.
- **Secrets are masked.** A name starting with `DBT_ENV_SECRET_`, or
  containing `PASSWORD`, `TOKEN`, `SECRET` or `KEY`, shows `••••••` in hints,
  hovers and previews.
- **Refresh.** The environment, `.env` and `profiles.yml` are read when the
  project context is built; a later change shows after the next refresh.

## Jinja preview

Hovering inside a `{{ … }}` of a SQL file, where nothing else has a hover
(the braces, `ref`, an operator — not a model name or a var, which keep
their own), shows what the expression renders to, with the renderer behind `dbt.expandJinja`: `ref()`/`source()` as the
active target's relations, `var()`/`env_var()` as above, the project's
macros, and stubs for `config()`, `is_incremental()` and `target`.

- **Bounded.** Each render has a fixed instruction budget; one that runs out,
  or that the stubs cannot render (an adapter call, an unknown macro), says
  so instead of a value. A hover shows at most 2,000 characters.
- **As inlay hints.** `inlay.jinja: true` adds `⇒ rendered` after short
  single-line expressions (up to 40 characters, 64 per request). It is off
  by default: every hint is a render.

## Turning the hints off

Each kind of hint can be switched in cogitator.yml; relation and parameter
hints stay:

```yaml
inlay:
  vars: false       # the value after var() (default true)
  env_vars: false   # the value after env_var() (default true)
  jinja: true       # what {{ … }} renders to (default false)
```
