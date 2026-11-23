# Macros: dispatch and call checks

## Dispatched macros

dbt picks a macro's implementation per adapter with
`adapter.dispatch('x', 'pkg')`. cogitator resolves it as dbt does:

- **Search order.** It looks package by package. It uses the `dispatch:`
  entry of `dbt_project.yml` whose `macro_namespace` is the namespace, if
  there is one, and otherwise the root project then the namespace.
  Without a namespace, it looks in the root project, then every installed
  package. dbt's internal namespace comes last. dbt's own macros are not
  scanned, so a name that only dbt implements resolves to nothing here.
- **Prefixes.** Within each package it tries `<adapter>__x`, then the
  adapters it builds on (redshift → postgres, databricks → spark,
  materialize → postgres), then `default__x`. Packages come first: a
  `default__x` in an earlier package wins over `postgres__x` in a later
  one.
- **The adapter.** The active adapter is the `type` of the active
  profiles target (`--target`, `dbt.setTarget`, `profiles.target` in
  `cogitator.yml`).

**Go to implementation** (`textDocument/implementation`) lists every
scanned `<adapter>__x` and `default__x`, in the root project and in the
installed packages. They come in the order dispatch would try them, and
the rest follow. You can ask from three places:

- an `adapter.dispatch('x', …)` call;
- a call of a macro whose body is such a dispatch;
- that macro's own `{% macro x(…) %}` header.

**Hover** on any of these three adds a line that names the implementation
the active adapter picks, for example
``Dispatches to `jaffle_shop.duckdb__x` for adapter `duckdb`.`` When no
scanned macro answers, the line says so. Hover is the one place that names
the pick: no lens or inlay hint does.

## Macro call checks

Every call to a macro of the project or of an installed package is held
to that macro's parameters. This covers `{{ m() }}`, calls inside `{% set
%}`, `{% if %}`, `{% for %}`, `{% do %}` and `{% call %}`, nested
arguments, and qualified calls (`pkg.m()`). The arguments are read with
minijinja's own expression parser.

| Code | Severity | When |
|---|---|---|
| `macro-arg-count` | warning | more positional arguments than parameters, or a parameter without a default that no argument fills |
| `macro-unknown-kwarg` | warning | a keyword the macro has no parameter for, with did-you-mean; the quickfix renames it |
| `macro-duplicate-kwarg` | warning | a keyword given twice, or a keyword for a parameter a positional argument already fills |
| `dispatch-no-default` | warning | `adapter.dispatch('x', …)` finds neither `default__x` nor an implementation for the active adapter in its search order |

Some calls and macros are not checked, so the checks never guess:

- **Lenient bodies.** A macro whose body reads `varargs` takes any number
  of positional arguments. One that reads `kwargs` takes any keyword.
- **Calls that are skipped.** These are never checked: calls with a
  `*args` or `**kwargs` splat, calls whose arguments do not parse, calls
  that resolve to no scanned macro, and calls of dbt's builtins.
- **Dispatches that are skipped.** `dispatch-no-default` judges only a
  namespace cogitator scans: the root project or an installed package.
  It never judges `dbt` or a package that is not installed. Without a
  namespace, it judges only a name that some scanned `<prefix>__x`
  implements, because dbt's own macros may answer the rest.

### Turning them off

These checks report what fails at compile or run time, so they are **on
by default**. That makes them the exception among the `lint:` checks,
which are style checks and are off by default. Turn them off in
`cogitator.yml`:

```yaml
lint:
  macro_calls: false   # macro-arg-count, macro-unknown-kwarg, macro-duplicate-kwarg
  dispatch: false      # dispatch-no-default
```

Removing a key brings back the default, which is on. `dbt.doctor` lists
both checks among the enabled lints.
