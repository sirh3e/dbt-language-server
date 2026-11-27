-- :DbtDag's modes on the booted session: focus with depth, follow,
-- the column-lineage graph (L, :DbtColumnLineage), back to the models
-- (m), and the redraw after a save.

return function(ctx)
  local H, bufnr = ctx.H, ctx.bufnr
  local dag = require("cogitator.dag")

  vim.api.nvim_set_current_buf(bufnr)
  dag.open()
  if
    not H.wait("graphs: :DbtDag opens", 15000, function()
      return dag._state ~= nil and dag._state.selected ~= nil
    end)
  then
    return
  end
  local s = dag._state
  local all = #s.data.nodes
  H.check(
    s.boxes[s.selected].node.name == "customers",
    "graphs: the DAG opens on the buffer's model",
    s.selected
  )

  H.feed("f")
  H.check(
    s.focused and #s.data.nodes < all and s.boxes["model:customers"] ~= nil,
    "graphs: f focuses on the selection's neighbourhood",
    ("%d of %d"):format(#s.data.nodes, all)
  )
  local narrow = #s.data.nodes
  H.feed("+")
  H.check(s.depth == 3 and #s.data.nodes >= narrow, "graphs: + widens the focus", s.depth)
  H.check(
    vim.wo[s.win].winbar:find("focus ±3", 1, true) ~= nil,
    "graphs: the winbar says the focus",
    vim.wo[s.win].winbar
  )
  H.feed("f")
  H.check(not s.focused and #s.data.nodes == all, "graphs: f again shows the whole project")

  H.feed("F")
  H.check(s.follow, "graphs: F turns follow on")
  vim.cmd("tabprevious")
  vim.cmd.edit(vim.fs.joinpath(ctx.fixture, "models", "orders.sql"))
  H.check(s.selected == "model:orders", "graphs: follow selects the model of the entered buffer", s.selected)
  vim.cmd.edit(vim.fs.joinpath(ctx.fixture, "models", "customers.sql"))
  vim.cmd("tabnext")
  H.feed("F")

  -- L on the selected model: the column graph of customers
  dag.select("model:customers")
  H.feed("L")
  H.wait("graphs: L walks the column lineage", 20000, function()
    return s.mode == "columns" and #s.data.nodes > 0
  end, function()
    return vim.inspect(s.note)
  end)
  local upstream_edge = false
  for _, edge in ipairs(s.data.edges) do
    if edge.to:find("^model:customers::") and not edge.from:find("^model:customers::") then
      upstream_edge = true
    end
  end
  H.check(
    upstream_edge,
    "graphs: customers' columns read columns of other models",
    vim.inspect(s.data.edges):sub(1, 300)
  )
  H.check(
    vim.wo[s.win].winbar:find("columns of customers", 1, true) ~= nil,
    "graphs: the winbar names the column graph",
    vim.wo[s.win].winbar
  )

  H.feed("m")
  H.check(
    s.mode == "models" and s.selected == "model:customers",
    "graphs: m goes back to the model graph on the column's model",
    s.selected
  )

  -- redraw after a save: the project payload is fetched again
  local before = s.project
  vim.cmd("tabprevious")
  vim.cmd("silent write")
  H.wait("graphs: a save refreshes the open DAG", 15000, function()
    return dag._state and dag._state.project ~= before
  end)
  dag.close()
  H.check(dag._state == nil, "graphs: q/close clears the view")

  -- :DbtColumnLineage graph straight from the model buffer, one column
  vim.api.nvim_set_current_buf(bufnr)
  vim.cmd("DbtColumnLineage graph customer_id")
  H.wait("graphs: :DbtColumnLineage opens the column graph", 20000, function()
    return dag._state
      and dag._state.mode == "columns"
      and dag._state.selected == "model:customers::customer_id"
  end, function()
    return dag._state and vim.inspect(dag._state.selected)
  end)
  dag.close()
  vim.api.nvim_set_current_buf(bufnr)

  -- the explorer's actions: g shows the selection in the DAG, L its
  -- column lineage, v opens it in a vsplit
  local explorer = require("cogitator.explorer")
  local function open_explorer(label)
    vim.api.nvim_set_current_buf(bufnr)
    explorer.open()
    local ok = H.wait(label, 10000, function()
      return explorer._state ~= nil and explorer._selected_node() ~= nil and explorer._state.info ~= nil
    end)
    if ok then
      vim.api.nvim_set_current_win(explorer._state.wins.center)
    end
    return ok
  end
  if open_explorer("graphs: explorer opens for g") then
    local picked = explorer._selected_node()
    H.feed("g")
    H.wait("graphs: explorer g opens the DAG on the selection", 15000, function()
      return explorer._state == nil and dag._state ~= nil and dag._state.selected == "model:" .. picked.name
    end, function()
      return dag._state and dag._state.selected
    end)
    dag.close()
  end
  if open_explorer("graphs: explorer opens for L") then
    -- the explorer opens on the buffer's model, customers, which has column lineage
    local picked = explorer._selected_node()
    H.feed("L")
    H.wait("graphs: explorer L opens the selection's column graph", 20000, function()
      return dag._state ~= nil and dag._state.mode == "columns" and dag._state.start.model == picked.name
    end)
    dag.close()
  end
  if open_explorer("graphs: explorer opens for v") then
    local picked = explorer._selected_node()
    local wins = #vim.api.nvim_tabpage_list_wins(0)
    H.feed("v")
    H.check(
      explorer._state == nil
        and vim.uri_from_bufnr(0) == picked.uri
        -- five floats closed, one vsplit opened
        and #vim.api.nvim_tabpage_list_wins(0) == wins - 4,
      "graphs: explorer v closes the explorer and opens the model in a vsplit",
      vim.api.nvim_buf_get_name(0)
    )
    vim.cmd.only()
  end
  vim.api.nvim_set_current_buf(bufnr)

  -- :Dbt routes to the same commands
  vim.cmd("Dbt dag")
  H.wait("graphs: :Dbt dag opens the DAG", 15000, function()
    return dag._state ~= nil and dag._state.selected ~= nil
  end)
  dag.close()
  vim.api.nvim_set_current_buf(bufnr)
  local reloaded = false
  local notify = vim.notify
  vim.notify = function(msg, ...)
    reloaded = reloaded or tostring(msg):find("reload requested", 1, true) ~= nil
    return notify(msg, ...)
  end
  vim.cmd("Dbt reload")
  H.wait("graphs: :Dbt reload asks the server to re-read cogitator.yml", 10000, function()
    return reloaded
  end)
  vim.notify = notify
end
