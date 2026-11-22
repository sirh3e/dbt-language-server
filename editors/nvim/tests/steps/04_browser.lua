-- The UI layers: graph browser (3 panes), the explorer :DbtBrowse (five
-- panes: upstream | model | downstream over dbt SQL | compiled SQL). Runs on the fixture COPY, so faking target/compiled
-- needs no cleanup and deletes nothing shared.

return function(ctx)
  local H, bufnr = ctx.H, ctx.bufnr

  local graph = require("cogitator.graph")
  graph.open()
  H.wait("integration: graph browser loads", 10000, function()
    return graph._state ~= nil and graph._state.data ~= nil
  end)
  local s = graph._state
  local ui_ok = s ~= nil
    and s.data ~= nil
    and vim.api.nvim_win_is_valid(s.wins.left)
    and vim.api.nvim_win_is_valid(s.wins.center)
    and vim.api.nvim_win_is_valid(s.wins.right)
  H.check(ui_ok, "integration: graph browser opens three panes")
  if ui_ok then
    local left = table.concat(vim.api.nvim_buf_get_lines(s.bufs.left, 0, -1, false), "\n")
    H.check(left:find("stg_customers") ~= nil, "integration: upstream pane lists stg_customers", left)
    local center = table.concat(vim.api.nvim_buf_get_lines(s.bufs.center, 0, -1, false), "\n")
    H.check(center:find("customers") ~= nil, "integration: center pane names the model", center)
  end
  if ui_ok then
    local start = s.data.model
    vim.api.nvim_set_current_win(s.wins.center)
    H.feed("h")
    H.check(vim.api.nvim_get_current_win() == s.wins.left, "integration: graph h enters upstream")
    H.feed("h")
    H.wait("integration: graph h walks upstream", 10000, function()
      return s.data.model ~= start and not s.loading
    end)
    H.check(vim.api.nvim_get_current_win() == s.wins.left, "integration: graph walk stays in the pane")
  end
  graph.close()
  H.check(graph._state == nil, "integration: graph browser closes cleanly")

  -- :DbtBrowse — dbt would write target/compiled/ at runtime; fake it
  -- inside the COPY
  local compiled_dir = vim.fs.joinpath(ctx.fixture, "target", "compiled", "jaffle_shop", "models")
  vim.fn.mkdir(compiled_dir, "p")
  vim.fn.writefile({ "select 42 as compiled_marker" }, vim.fs.joinpath(compiled_dir, "customers.sql"))

  -- the explorer opens on the buffer's model (customers)
  vim.api.nvim_set_current_buf(bufnr)
  local browse = require("cogitator.browse")
  browse.open()
  H.wait("integration: the explorer opens on the buffer's model", 10000, function()
    return browse._state ~= nil and browse._state.selected == "customers" and browse._state.info ~= nil
  end)
  local bst = browse._state
  local panes = { "left", "center", "right", "raw", "compiled" }
  local browse_ok = bst ~= nil
  for _, pane in ipairs(panes) do
    browse_ok = browse_ok and bst.wins[pane] ~= nil and vim.api.nvim_win_is_valid(bst.wins[pane])
  end
  H.check(browse_ok, "integration: the explorer opens five panes")
  if not browse_ok then
    pcall(browse.close)
    return
  end

  -- the layout: upstream | model | downstream above, dbt SQL | compiled below
  local pos = {}
  for _, pane in ipairs(panes) do
    local c = vim.api.nvim_win_get_config(bst.wins[pane])
    pos[pane] = { row = c.row, col = c.col, width = c.width }
  end
  H.check(
    pos.left.row == pos.center.row
      and pos.center.row == pos.right.row
      and pos.left.col < pos.center.col
      and pos.center.col < pos.right.col
      and pos.raw.row == pos.compiled.row
      and pos.raw.row > pos.center.row
      and pos.raw.col == pos.left.col
      and pos.raw.col < pos.compiled.col
      and pos.compiled.col + pos.compiled.width == pos.right.col + pos.right.width,
    "integration: upstream | model | downstream over dbt SQL | compiled SQL",
    vim.inspect(pos)
  )

  -- the card: description, tags, relation, columns, tests
  H.wait("integration: the card shows the relation", 10000, function()
    local text = table.concat(vim.api.nvim_buf_get_lines(bst.bufs.center, 0, -1, false), "\n")
    return text:find("relation: jaffle_shop%.main%.customers") ~= nil
  end)
  local card = table.concat(vim.api.nvim_buf_get_lines(bst.bufs.center, 0, -1, false), "\n")
  H.check(
    card:find("^customers") ~= nil
      and card:find("basic information", 1, true) ~= nil
      and card:find("#nightly", 1, true) ~= nil
      and card:find("• customer_id", 1, true) ~= nil
      and card:find("unique on customer_id", 1, true) ~= nil,
    "integration: the card shows description, tags, columns and tests",
    card
  )
  local left = table.concat(vim.api.nvim_buf_get_lines(bst.bufs.left, 0, -1, false), "\n")
  H.check(left:find("stg_customers") ~= nil, "integration: upstream lists the model's parents", left)

  -- the two SQL panes
  H.wait("integration: the dbt SQL pane shows the model's file", 10000, function()
    return table.concat(vim.api.nvim_buf_get_lines(bst.bufs.raw, 0, -1, false), "\n"):find("ref%(") ~= nil
  end)
  H.wait("integration: the compiled pane serves dbt.compiledSql", 10000, function()
    return table
      .concat(vim.api.nvim_buf_get_lines(bst.bufs.compiled, 0, -1, false), "\n")
      :find("compiled_marker") ~= nil
  end)
  H.check(
    vim.bo[bst.bufs.raw].filetype == "sql" and vim.bo[bst.bufs.compiled].filetype == "sql",
    "integration: both SQL panes are SQL buffers"
  )
  -- both coloured by tree-sitter-dbt through the server (dbt.highlight)
  local tshl = require("cogitator.tshl")
  local function groups(buf)
    local out = {}
    for _, m in ipairs(tshl.marks(buf)) do
      out[m[4].hl_group] = true
    end
    return out
  end
  H.wait("integration: the dbt SQL pane is coloured by tree-sitter-dbt", 10000, function()
    return groups(bst.bufs.raw)["@tag.delimiter"] ~= nil
  end, function()
    return vim.inspect(groups(bst.bufs.raw))
  end)
  H.check(
    next(vim.tbl_filter(function(g)
      return g:find("^@keyword") ~= nil
    end, vim.tbl_keys(groups(bst.bufs.raw)))) ~= nil,
    "integration: the SQL between the tags is coloured too",
    vim.inspect(groups(bst.bufs.raw))
  )
  H.check(vim.bo[bst.bufs.raw].syntax == "OFF", "integration: regex syntax gives way to the server's colours")
  H.wait("integration: the compiled SQL pane is coloured as SQL", 10000, function()
    local g = groups(bst.bufs.compiled)
    return next(g) ~= nil and g["@tag.delimiter"] == nil
  end, function()
    return vim.inspect(groups(bst.bufs.compiled))
  end)

  -- a model without compiled SQL says so
  browse.focus("orders")
  H.wait("integration: an uncompiled model says so", 10000, function()
    return table
      .concat(vim.api.nvim_buf_get_lines(bst.bufs.compiled, 0, -1, false), "\n")
      :find("not compiled", 1, true) ~= nil
  end)

  -- versions on the card (customer_orders is versioned)
  browse.focus("customer_orders")
  H.wait("integration: the card lists the versions", 10000, function()
    local text = table.concat(vim.api.nvim_buf_get_lines(bst.bufs.center, 0, -1, false), "\n")
    return text:find("versions (latest 2)", 1, true) ~= nil and text:find("v1", 1, true) ~= nil
  end)

  -- <CR> on a side entry makes it the model
  vim.api.nvim_set_current_win(bst.wins.right)
  browse.focus("stg_customers")
  H.wait("integration: stg_customers selected", 10000, function()
    return bst.info and bst.info.model == "stg_customers"
  end)
  vim.api.nvim_set_current_win(bst.wins.right)
  vim.api.nvim_win_set_cursor(bst.wins.right, { 1, 0 })
  local child = bst.info.downstream[1].name
  H.feed("<CR>")
  H.wait("integration: <CR> on downstream makes it the model", 10000, function()
    return bst.selected == child
  end)

  -- f / /: pick a model with the fuzzy picker
  local config = require("cogitator").config
  local before = config.picker
  config.picker = "builtin"
  vim.api.nvim_set_current_win(bst.wins.center)
  H.feed("/")
  local builtin = require("cogitator.pick.backends.builtin")
  H.wait("integration: / opens the models picker", 10000, function()
    return builtin._state ~= nil and builtin._state.items ~= nil
  end)
  builtin._set_query("^stg_pay")
  builtin._accept("default")
  H.wait("integration: the picked model becomes the selection", 10000, function()
    return bst.selected == "stg_payments"
  end)
  config.picker = before

  -- marks + r: one dbt.runSelection with every marked model
  browse.focus("customers")
  H.wait("integration: back on customers", 10000, function()
    return bst.info and bst.info.model == "customers"
  end)
  vim.api.nvim_set_current_win(bst.wins.center)
  H.feed("<Space>")
  vim.api.nvim_set_current_win(bst.wins.left)
  vim.api.nvim_win_set_cursor(bst.wins.left, { 1, 0 })
  H.feed("<Space>")
  local chosen = browse._chosen()
  H.check(
    #chosen == 2 and vim.tbl_contains(chosen, "customers"),
    "integration: <Space> marks the model and an entry",
    vim.inspect(chosen)
  )
  H.check(
    table.concat(vim.api.nvim_buf_get_lines(bst.bufs.center, 0, 1, false), ""):find("● marked", 1, true)
      ~= nil,
    "integration: the card says it is marked"
  )
  -- the suite has no dbt: catch the selector the run is started with
  local commands = require("cogitator.commands")
  local run_selection = commands.run_selection
  local selector
  commands.run_selection = function(opts)
    selector = opts.args
  end
  vim.api.nvim_set_current_win(bst.wins.center)
  H.feed("r")
  commands.run_selection = run_selection
  H.check(
    selector ~= nil and selector:find(chosen[1], 1, true) ~= nil and selector:find(chosen[2], 1, true) ~= nil,
    "integration: r runs both marked models in one dbt run",
    selector
  )
  H.check(next(bst.marked) == nil, "integration: running clears the marks")

  -- <CR> on a test line of the card jumps to its YAML
  local jump_line
  for i, l in ipairs(vim.api.nvim_buf_get_lines(bst.bufs.center, 0, -1, false)) do
    if l:find("unique on customer_id") then
      jump_line = i
    end
  end
  H.check(jump_line ~= nil, "integration: the card lists the model's tests")
  if jump_line then
    vim.api.nvim_set_current_win(bst.wins.center)
    vim.api.nvim_win_set_cursor(bst.wins.center, { jump_line, 0 })
    H.feed("<CR>")
    H.check(
      vim.api.nvim_buf_get_name(0):find("schema%.yml$") ~= nil and browse._state == nil,
      "integration: <CR> on a test opens its schema entry",
      vim.api.nvim_buf_get_name(0)
    )
  end
  vim.api.nvim_set_current_buf(bufnr)
  browse.open()
  H.wait("integration: the explorer reopens", 10000, function()
    return browse._state ~= nil and browse._state.selected == "customers"
  end)

  -- macros and tests (specs/highlighting §6.2): with every kind shown
  bst = browse._state
  bst.show_all = true
  H.wait("explorer: the project's macros are known", 10000, function()
    return bst.node_of["times_five"] ~= nil and bst.node_of["times_five"].kind == "macro"
  end)
  H.check(browse.focus("times_five", { record = true }), "explorer: a macro can be selected")
  H.wait("explorer: a macro's card", 10000, function()
    local card_text = table.concat(vim.api.nvim_buf_get_lines(bst.bufs.center, 0, -1, false), "\n")
    return card_text:find("times_five(int_value)", 1, true) ~= nil
      and card_text:find("called by", 1, true) ~= nil
  end, function()
    return table.concat(vim.api.nvim_buf_get_lines(bst.bufs.center, 0, -1, false), "\n")
  end)
  local right = table.concat(vim.api.nvim_buf_get_lines(bst.bufs.right, 0, -1, false), "\n")
  H.check(right:find("customers") ~= nil, "explorer: a macro's callers are downstream", right)
  H.wait("explorer: the raw pane starts at the macro and marks it", 10000, function()
    local marks = vim.api.nvim_buf_get_extmarks(bst.bufs.raw, tshl.def_ns, 0, -1, {})
    local top = vim.fn.getwininfo(bst.wins.raw)[1].topline
    local line = vim.api.nvim_buf_get_lines(bst.bufs.raw, top - 1, top, false)[1] or ""
    return #marks > 0 and line:find("times_five", 1, true) ~= nil
  end)
  H.wait("explorer: a macro's compiled pane lists its callers", 10000, function()
    local lines = vim.api.nvim_buf_get_lines(bst.bufs.compiled, 0, -1, false)
    return lines[1] and lines[1]:find("a macro compiles into", 1, true) ~= nil
  end)
  local caller_line
  for i, l in ipairs(vim.api.nvim_buf_get_lines(bst.bufs.compiled, 0, -1, false)) do
    if l:find("customers", 1, true) then
      caller_line = i
    end
  end
  if caller_line then
    vim.api.nvim_set_current_win(bst.wins.compiled)
    vim.api.nvim_win_set_cursor(bst.wins.compiled, { caller_line, 0 })
    H.feed("<CR>")
    H.wait("explorer: <CR> on a caller re-centres on it", 10000, function()
      return bst.selected == "customers"
    end)
  else
    H.check(false, "explorer: the compiled pane names the caller")
  end

  H.check(
    browse.focus("assert_positive_order_amount", { record = true }),
    "explorer: a singular test can be selected"
  )
  H.wait("explorer: a test's card and refs", 10000, function()
    local card_text = table.concat(vim.api.nvim_buf_get_lines(bst.bufs.center, 0, -1, false), "\n")
    local left_text = table.concat(vim.api.nvim_buf_get_lines(bst.bufs.left, 0, -1, false), "\n")
    return card_text:find("tests (1)", 1, true) ~= nil and left_text:find("orders") ~= nil
  end, function()
    return table.concat(vim.api.nvim_buf_get_lines(bst.bufs.center, 0, -1, false), "\n")
  end)
  H.wait("explorer: an uncompiled test says how it compiles", 10000, function()
    local lines = vim.api.nvim_buf_get_lines(bst.bufs.compiled, 0, -1, false)
    return lines[1] and lines[1]:find("target/compiled", 1, true) ~= nil
  end)
  bst.show_all = false
  browse.focus("customers", { record = true })
  H.wait("explorer: back on the model", 10000, function()
    return bst.selected == "customers" and bst.info and bst.info.model == "customers"
  end)

  dofile(vim.fs.joinpath(ctx.repo_root, "editors/nvim/tests/steps/browser_motions.lua"))(ctx, browse)

  browse.close()
  H.check(browse._state == nil, "integration: browser closes cleanly")
end
