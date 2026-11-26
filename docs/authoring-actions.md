# Authoring actions

Three code actions for writing models faster. Every one is an edit the
editor applies (a `WorkspaceEdit`), offered at the cursor through
`textDocument/codeAction`: no command, no client code, so every LSP
editor has them. Each refuses, by offering nothing, where it cannot be
sure of the result.

## Scaffold a staging model

**Where:** on a `source('a', 'b')` or `ref('x')` in SQL, or anywhere in a
source table's entry of a properties file.

**What:** "Scaffold staging model stg_a__b" (`refactor`) creates two files:

- `stg_<source>__<table>.sql` (or `stg_<model>.sql` for a ref), in the shape
  of dbt's style guide:

  ```sql
  with

  source as (

      select * from {{ source('bakery', 'bakes') }}

  ),

  renamed as (

      select
          cast(id as integer) as bake_id,
          cast("bakedAt" as timestamp) as baked_at,
          "Flour Kind" as flour_kind

      from source

  )

  select * from renamed
  ```

- `stg_<…>.yml` beside it, with the model's description taken from the
  source table's (or model's), every column's description from the column
  it is read from, written as it is written there (a `{{ doc('x') }}` stays
  the call), and `unique` + `not_null` on the key.

**Where the files go:** for a source, beside the properties file that
declares it (where a source is declared is where its staging lives); for
a ref, beside the file the ref is in.

**Columns**, from the first that knows any:

| Relation | 1st | 2nd | 3rd |
|---|---|---|---|
| source | `target/catalog.json`'s columns, in the warehouse's order | the source's YAML columns | — |
| model or seed | the inferred output types (the project schema job) | `target/catalog.json` | the model's YAML columns |

A YAML `data_type:` wins over the catalog's. A column is cast only where
its type is known, in the dialect's spelling (`column_types::render` for
inferred types; the catalog's as the warehouse wrote it, lower-cased).
Without any column the body is `select *` with a comment saying so, and
the title ends "(columns unknown: select *)".

**Renaming** is deterministic:

1. snake_case: an upper-case letter after a lower-case letter or a digit
   starts a word, any run of other characters is one `_`, all lower case;
   a leading digit gets `col_`;
2. a column named `id` becomes `<object>_id`, the object being the
   relation's name without a `raw_` prefix, made singular by `ies` → `y`,
   dropping the `es` of `ches`/`shes`/`sses`/`xes`, or dropping one
   trailing `s` (not after `s` or `u`). Irregular plurals come out
   regular (`loaves` → `loave`): rename them by hand;
3. a name another column already took keeps the column's own name, and
   `_2`, `_3`, … after it where that is taken too.

A source name that is not a plain lower-case word is quoted (`"x"`, or
backticks on BigQuery).

**Tests:** only the key column (`<object>_id`) gets `unique` and
`not_null`. A foreign key is not unique, a timestamp has nothing to test,
and a boolean's type already limits its values; a test is a claim the
scaffold cannot back, so it writes none.

**Refused:** when a model of that name exists, or either file does.

## Extract a CTE, with its documentation

"Extract CTE 'x' to a model" (on a CTE's name) moves the CTE to `x.sql`
and reads it with `{{ ref('x') }}`. It now also takes the documentation
along: each documented column of the parent model's properties entry
that the CTE produces and the parent passes through unchanged — lineage
`direct` on both sides, through the same one upstream column — becomes a
column of a new `x` entry, with the parent's description as written. The
entry goes into the parent's properties file, right after the parent's
entry (the file the author already keeps for that folder). Nothing is
written to YAML when the parent has no entry, the CTE does not parse on
its own, or no column qualifies.

## Inline a model as a CTE

"Inline model 'x' as a CTE" (`refactor.inline`), on a `{{ ref('x') }}`:
the reverse of extraction. x's SQL becomes the first CTE of this model's
`with` (or a new `with`), the call reads `x`, x's file is deleted (a
`DeleteFile` resource operation, with `ignoreIfNotExists`) and its
properties entry removed.

Refused unless x is small and plain:

- x is a model of this project (not a seed, snapshot or package's model);
- nothing but this model depends on x (models, singular tests, analyses,
  exposures);
- x has at most 60 lines (`INLINE_MAX_LINES`), no Jinja statement
  (`{% … %}`) and no `config(` call;
- x's entry has no tests, no versions, no contract;
- this file has no CTE named `x`, and does not start `with recursive`.

x's own `with` stays inside the new CTE's parentheses rather than being
merged. Removing x's entry can leave a properties file with an empty
`models:`, which is left for the author.

The client must support the `delete` resource operation
(`workspace.workspaceEdit.resourceOperations`); Neovim's plugin
advertises it.

## Inherit a description from upstream

"Inherit description of 'col' from <upstream>" (`quickfix`):

- **In a properties file**, on an undocumented column of a model's entry
  (up to five per entry).
- **In SQL**, on a select item's output name (the word right before a
  comma or `from`): the description is written under the column's entry,
  as a new column of the model's entry, or in a new `<model>.yml` when the
  model has none.

Both follow the schema-YAML sync's walk (docs/yaml-sync.md): only a
column passed through unchanged (lineage `direct`) inherits — a computed
or renamed column does not — through any number of undocumented models,
to the nearest documented model or source column. The title names where
the description comes from and the models walked through
(`… from stg_x.id (via a, b)`).

**doc() propagation:** a description is copied as written, never
expanded: an upstream `{{ doc('x') }}` is written as the same
`{{ doc('x') }}`, so the doc block stays the one place the words live.
A description written as a `|` or `>` block is not read back from the
file: it is copied as the project reads it, a doc() call expanded.
