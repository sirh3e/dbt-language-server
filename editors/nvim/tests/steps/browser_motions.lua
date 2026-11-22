-- The explorer's keys with real keypresses. Runs inside 04_browser on
-- the explorer it opened, which it leaves open for the caller.

return function(ctx, browse)
  local H = ctx.H
  local st = browse._state

  local function select_model(name)
    browse.focus(name)
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

  -- J/K scroll both SQL panes from the model
  select_model("customers")
  H.wait("motions: the dbt SQL is read", 10000, function()
    return vim.api.nvim_buf_line_count(st.bufs.raw) > 10
  end)
  vim.api.nvim_set_current_win(st.wins.center)
  H.feed("J")
  H.check(vim.fn.getwininfo(st.wins.raw)[1].topline == 6, "motions: J scrolls the dbt SQL down")
  H.feed("K")
  H.check(vim.fn.getwininfo(st.wins.raw)[1].topline == 1, "motions: K scrolls it back up")

  -- y yanks the selected name
  H.feed("y")
  H.check(vim.fn.getreg('"') == "customers", "motions: y yanks the model name", vim.fn.getreg('"'))

  -- h/l between the top panes, p and h/l in the SQL panes, <Esc> back
  H.feed("h")
  H.check(current_pane() == "left", "motions: h from the model enters upstream", current_pane())
  H.feed("l")
  H.check(current_pane() == "center", "motions: l from upstream returns", current_pane())
  H.feed("l")
  H.check(current_pane() == "right", "motions: l from the model enters downstream", current_pane())
  H.feed("h")
  H.check(current_pane() == "center", "motions: h from downstream returns", current_pane())
  H.feed("p")
  H.check(current_pane() == "raw", "motions: p enters the dbt SQL", current_pane())
  H.feed("l")
  H.check(current_pane() == "compiled", "motions: l moves to the compiled SQL", current_pane())
  H.feed("h")
  H.check(current_pane() == "raw", "motions: h moves back to the dbt SQL", current_pane())
  H.feed("<Esc>")
  H.check(current_pane() == "center", "motions: <Esc> returns to the model", current_pane())

  -- side cursors survive a same-model refresh
  H.feed("h")
  vim.api.nvim_win_set_cursor(st.wins.left, { 2, 0 })
  require("cogitator.explorer.render").sides(st)
  H.check(vim.api.nvim_win_get_cursor(st.wins.left)[1] == 2, "motions: side cursor survives a refresh")

  -- two fast presses walk two levels: customer_orders → orders → stg_orders
  select_model("customer_orders")
  vim.api.nvim_set_current_win(st.wins.center)
  H.feed("h")
  H.feed("hh")
  H.check(current_pane() == "left", "motions: walking stays in the pane", current_pane())
  H.wait("motions: fast hh walks two levels", 10000, function()
    return st.info and st.info.model == "stg_orders" and st.pending_walk == nil
  end)
  H.check(
    vim.api.nvim_win_get_cursor(st.wins.left)[1] == 1,
    "motions: a new selection resets the side cursor"
  )
  H.check(
    vim.deep_equal(st.history.back, { "customers", "customer_orders", "orders" })
      or st.history.back[#st.history.back] == "orders",
    "motions: walking records the history",
    vim.inspect(st.history.back)
  )
  H.feed("[")
  H.wait("motions: [ goes back", 10000, function()
    return st.selected == "orders"
  end)
  H.feed("]")
  H.wait("motions: ] goes forward", 10000, function()
    return st.selected == "stg_orders"
  end)

  -- [count]h walks too
  select_model("customer_orders")
  vim.api.nvim_set_current_win(st.wins.center)
  H.feed("h")
  H.feed("2h")
  H.wait("motions: 2h walks two levels", 10000, function()
    return st.info and st.info.model == "stg_orders" and st.pending_walk == nil
  end)

  -- sources are coloured, and can't be walked onto until . lets them
  select_model("stg_stripe_payments")
  local ns = vim.api.nvim_get_namespaces()["cogitator.pane"]
  local marks = vim.api.nvim_buf_get_extmarks(st.bufs.left, ns, 0, -1, { details = true })
  H.check(
    #marks > 0 and marks[1][4].hl_group == "DbtDagSourceFg",
    "motions: a source entry uses the source colour",
    vim.inspect(marks)
  )
  vim.api.nvim_set_current_win(st.wins.center)
  H.feed("h")
  H.feed("h")
  H.check(st.selected == "stg_stripe_payments", "motions: a source can't be walked onto", st.selected)
  H.check(st.pending_walk == nil, "motions: a refused walk leaves nothing queued")
  H.feed(".")
  H.feed("h")
  H.wait("motions: with . a walk centres the source", 10000, function()
    return st.selected ~= "stg_stripe_payments" and st.node_of[st.selected].kind == "source"
  end, function()
    return st.selected
  end)
  H.feed(".")
end
