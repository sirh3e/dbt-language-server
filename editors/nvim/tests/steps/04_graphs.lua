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

  -- :DbtColumnLineage straight from the model buffer, one column
  vim.api.nvim_set_current_buf(bufnr)
  vim.cmd("DbtColumnLineage customer_id")
  H.wait("graphs: :DbtColumnLineage opens the column graph", 20000, function()
    return dag._state
      and dag._state.mode == "columns"
      and dag._state.selected == "model:customers::customer_id"
  end, function()
    return dag._state and vim.inspect(dag._state.selected)
  end)
  dag.close()
  vim.api.nvim_set_current_buf(bufnr)
end
