-- The explorer's pure parts: the five panes' geometry and the model card.

return function(ctx)
  local H = ctx.H

  local render = require("cogitator.explorer.render")
  for _, size in ipairs({ { 200, 60 }, { 120, 40 }, { 80, 24 } }) do
    local g = render.geometry(size[1], size[2])
    local label = ("explorer: geometry at %dx%d"):format(size[1], size[2])
    -- each float has a one-cell border on every side
    local function right(r)
      return r.col + r.width + 2
    end
    H.check(
      g.left.row == g.center.row
        and g.center.row == g.right.row
        and right(g.left) <= g.center.col
        and right(g.center) <= g.right.col
        and g.raw.row == g.compiled.row
        and g.raw.row >= g.center.row + g.center.height + 2
        and g.raw.col == g.left.col
        and right(g.raw) <= g.compiled.col
        and right(g.compiled) == right(g.right)
        and right(g.right) <= size[1]
        and g.raw.row + g.raw.height + 2 <= size[2],
      label .. ": upstream | model | downstream over dbt SQL | compiled SQL, nothing overlaps",
      vim.inspect(g)
    )
  end

  local card = require("cogitator.explorer.card")
  local node = {
    name = "customers",
    kind = "model",
    uri = "file:///p/models/customers.sql",
    tags = { "mart" },
    columns = { { name = "customer_id", description = "the key" } },
  }
  local s = {
    selected = "customers",
    node_of = { customers = node },
    marked = { customers = true },
    info = {
      model = "customers",
      uri = "file:///p/models/customers.sql",
      description = "One row per customer.",
      tags = { "mart", "nightly" },
      versions = { latestVersion = 2, versions = { { v = 1, uri = "file:///p/models/customers_v1.sql" } } },
      tests = {
        { name = "unique", column = "customer_id", uri = "file:///p/models/schema.yml", range = vim.NIL },
      },
    },
    relation = { relation = "db.main.customers" },
  }
  local view = card.view(s, 60)
  local text = table.concat(view.lines, "\n")
  H.check(
    view.lines[1] == "customers  ● marked"
      and text:find("One row per customer.", 1, true)
      and text:find("#mart #nightly", 1, true)
      and text:find("relation: db.main.customers", 1, true)
      and text:find("versions (latest 2)", 1, true)
      and text:find("• customer_id — the key", 1, true)
      and text:find("unique on customer_id", 1, true),
    "explorer: the card shows what the model is, its description, tags, relation, versions, columns and tests",
    text
  )
  local jumps = vim.tbl_count(view.jumps)
  H.check(jumps == 2, "explorer: the card's version and test lines jump", jumps)
  s.info, s.relation = nil, nil
  text = table.concat(card.view(s, 60).lines, "\n")
  H.check(text:find("· loading ·", 1, true) ~= nil, "explorer: the card says it is loading", text)
  H.check(
    card.view({ selected = nil, node_of = {}, marked = {} }).lines[1]:find("pick one", 1, true) ~= nil,
    "explorer: with no model the card says how to pick one"
  )

  -- a column documented by a doc block: Markdown over many lines, which
  -- nvim_buf_set_lines refuses as one line (a real project's every column)
  H.check(
    card.summary("Orders can be:\n\n| status | meaning |\n|---|---|\n| placed | … |\n")
      == "Orders can be: …",
    "explorer: a doc block's description shows its first paragraph on one line",
    card.summary("Orders can be:\n\n| status |")
  )
  H.check(
    card.summary("  The row's\n  key. ") == "The row's key.",
    "explorer: a description wrapped over lines folds onto one",
    card.summary("  The row's\n  key. ")
  )
  node.columns = { { name = "status", description = "Orders can be:\n\n| status |\n|---|\n| placed |" } }
  s.info = { model = "customers", description = "One row\nper customer.", tests = {} }
  local ok, multi = pcall(card.view, s, 60)
  local all_single = ok
  for _, line in ipairs(ok and multi.lines or {}) do
    all_single = all_single and not line:find("\n", 1, true)
  end
  H.check(all_single, "explorer: no card line holds a newline", ok and table.concat(multi.lines, "⏎") or multi)
  local buf = vim.api.nvim_create_buf(false, true)
  local set_ok = pcall(require("cogitator.ui").set_lines, buf, { "a\nb", "c" })
  H.check(
    set_ok and vim.deep_equal(vim.api.nvim_buf_get_lines(buf, 0, -1, false), { "a", "b", "c" }),
    "ui: set_lines splits a line that holds a newline instead of failing"
  )
  vim.api.nvim_buf_delete(buf, { force = true })

  -- the side panes list what walking can select: models, unless `.`
  local render = require("cogitator.explorer.render")
  local side = {
    selected = "dim_inventories",
    node_of = { dim_inventories = { name = "dim_inventories", kind = "model" } },
    info = {
      upstream = { { name = "bv_inventory_transitions", kind = "model" }, { name = "raw", kind = "source" } },
      downstream = {
        { name = "assert_fixture_watermarks", kind = "test" },
        { name = "fct_inventory_items", kind = "model" },
        { name = "assert_holdings_match", kind = "test" },
      },
    },
  }
  local shown, hidden = render.side_entries(side, "right")
  H.check(
    #shown == 1 and shown[1].name == "fct_inventory_items" and hidden.test == 2,
    "explorer: downstream lists the lineage, and counts the tests it leaves out",
    vim.inspect({ shown, hidden })
  )
  H.check(
    render.side_count(#shown, hidden) == "(1 · +2 tests)",
    "explorer: the pane's title says what `.` would add",
    render.side_count(#shown, hidden)
  )
  local up = render.side_entries(side, "left")
  H.check(#up == 2, "explorer: a source stays upstream, as lineage", vim.inspect(up))
  H.check(
    render.side_count(1, { test = 1, generic_test = 3 }) == "(1 · +3 generic tests, +1 test)",
    "explorer: several kinds left out, in order, with their plurals",
    render.side_count(1, { test = 1, generic_test = 3 })
  )
  side.show_all = true
  H.check(#render.side_entries(side, "right") == 3, "explorer: with `.` every neighbour is listed")
  side.show_all = false
  side.node_of.assert_fixture_watermarks = { name = "assert_fixture_watermarks", kind = "test" }
  side.selected = "assert_fixture_watermarks"
  H.check(
    #render.side_entries(side, "left") == 2,
    "explorer: a test's or a macro's neighbours are all listed — they are what it is about"
  )

  -- from a properties file, the entry under the cursor is the node
  local graph = require("cogitator.pick.graph")
  local yml = {
    "version: 2", -- 1
    "models:", -- 2
    "  - name: dim_inventories", -- 3
    "    description: >", -- 4
    "      The inventories.", -- 5
    "    columns:", -- 6
    "      - name: inventory_id", -- 7
    "        description: '{{ doc(\"inventory_id\") }}'", -- 8
    "  - name: 'dim_inventory_items'", -- 9
    "    columns:", -- 10
    "      - name: dim_inventories", -- 11  a column named like a model
  }
  H.check(graph.yaml_entry(yml, 1) == nil, "yaml: above the models list there is no entry")
  H.check(graph.yaml_entry(yml, 3) == "dim_inventories", "yaml: on an entry's name line, that entry")
  H.check(
    graph.yaml_entry(yml, 8) == "dim_inventories",
    "yaml: on a column of an entry, the entry — not the column",
    graph.yaml_entry(yml, 8)
  )
  H.check(
    graph.yaml_entry(yml, 11) == "dim_inventory_items",
    "yaml: a quoted name, and a column named like another model is still a column",
    graph.yaml_entry(yml, 11)
  )
  H.check(graph.yaml_entry({ "sources:", "  - name: raw" }, 2) == nil, "yaml: a source block is no node entry")

  -- the card says when the relation is only dbt's defaults
  s.relation = { relation = "jaffle.main.customers", origin = "defaults" }
  s.info = { model = "customers", tests = {} }
  local said = table.concat(card.view(s, 60).lines, "\n")
  H.check(said:find("dbt's defaults", 1, true) ~= nil, "explorer: a relation by the defaults says so", said)
  s.relation = { relation = "lake.dim.customers", origin = "manifest" }
  said = table.concat(card.view(s, 60).lines, "\n")
  H.check(
    said:find("relation: lake.dim.customers", 1, true) ~= nil and not said:find("defaults", 1, true),
    "explorer: a relation dbt rendered is shown as it is",
    said
  )
end
