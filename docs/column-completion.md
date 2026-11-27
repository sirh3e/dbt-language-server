# Column completion with types and descriptions

Column completion shows each column's data type and relation beside its
name, and a documentation panel with its description, the way rust-analyzer
or TypeScript show signatures:

```
customer_id      bigint          stg_customers
first_order_at   timestamp       stg_orders
order_date       date            o (CTE)
```

```
`customer_id` · `bigint` (YAML data_type)

Column of stg_customers

The customer's id in the shop.

*Description from stg_orders.customer_id*
```

The column hover shows the same text: both render one fact per column
(`features/completion/column_info.rs`).

## Where the type and the description come from

- **Type**, first that knows: the YAML `data_type`, `target/catalog.json`,
  the schema job's inferred type (marked "inferred, not proven" when it rests
  on an unenforced YAML type).
- **Description**: the YAML's, `{{ doc('…') }}` resolved. An undocumented
  model column read straight from one upstream column takes that column's
  description; a CTE column takes the type and description of the one
  relation the document reads that documents a column of that name. Both say
  where it came from.
- Model columns the YAML does not list but the SQL produces are offered too,
  with their inferred type.

The list itself reads only caches (the published schema snapshot, the loaded
artifacts); the documentation of a model's or source's column is filled on
`completionItem/resolve`, so a long select list stays cheap. A CTE's columns
come documented.

## Other items

Models show their materialization (from `target/manifest.json`, else
`model`/`seed`/`snapshot`), sources `source`, macros their parameters and
package, dialect functions their signature.

## Clients

`labelDetails` is sent to clients that advertise
`completionItem.labelDetailsSupport`; others get `"<type> · <relation>"` in
`detail`. Markdown documentation needs `documentationFormat: ["markdown"]`,
else it is plain text. Neovim's built-in completion and blink.cmp show
`labelDetails` as they are; for nvim-cmp, `require("cogitator.completion").format()`
puts them in the menu column (`:h cogitator-nvim-cmp`).
