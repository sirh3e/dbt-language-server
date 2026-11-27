-- The column-lineage dialog's pure core: panes, walking, filtering, rows,
-- and where a column is written — no server, no windows.

return function(ctx)
  local H = ctx.H
  local model = require("cogitator.column_dialog.model")
  local locate = require("cogitator.column_dialog.locate")
  local rows = require("cogitator.column_dialog.rows")

  local function col(m, c, kind)
    return {
      id = ("%s:%s::%s"):format(kind or "model", m, c),
      name = m .. "." .. c,
      kind = kind or "model",
      model = (kind or "model") == "model" and m or nil,
      column = c,
    }
  end
  local data = {
    nodes = {
      col("customers", "customer_id"),
      col("customers", "first_name"),
      col("customers", "lifetime_value"),
      col("stg_customers", "customer_id"),
      col("stg_orders", "customer_id"),
      col("stg_payments", "amount"),
      col("raw.customers", "id", "source"),
      col("orders", "customer_id"),
    },
    edges = {
      { from = "model:stg_customers::customer_id", to = "model:customers::customer_id", kind = "join" },
      { from = "model:stg_orders::customer_id", to = "model:customers::customer_id" },
      { from = "model:stg_payments::amount", to = "model:customers::lifetime_value" },
      { from = "source:raw.customers::id", to = "model:stg_customers::customer_id" },
      { from = "model:customers::customer_id", to = "model:orders::customer_id" },
      { from = "model:nowhere::x", to = "model:orders::customer_id" },
    },
  }
  local ix = model.index(data)
  H.check(#ix.parents["model:orders::customer_id"] == 1, "column_dialog: edges to unknown nodes are dropped")
  H.check(
    model.edge_kind({ from = "model:stg_orders::customer_id", to = "model:customers::customer_id" }, ix.nodes)
        == "direct"
      and model.edge_kind(
        { from = "model:stg_payments::amount", to = "model:customers::lifetime_value" },
        ix.nodes
      ) == "derived"
      and ix.parents["model:customers::customer_id"][1].kind == "join",
    "column_dialog: an edge's kind is the server's, else direct for the same name, else derived"
  )

  local nav = model.start("customers", "model:customers::customer_id")
  local p = rows.panes(ix, nav)
  local ids = function(list)
    return vim.tbl_map(function(r)
      return r.id
    end, list)
  end
  H.check(
    vim.deep_equal(ids(p.center), {
      "model:customers::customer_id",
      "model:customers::first_name",
      "model:customers::lifetime_value",
    })
      and vim.deep_equal(ids(p.left), { "model:stg_customers::customer_id", "model:stg_orders::customer_id" })
      and vim.deep_equal(ids(p.right), { "model:orders::customer_id" })
      and p.current == "model:customers::customer_id",
    "column_dialog: the model's columns in the middle, the cursor's feeders left and readers right",
    vim.inspect(p)
  )
  H.check(
    p.left[1].text:find("⋈", 1, true) ~= nil and p.left[2].text:find("=", 1, true) ~= nil,
    "column_dialog: a side row shows how its edge carries the column (⋈ join, = direct)",
    vim.inspect({ p.left[1].text, p.left[2].text })
  )
  H.check(
    rows.row(ix, "model:stg_customers::customer_id").text:find("⋈", 1, true) ~= nil
      and rows.row(ix, "model:stg_customers::customer_id").text:find("↑1 ↓1", 1, true) ~= nil,
    "column_dialog: a row marks a column that feeds a join and counts its edges",
    rows.row(ix, "model:stg_customers::customer_id").text
  )
  local fit = rows.row(ix, "model:stg_customers::customer_id", nil, 40)
  H.check(
    vim.fn.strdisplaywidth(fit.text) == 40 and vim.endswith(fit.text, "⋈ ↑1 ↓1"),
    "column_dialog: a row fills its pane, the tags right-aligned",
    fit.text
  )
  fit = rows.row(ix, "model:stg_customers::customer_id", nil, 24)
  H.check(
    vim.fn.strdisplaywidth(fit.text) == 24 and fit.text:find("…", 1, true) and vim.endswith(fit.text, "⋈"),
    "column_dialog: a narrow pane drops the counts, then cuts the name",
    fit.text
  )
  local r = rows.row(ix, "source:raw.customers::id")
  H.check(
    r.text:find("◆", 1, true) and r.marks[1].hl == "DbtDagSourceFg",
    "column_dialog: a source column has its icon and colour",
    vim.inspect(r)
  )

  -- moving and walking
  local moved = model.move(ix, nav, 1)
  H.check(
    model.current(ix, moved) == "model:customers::first_name",
    "column_dialog: j moves to the next column"
  )
  H.check(
    model.current(ix, model.move(ix, moved, 10)) == "model:customers::lifetime_value",
    "column_dialog: moves clamp"
  )
  local up = assert(model.walk(ix, nav, "left"))
  H.check(
    vim.deep_equal(
      model.visible(ix, up),
      { "model:stg_customers::customer_id", "model:stg_orders::customer_id" }
    ) and model.current(ix, up) == "model:stg_customers::customer_id",
    "column_dialog: h makes the feeders the list",
    vim.inspect(up)
  )
  local up2 = assert(model.walk(ix, up, "left"))
  H.check(
    model.current(ix, up2) == "source:raw.customers::id",
    "column_dialog: h again walks on to the source"
  )
  H.check(model.walk(ix, up2, "left") == nil, "column_dialog: nothing feeds a source column: no walk")
  local back = assert(model.walk(ix, assert(model.walk(ix, up2, "right")), "right"))
  H.check(
    back.list.model == "customers"
      and model.current(ix, back) == "model:customers::customer_id"
      and #back.trail == 0,
    "column_dialog: l back the way one came lands on the column one came from",
    vim.inspect(back)
  )
  local down = assert(model.walk(ix, nav, "right"))
  H.check(
    model.current(ix, down) == "model:orders::customer_id"
      and rows.title(ix, down) == "reads customers.customer_id · 1",
    "column_dialog: l makes the readers the list",
    rows.title(ix, down)
  )

  -- filtering, fzf syntax
  local f = model.filtered(nav, "life")
  H.check(
    vim.deep_equal(model.visible(ix, f), { "model:customers::lifetime_value" })
      and model.current(ix, f) == "model:customers::lifetime_value"
      and rows.title(ix, f) == "columns of customers · 1/3  /life",
    "column_dialog: / filters the list and keeps the cursor on a visible row",
    rows.title(ix, f)
  )
  H.check(#model.visible(ix, model.filtered(nav, "!'first")) == 2, "column_dialog: !'not excludes")
  H.check(
    #model.visible(ix, model.filtered(nav, "^customers.f | value$")) == 2,
    "column_dialog: ^prefix | suffix$"
  )
  H.check(#model.visible(ix, model.filtered(nav, "zzz")) == 0, "column_dialog: a filter can hide everything")
  H.check(
    model.current(ix, model.filtered(nav, "")) == "model:customers::customer_id",
    "column_dialog: clearing the filter keeps the cursor"
  )
  -- the list follows a graph that grows while the walk runs
  local grown = model.index({ nodes = { col("customers", "customer_id") }, edges = {} })
  H.check(
    #model.visible(grown, nav) == 1 and #model.visible(ix, nav) == 3,
    "column_dialog: the list is read from the graph"
  )

  -- where a column is written
  local sql = {
    "with orders as (select * from {{ ref('stg_orders') }}),",
    "-- customer_id is the key",
    "final as (",
    "  select",
    "    customers.customer_id,",
    "    sum(amount) as lifetime_value,",
    "    first_name",
    "  from customers",
    ")",
    "select * from final",
  }
  local spot = locate.column(sql, "customer_id")
  H.check(
    spot and spot.line == 5 and sql[5]:sub(spot.col + 1, spot.stop) == "customer_id",
    "locate: a select-list item",
    vim.inspect(spot)
  )
  spot = locate.column(sql, "lifetime_value")
  H.check(spot and spot.line == 6 and spot.col == 19, "locate: an alias wins", vim.inspect(spot))
  spot = locate.column(sql, "first_name")
  H.check(spot and spot.line == 7, "locate: a bare item", vim.inspect(spot))
  H.check(locate.column(sql, "nope") == nil, "locate: a column the file does not name")
  local yaml = {
    "sources:",
    "  - name: raw",
    "    tables:",
    "      - name: customers",
    "        columns:",
    "          - name: id",
  }
  spot = locate.column(yaml, "id", true)
  H.check(
    spot and spot.line == 6 and yaml[6]:sub(spot.col + 1, spot.stop) == "id",
    "locate: a YAML column entry",
    vim.inspect(spot)
  )
  spot = locate.of_range({ start = { line = 3, character = 4 }, ["end"] = { line = 3, character = 15 } })
  H.check(
    spot and spot.line == 4 and spot.col == 4 and spot.stop == 15,
    "locate: a server range",
    vim.inspect(spot)
  )
end
