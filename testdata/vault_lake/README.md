# vault_lake

A small, made-up dbt project in the shape of a real Data Vault lake, for the
editor suites (`editors/nvim/tests/steps/06_vault_lake.lua`): what jaffle_shop
does not have and real projects do.

- every model is a chain of CTEs, and a CTE is filtered by a subquery of another
  (`where x in (select x from touched)`);
- columns are documented by doc blocks of Markdown over several lines, tables
  included (`models/_lake__docs.md`);
- singular tests (`tests/assert_*.sql`) depend on the models, so a model's
  downstream holds tests beside models;
- a project `generate_schema_name` puts each folder in its own schema, which only
  `target/manifest.json` knows (`"lake"."dim"."dim_items"`);
- properties files per folder, named `_<layer>__models.yml`.

Nothing here runs against a warehouse; `target/manifest.json` is written by hand
with only the fields the server reads.
