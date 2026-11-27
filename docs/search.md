# Structural search over the models' SQL (`dbt.search`)

`dbt.astGrep` searches the tree-sitter-dbt tree, in which the SQL between
the Jinja tags is opaque text: it can find every `{{ ref('x') }}`, but not
"the models that join `stg_orders` on `customer_id`". `dbt.search` asks
that kind of question of each model's **HIR** — the front end's bound,
name-resolved reading of the template (the same one column lineage uses).

## The query language

Clauses, combined with `and`, `or`, `not` and parentheses; a model matches
when the whole query holds in it. Names and the words `and`, `or`, `not`
compare case-insensitively.

| Clause | Finds |
|---|---|
| `joins <rel>` | a `JOIN` whose either side is `<rel>` (the place: the joined relation) |
| `joins <rel> on <column>` | the same, when the `ON` condition reads `<column>` or `USING` names it |
| `select-star` | every `*` / `t.*` |
| `select-star from <rel>` | a `t.*` over `<rel>`, or a bare `*` whose `SELECT` reads `<rel>` alone |
| `reads <rel>` | every `ref`/`source` of `<rel>` in a `FROM` or `JOIN` |
| `uses column <rel>.<column>` | every reference to `<column>` bound to `<rel>` |

A `<rel>` is `model` (any model), `source` (any source), a model's name
(or `model:<name>`), or `source:<source>` / `source:<source>.<table>`.

```
joins stg_orders on customer_id
select-star from source
reads source:shop.orders and select-star
uses column stg_payments.amount
select-star from source or select-star from stg_customers
reads stg_orders and not reads stg_customers
(joins stg_orders or joins stg_payments) and not select-star
```

`not` binds tightest, then `and`, then `or`, as in SQL, so `reads a or
reads b and reads c` is `reads a or (reads b and reads c)`; parentheses
group, and may touch the words they enclose (`(reads a or reads b)`).

Which places are listed: those of the clauses that made the model match —
every clause of an `and`, only the `or` branches that held, and nothing
under a `not` (a negated clause has no place: it is about what the model
does *not* do). A model that matches only through `not` — e.g. every model
that `not reads source` — is listed once at the top of its file, with the
whole query as its `env.clause`.

Relations are what the template refers to: a CTE is not a relation of the
project, so `joins stg_orders` finds a join on the `ref`, not on a CTE that
selects from it (`reads stg_orders` finds that CTE's `ref`).

The language is ours and tiny: it is read word by word, not by a SQL or
Jinja lexer (which only the front end uses on the models themselves).

## What it answers

`{query, searched, models, matches, notSearched, truncated}`. `matches`
have `dbt.astGrep`'s shape — `{uri, range, text, env, replacement}`, with
`env.clause` and `env.model` — so every editor reuses its ast-grep lists
(quickfix in Neovim, a grep buffer in Emacs, a quick pick in VS Code). At
most 500 matches.

**Nothing is dropped silently.** A model whose HIR cannot be built — Jinja
control blocks (`{% if %}`, `{% for %}`), whose SQL depends on how they
render, or SQL that does not parse into a full query — is listed under
`notSearched` with why, and counted apart from `searched`.

## Editors

- Neovim: `:DbtSearch joins stg_orders on customer_id`
- Emacs: `M-x cogitator-search`
- VS Code: "dbt: Structural search over the models' SQL"
- Zed: "dbt…" → Model › Find in the project: fixed queries about the open
  model (who reads, joins or selects `*` from it, which models use one of
  its columns, …), answered as a document of matches — a button carries no
  free query

## What it does not do

- No compiled SQL: control blocks make a model not searched rather than
  searched in one rendering.
- No rewriting (use `dbt.astGrep` for that).
