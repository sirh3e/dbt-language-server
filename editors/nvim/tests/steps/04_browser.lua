-- The UI layers: graph browser (3 panes), :DbtBrowse (4 panes, filter,
-- tabs, test jump). Runs on the fixture COPY, so faking target/compiled
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
  graph.close()
  H.check(graph._state == nil, "integration: graph browser closes cleanly")

  -- :DbtBrowse — dbt would write target/compiled/ at runtime; fake it
  -- inside the COPY
  local compiled_dir = vim.fs.joinpath(ctx.fixture, "target", "compiled", "jaffle_shop", "models")
  vim.fn.mkdir(compiled_dir, "p")
  vim.fn.writefile({ "select 42 as compiled_marker" }, vim.fs.joinpath(compiled_dir, "customers.sql"))

  local browse = require("cogitator.browse")
  browse.open()
  H.wait("integration: browser loads the project", 10000, function()
    return browse._state ~= nil and #browse._state.nodes > 0
  end)
  local bst = browse._state
  local browse_ok = bst ~= nil
    and #bst.nodes > 0
    and vim.api.nvim_win_is_valid(bst.wins.left)
    and vim.api.nvim_win_is_valid(bst.wins.center)
    and vim.api.nvim_win_is_valid(bst.wins.right)
    and vim.api.nvim_win_is_valid(bst.wins.preview)
  H.check(browse_ok, "integration: browser opens four panes")
  if not browse_ok then
    pcall(browse.close)
    return
  end

  local center = table.concat(vim.api.nvim_buf_get_lines(bst.bufs.center, 0, -1, false), "\n")
  H.check(
    center:find("customers") ~= nil and center:find("customer_orders") ~= nil,
    "integration: browser lists the whole project",
    center
  )

  -- filter + selection: the CursorMoved autocmd (not the filter itself)
  -- drives the modelInfo fetch, so fire it like a user would
  browse._apply_filter("customer_orders")
  local co_row
  for i, n in ipairs(browse._state.filtered) do
    if n.name == "customer_orders" then
      co_row = i
    end
  end
  H.check(co_row ~= nil, "integration: browser filter keeps customer_orders reachable")
  if co_row then
    vim.api.nvim_win_set_cursor(bst.wins.center, { co_row, 0 })
    vim.api.nvim_exec_autocmds("CursorMoved", { buffer = bst.bufs.center })
  end
  H.wait("integration: browser selection settles on customer_orders", 10000, function()
    local st = browse._state
    return st and st.info and st.info.model == "customer_orders"
  end)
  local left = table.concat(vim.api.nvim_buf_get_lines(bst.bufs.left, 0, -1, false), "\n")
  H.check(left:find("orders") ~= nil, "integration: browser upstream pane follows the selection", left)
  local prev = table.concat(vim.api.nvim_buf_get_lines(bst.bufs.preview, 0, -1, false), "\n")
  H.check(prev:find("ref%(") ~= nil, "integration: browser raw tab shows the model SQL", prev)

  -- versions & tests tab
  vim.api.nvim_set_current_win(bst.wins.center)
  vim.api.nvim_feedkeys("3", "x", false)
  prev = table.concat(vim.api.nvim_buf_get_lines(bst.bufs.preview, 0, -1, false), "\n")
  H.check(
    prev:find("latest_version: 2") ~= nil and prev:find("not_null") ~= nil and prev:find("v1") ~= nil,
    "integration: browser versions/tests tab reads the schema",
    prev
  )

  -- compiled tab on customers (cursor move drives the CursorMoved autocmd)
  browse._apply_filter("customers")
  local row
  for i, n in ipairs(browse._state.filtered) do
    if n.name == "customers" then
      row = i
    end
  end
  H.check(row ~= nil, "integration: browser filter keeps customers reachable")
  if row then
    vim.api.nvim_win_set_cursor(bst.wins.center, { row, 0 })
    vim.api.nvim_exec_autocmds("CursorMoved", { buffer = bst.bufs.center })
    H.wait("integration: browser selection settles on customers", 10000, function()
      local st = browse._state
      return st and st.info and st.info.model == "customers"
    end)
    vim.api.nvim_feedkeys("2", "x", false)
    -- the compiled tab is a dbt.compiledSql round trip: wait for it
    H.wait("integration: compiled tab round trip", 10000, function()
      local p = table.concat(vim.api.nvim_buf_get_lines(bst.bufs.preview, 0, -1, false), "\n")
      return p:find("compiled_marker") ~= nil
    end)
    prev = table.concat(vim.api.nvim_buf_get_lines(bst.bufs.preview, 0, -1, false), "\n")
    H.check(prev:find("compiled_marker") ~= nil, "integration: browser compiled tab serves dbt.compiledSql", prev)
  end

  -- search narrows, the live `/` loop works, <Esc> clears
  browse._apply_filter("stg")
  local stg_names = vim.tbl_map(function(n)
    return n.name
  end, browse._state.filtered)
  local only_stg = #stg_names > 0 and vim.tbl_contains(stg_names, "stg_payments")
  for _, name in ipairs(stg_names) do
    only_stg = only_stg and name:find("stg") ~= nil
  end
  H.check(only_stg, "integration: browser filter narrows to stg models", vim.inspect(stg_names))
  browse._apply_filter("")
  vim.api.nvim_set_current_win(bst.wins.center)
  vim.api.nvim_feedkeys("/stg\r", "x", false)
  H.check(browse._state.filter == "stg", "integration: live / filter loop", browse._state.filter)
  vim.api.nvim_feedkeys(vim.api.nvim_replace_termcodes("<Esc>", true, false, true), "x", false)
  H.check(browse._state ~= nil and browse._state.filter == "", "integration: <Esc> clears the filter first")

  -- info tab: description/tags/relation + test jump
  browse._apply_filter("customers")
  local info_row
  for i, n in ipairs(browse._state.filtered) do
    if n.name == "customers" then
      info_row = i
    end
  end
  if info_row then
    vim.api.nvim_win_set_cursor(bst.wins.center, { info_row, 0 })
    vim.api.nvim_exec_autocmds("CursorMoved", { buffer = bst.bufs.center })
    H.wait("integration: info selection settles", 10000, function()
      local st = browse._state
      return st and st.info and st.info.model == "customers"
    end)
    vim.api.nvim_set_current_win(bst.wins.center)
    vim.api.nvim_feedkeys("4", "x", false)
    H.wait("integration: info tab renders", 10000, function()
      local p = table.concat(vim.api.nvim_buf_get_lines(bst.bufs.preview, 0, -1, false), "\n")
      return p:find("relation: jaffle_shop%.main%.customers") ~= nil
    end)
    local prev_info = table.concat(vim.api.nvim_buf_get_lines(bst.bufs.preview, 0, -1, false), "\n")
    H.check(
      prev_info:find("#nightly") ~= nil and prev_info:find("relation: jaffle_shop%.main%.customers") ~= nil,
      "integration: browser info tab shows tags and relation",
      prev_info:sub(1, 200)
    )
    local jump_line
    for i, l in ipairs(vim.api.nvim_buf_get_lines(bst.bufs.preview, 0, -1, false)) do
      if l:find("unique on customer_id") then
        jump_line = i
      end
    end
    H.check(jump_line ~= nil, "integration: info tab lists the model's tests", prev_info:sub(1, 200))
    if jump_line then
      vim.api.nvim_set_current_win(bst.wins.preview)
      vim.api.nvim_win_set_cursor(bst.wins.preview, { jump_line, 0 })
      vim.api.nvim_feedkeys(vim.api.nvim_replace_termcodes("<CR>", true, false, true), "x", false)
      H.check(
        vim.api.nvim_buf_get_name(0):find("schema%.yml$") ~= nil,
        "integration: info tab test jump opens the schema entry",
        vim.api.nvim_buf_get_name(0)
      )
    end
    vim.api.nvim_set_current_buf(bufnr)
  end

  browse.close()
  H.check(browse._state == nil, "integration: browser closes cleanly")
end
