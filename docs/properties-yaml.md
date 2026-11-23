# Properties YAML

What cogitator does in dbt properties files (`schema.yml`, `_models.yml`,
any `.yml` that is not `dbt_project.yml`, `packages.yml`,
`dependencies.yml`, `profiles.yml`, `selectors.yml` or `cogitator.yml`).

## Completion

Driven by the vendored dbt JSON schema (`schemas/dbt/processed/`,
walked by `cogitator_schemas::schema_walk`) at the cursor's path, which
follows indentation and `- ` items, so a half-typed file completes too:

| Where | Offers |
|---|---|
| a key of any mapping (`models:` items, `config:`, `columns:` items, sources' `tables:`…) | the keys the schema allows there, with its description |
| a value (`access: `, `granularity: `, `quote: `) | the schema's enum values, `true`/`false` for a boolean |
| a `- ` item of `data_tests:` / `tests:` | `not_null`, `unique`, `accepted_values`, `relationships`, the project's and packages' `{% test %}` blocks (`dbt_utils.at_least_one`) and legacy `test_*` macros; a test with arguments opens a mapping |
| the keys under a test | its arguments: the schema's for the built-ins (`values`, `quote`; `to`, `field`), the macro parameters less `model` and `column_name` for the rest, plus `arguments:` (dbt 1.10's nesting) |
| `- name: ` under a model's `columns:` (or a bare `- ` there) | the model's output columns as lineage knows them, the undeclared ones first |

## Diagnostics

`yaml-unproduced-column` (Warning, pull diagnostics): a column entry that
carries tests although the model never produces that column. Only where
lineage is certain of the output (no `*` of an unknown upstream), and not
under an enforced contract, whose `contract-drift` check covers it. Its
quickfixes rename the entry to the closest produced column, or remove it.

## Code actions

"Add N missing column(s) of 'model' to its schema YAML" is offered from
the model's SQL and from anywhere in its YAML entry: the produced columns
the entry does not declare become `- name:` items in the entry's
indentation (with `data_type:` under an enforced contract). A model with
no entry at all gets "Extract schema YAML" from its SQL instead.

## Column types

Each model's output columns are typed from its saved SQL by the project
schema job (no extra planning): casts, literals, typed functions,
aggregates (`count(*)` is a never-null integer), window functions, union
arms widened, and columns passed through from upstream models and
sources. A type is only claimed when it is inferred; unknown stays
unknown, and an integer whose width is not proven is "integer (width not
proven)", never `bigint`. A type is *proven* when every upstream column it
rests on is typed by that model's own SQL, the catalog, or an enforced
contract; one that rests on a YAML `data_type` nothing enforces is only
*declared*: shown on hover, never diagnosed. Spellings are normalised per
dialect (`int`/`integer`/`int4`, `bigint`/`int8`, `varchar`/`text`/`string`,
`numeric(p,s)`, `timestamp`/`timestamptz`).

Hovering a column's `name:` in a properties file, or an output column's
name in the model's SQL, shows the type and why
(`Type: \`bigint\` — an explicit cast`). `dbt.modelInfo` carries the same
as `columnTypes`.

`column-type-mismatch` (pull diagnostics, on the `data_type:` value): the
declared type is of another family than the proven one — a number
against text, a date against a timestamp, two explicit and different time
zones. Widths, precisions and types the lattice does not model
(`variant`, arrays, JSON against text) never disagree. A Warning, an
Error under `contract: {enforced: true}` (dbt build fails the contract).
A column the model does not produce is `contract-drift`'s or
`yaml-unproduced-column`'s, never this check's. Its quickfix is
"Set data_type to <type>" when the type has an exact spelling. In the
entry of a contracted model, "Fill data_type from the inferred types"
adds `data_type:` to every column that lacks one and whose type is proven
and spelled exactly.

`unit-test-value-type` (Hint): a `given:` or `expect:` row value (dict
format) that cannot be cast to the column's proven type — `'abc'` for an
integer, `'maybe'` for a boolean, `true` for a date. `expect` rows are
checked against the tested model, `given` rows against the model the
input `ref`s; sources, `csv` and `sql` fixtures are not checked.
