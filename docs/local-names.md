# Names local to a file

Rename (F2 in most editors), find references and document highlights work
on names that live in one file, as well as on models, macros and vars:

| Name | Where it is bound | Renamed |
|---|---|---|
| a Jinja variable | `{% set x = … %}`, `{% set x %}…{% endset %}` | the `set`s and every use that reads it |
| a loop variable | `{% for x in … %}`, `{% for k, v in … %}` | the target and the uses inside the loop body |
| a macro parameter | `{% macro m(a, b=1) %}` | the header, the uses in the body, and every `m(b=…)` keyword argument project-wide |
| a `call` block's argument | `{% call(row) m() %}` | the argument and its uses in the block |
| a CTE | `with x as (…)` | the definition, every `from x` / `join x`, and every `x.col` |
| a table alias | `from orders o`, `join … as o` | the alias and every `o.col` in that query |

Highlights mark a binding as a write and a use as a read.

## Go to definition, and column aliases

Go to definition on a CTE, a table alias or a column alias jumps to where
it is declared, in the scope the SQL binder resolved it in: `o` in
`o.amount` to `from orders o` (a subquery's own `o` to its own alias), a
CTE named in `from` or `join` to its `name as (`, and a column to the
`… as col` of the select list that makes it:

| Read as | Declared at |
|---|---|
| `order by n`, `group by n`, `qualify n = 1` | `… as n` of the same select |
| `c.col` or `col` from a CTE or derived table | `… as col` in its select list (its first `select` for a `union`) |
| `c.col` of `with c (col, …) as` | the name in the CTE's column list |

References and highlights list every use of a column alias in the file.
Two exceptions defer to the project-wide column features: references on a
column of the model's own output (they follow it into every consumer and
its YAML entry), and definition on the alias itself (which goes on to the
column's YAML entry). `ref()`, `source()`, macros and vars keep their own
definitions. Column aliases are never renamed here; renaming a model's
column is its own feature.

Over a table whose columns are unknown, `group by n`, `having n` and
`qualify n` read the select alias `n` when the select has one; `where n`
reads the table.

## How Jinja scopes a name

The rename follows Jinja's own scoping, read off minijinja's parser:

- A `for` body is a scope of its own: its loop variables, and a `set`
  inside it, do not leak out. `{% else %}` of a loop is outside it.
- A macro body is a scope holding its parameters; defaults are evaluated
  outside it. The same holds for the arguments of a `{% call(…) %}` block.
- `if` makes no scope. A use reads the innermost scope that bound the name
  *before* it, so `{% set x = x + 1 %}` reads the outer `x`, and two
  `set`s of one name in one scope are one variable. A name used before any
  `set` of it is someone else's (a context variable) and is left alone.

An inner binding of the same name shadows: renaming the outer variable
leaves the inner one, and its uses, unchanged.

## Macro parameters and their call sites

A parameter's name is part of the macro's API: a call that passes it by
keyword, `full_name('a', last_name='b')`, names it. Renaming the
parameter therefore also renames those keywords in every model, test,
analysis and macro file of the project that calls the macro (calls are
resolved as dbt resolves them, bare and `pkg.macro` alike). Positional
arguments do not name the parameter and are left alone. Installed
packages' files are never edited.

A macro defined inside a model (not registered as a project macro) has
only its own file's calls.

## When a rename is refused

The rename answers an error with the reason instead of a partial edit
when:

- the new name is not an identifier (letters, digits and underscores, not
  starting with a digit), or is a Jinja keyword (`in`, `loop`, `caller`, …)
  or a SQL keyword reserved as an alias (`select`, `join`, …);
- the new name is already bound or used where the old one is visible
  (two names would merge, or one would capture the other);
- a CTE would take the name of a model, of another CTE, of an alias or of
  a table the model reads; an alias the name of another relation of its
  query;
- a call of the macro already passes the new name as a keyword;
- the parameter belongs to a macro of an installed package (`dbt deps`
  would overwrite the edit), or to a generic test (`{% test %}`), whose
  parameters its YAML `tests:` entries pass by name;
- the SQL binder cannot follow every place the CTE or alias is written:
  inside an `{% else %}` branch, or a qualified column whose name is made
  by Jinja (`o.{{ col }}`), or a model whose SQL does not parse.

## How the SQL side reads a model

CTEs and aliases come from the same binder the query plans use: Jinja
expressions are masked in place, `for` tags are blanked so the loop body
is read once, and an `if` keeps its first branch. Every place the file
writes the name as a relation (after `from`/`join`) or as a qualifier
(`name.`) must then be one the binder resolved; otherwise the rename is
refused (references and highlights still show what was resolved).
