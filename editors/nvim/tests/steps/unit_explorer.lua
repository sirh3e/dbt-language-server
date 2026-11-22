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
end
