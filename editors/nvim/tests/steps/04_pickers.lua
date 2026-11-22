-- The fuzzy pickers and the yazi-style navigator on the live server, in
-- the built-in picker (no picker plugin is installed in the suite).

return function(ctx)
  local H, bufnr = ctx.H, ctx.bufnr
  local pick = require("cogitator.pick")
  local builtin = require("cogitator.pick.backends.builtin")
  local config = require("cogitator").config
  local before = config.picker
  config.picker = "builtin"

  local function open(source)
    vim.api.nvim_set_current_buf(bufnr)
    local t = vim.uv.hrtime()
    pick.find(source)
    H.check((vim.uv.hrtime() - t) / 1e6 < 100, ("pickers: %s opens without waiting"):format(source))
    return H.wait(("pickers: %s lists its items"):format(source), 15000, function()
      return builtin._state ~= nil and builtin._state.items ~= nil and #builtin._state.items > 0
    end)
  end

  -- models: filter, preview, open
  if open("models") then
    builtin._set_query("stg cust")
    local item = builtin.current()
    H.check(
      item and item.name == "stg_customers",
      "pickers: a fuzzy query finds stg_customers",
      item and item.name
    )
    H.wait("pickers: the model's file is previewed", 10000, function()
      local st = builtin._state
      return st
        and table
            .concat(vim.api.nvim_buf_get_lines(st.bufs.preview, 0, -1, false), "\n")
            :find("select", 1, true)
          ~= nil
    end)
    builtin._accept("default")
    H.wait("pickers: <CR> opens the model", 10000, function()
      return vim.api.nvim_buf_get_name(0):find("stg_customers%.sql$") ~= nil
    end)
  end

  -- definitions: a macro, at its line
  if open("definitions") then
    builtin._set_query("times_five")
    builtin._accept("default")
    H.wait("pickers: a macro opens at its definition", 10000, function()
      return vim.api.nvim_buf_get_name(0):find("jaffle_macros%.sql$") ~= nil
        and vim.api.nvim_get_current_line():find("times_five", 1, true) ~= nil
    end, function()
      return vim.api.nvim_buf_get_name(0) .. " :: " .. vim.api.nvim_get_current_line()
    end)
  end

  -- columns: lands on the column
  if open("columns") then
    builtin._set_query("^customers.customer_id")
    local item = builtin.current()
    H.check(
      item and item.text:find("^customers%.customer_id") ~= nil,
      "pickers: columns are model.column",
      item and item.text
    )
    builtin._accept("default")
    H.wait("pickers: a column opens its model on the column", 10000, function()
      return vim.api.nvim_buf_get_name(0):find("customers%.sql$") ~= nil
        and vim.api.nvim_get_current_line():find("customer_id", 1, true) ~= nil
    end)
  end

  -- upstream of customers: its staging models, nearest first
  vim.cmd.edit(vim.fs.joinpath(ctx.fixture, "models", "customers.sql"))
  bufnr = vim.api.nvim_get_current_buf()
  H.wait("pickers: the model buffer attaches", 15000, function()
    return #vim.lsp.get_clients({ bufnr = bufnr, name = "cogitator" }) > 0
  end)
  if open("upstream") then
    local texts = vim.tbl_map(function(i)
      return i.text
    end, builtin._state.items)
    H.check(
      texts[1]:find("^↑1 ") ~= nil and table.concat(texts, "\n"):find("stg_customers", 1, true) ~= nil,
      "pickers: upstream lists the lineage with its depth",
      vim.inspect(texts)
    )
    -- <C-g>: show it in the navigator
    builtin._set_query("stg_orders")
    builtin._accept("nav")
  end
  local explorer = require("cogitator.explorer")
  H.wait("pickers: <C-g> opens the navigator on the pick", 15000, function()
    return explorer._state ~= nil and explorer._state.selected == "stg_orders"
  end, function()
    return vim.inspect(explorer._state and explorer._state.selected)
  end)

  -- the navigator: history, marks, fuzzy filter, columns tab, every kind
  local s = explorer._state
  if s then
    explorer.focus("customers", { record = true })
    explorer.focus("orders", { record = true })
    H.check(
      vim.deep_equal(s.history.back, { "stg_orders", "customers" }) and s.selected == "orders",
      "navigator: re-centring records the history",
      vim.inspect(s.history)
    )
    local title = vim.api.nvim_win_get_config(s.wins.center).title
    title = type(title) == "table" and title[1][1] or title
    H.check(title:find("customers › orders", 1, true) ~= nil, "navigator: the title is a breadcrumb", title)
    explorer._history(-1)
    H.check(s.selected == "customers" and #s.history.forward == 1, "navigator: [ goes back", s.selected)
    explorer._history(1)
    H.check(s.selected == "orders", "navigator: ] goes forward", s.selected)

    explorer._apply_filter("stg pay")
    H.check(
      #s.filtered >= 1 and s.filtered[1].name == "stg_payments" and #(s.positions.stg_payments or {}) > 0,
      "navigator: / filters with fzf's syntax and remembers the matches",
      vim.inspect(vim.tbl_map(function(n)
        return n.name
      end, s.filtered))
    )
    explorer._apply_filter("^stg")
    vim.api.nvim_win_set_cursor(s.wins.center, { 1, 0 })
    explorer._toggle_mark()
    explorer._toggle_mark()
    local chosen = vim.tbl_map(function(n)
      return n.name
    end, explorer._chosen())
    H.check(#chosen == 2, "navigator: <Space> marks nodes", vim.inspect(chosen))
    local center = table.concat(vim.api.nvim_buf_get_lines(s.bufs.center, 0, -1, false), "\n")
    H.check(center:find("● ", 1, true) ~= nil, "navigator: marked nodes are shown", center)

    explorer._apply_filter("")
    explorer.focus("customers")
    explorer._set_tab(5)
    H.wait("navigator: the columns tab lists the documented columns", 10000, function()
      return table
        .concat(vim.api.nvim_buf_get_lines(s.bufs.preview, 0, -1, false), "\n")
        :find("customer_id", 1, true) ~= nil
    end)
    local models_only = #s.nodes
    explorer._toggle_all()
    H.check(
      #s.nodes > models_only,
      "navigator: . lists every kind of node",
      ("%d → %d"):format(models_only, #s.nodes)
    )
    explorer._toggle_all()
    explorer.close()
  end

  -- :DbtBrowse at the buffer's model (<leader>dn)
  vim.api.nvim_set_current_buf(bufnr)
  explorer.open({ focus_buffer = true })
  H.wait("navigator: it opens at the buffer's model", 15000, function()
    return explorer._state ~= nil and explorer._state.selected == "customers"
  end)
  explorer.close()

  config.picker = before
  ctx.bufnr = vim.fn.bufnr(vim.fs.joinpath(ctx.fixture, "models", "customers.sql"))
end
