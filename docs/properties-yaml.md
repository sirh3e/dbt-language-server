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
