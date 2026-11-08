-- :DbtBrowse vim motions with real keypresses. Runs inside 04_browser
-- and opens its own browser, which it leaves open for the caller.

return function(ctx, browse)
  local H = ctx.H
  browse.open()
  H.wait("motions: browser loads", 10000, function()
    return browse._state ~= nil and #browse._state.nodes > 0 and browse._state.wins.preview ~= nil
  end)
  local st = browse._state

  local function select_model(name)
    browse._apply_filter(name)
    local row
    for i, n in ipairs(browse._state.filtered) do
      if n.name == name then
        row = i
      end
    end
    H.check(row ~= nil, "motions: " .. name .. " is listed")
    vim.api.nvim_set_current_win(st.wins.center)
    vim.api.nvim_win_set_cursor(st.wins.center, { row, 0 })
    vim.api.nvim_exec_autocmds("CursorMoved", { buffer = st.bufs.center })
    H.wait("motions: selection settles on " .. name, 10000, function()
      return st.info and st.info.model == name
    end)
  end

  local function current_pane()
    local win = vim.api.nvim_get_current_win()
    for name, w in pairs(st.wins) do
      if w == win then
        return name
      end
    end
  end

  -- counts reach native motions now that digits aren't mapped
  browse._apply_filter("")
  vim.api.nvim_set_current_win(st.wins.center)
  vim.api.nvim_win_set_cursor(st.wins.center, { 1, 0 })
  local tab_before = st.tab
  H.feed("2j")
  H.check(vim.api.nvim_win_get_cursor(st.wins.center)[1] == 3, "motions: 2j moves two rows")
  H.check(st.tab == tab_before, "motions: 2j leaves the preview tab alone")

  -- preview tabs
  H.feed("3<Tab>")
  H.check(st.tab == 3, "motions: 3<Tab> selects tab 3", st.tab)
  H.feed("<Tab>")
  H.check(st.tab == 4, "motions: <Tab> cycles forward", st.tab)
  H.feed("<S-Tab>")
  H.check(st.tab == 3, "motions: <S-Tab> cycles back", st.tab)
  H.feed("1<Tab>")

  -- n/N wrap around the center list
  local total = #st.filtered
  vim.api.nvim_win_set_cursor(st.wins.center, { total, 0 })
  H.feed("n")
  H.check(vim.api.nvim_win_get_cursor(st.wins.center)[1] == 1, "motions: n wraps to the top")
  H.feed("N")
  H.check(vim.api.nvim_win_get_cursor(st.wins.center)[1] == total, "motions: N wraps to the bottom")

  -- J/K scroll the preview from the center
  select_model("customers")
  H.feed("J")
  H.check(vim.fn.getwininfo(st.wins.preview)[1].topline == 6, "motions: J scrolls the preview down")
  H.feed("K")
  H.check(vim.fn.getwininfo(st.wins.preview)[1].topline == 1, "motions: K scrolls the preview up")

  -- y yanks the selected name
  H.feed("y")
  H.check(vim.fn.getreg('"') == "customers", "motions: y yanks the model name", vim.fn.getreg('"'))

  -- h/l between panes
  H.feed("h")
  H.check(current_pane() == "left", "motions: h from center enters upstream", current_pane())
  H.feed("l")
  H.check(current_pane() == "center", "motions: l from upstream returns", current_pane())
  H.feed("l")
  H.check(current_pane() == "right", "motions: l from center enters downstream", current_pane())
  H.feed("h")
  H.check(current_pane() == "center", "motions: h from downstream returns", current_pane())

  -- side cursors survive a same-model refresh
  H.feed("h")
  vim.api.nvim_win_set_cursor(st.wins.left, { 2, 0 })
  require("cogitator.explorer.render").sides(st)
  H.check(vim.api.nvim_win_get_cursor(st.wins.left)[1] == 2, "motions: side cursor survives a refresh")

  -- two fast presses walk two levels: customer_orders → orders → stg_orders
  select_model("customer_orders")
  H.feed("h")
  H.feed("hh")
  H.check(current_pane() == "left", "motions: walking stays in the pane", current_pane())
  H.wait("motions: fast hh walks two levels", 10000, function()
    return st.info and st.info.model == "stg_orders" and st.pending_walk == nil
  end)
  H.check(vim.api.nvim_win_get_cursor(st.wins.left)[1] == 1, "motions: a new selection resets the side cursor")

  -- [count]h walks too
  select_model("customer_orders")
  H.feed("h")
  H.feed("2h")
  H.wait("motions: 2h walks two levels", 10000, function()
    return st.info and st.info.model == "stg_orders" and st.pending_walk == nil
  end)

  -- sources are coloured, and can't be re-centered
  select_model("stg_stripe_payments")
  local ns = vim.api.nvim_get_namespaces()["cogitator.pane"]
  local marks = vim.api.nvim_buf_get_extmarks(st.bufs.left, ns, 0, -1, { details = true })
  H.check(
    #marks > 0 and marks[1][4].hl_group == "DbtDagSourceFg",
    "motions: a source entry uses the source colour",
    vim.inspect(marks)
  )
  H.feed("h")
  H.feed("h")
  H.check(st.selected == "stg_stripe_payments", "motions: a source can't be walked onto", st.selected)
  H.check(st.pending_walk == nil, "motions: a refused walk leaves nothing queued")
  browse._apply_filter("")
end
