-- dbt.columnLineageGraph against the plugin's own walk: the server's one
-- answer must be the graph column_lineage.walk assembles from one
-- dbt.columnLineage request per hop, node for node and edge for edge.

return function(ctx)
  local H, client, bufnr = ctx.H, ctx.client, ctx.bufnr
  local column_lineage = require("cogitator.column_lineage")
  local lineage_graph = require("cogitator.lineage_graph")

  H.check(lineage_graph.supported(client), "lineage graph: the server advertises the command")
  H.check(not lineage_graph.supported(nil), "lineage graph: no client, no command")
  local arg = lineage_graph.argument("file:///m.sql", { column = "id", depth = 0 })
  H.check(
    arg.direction == "both" and arg.depth == 1 and arg.column == "id",
    "lineage graph: the argument defaults to both ways, at least one hop",
    vim.inspect(arg)
  )

  local project = H.exec_cmd(client, bufnr, "dbt.projectGraph", vim.empty_dict(), "lineage graph: project")
  if not H.check(type(project) == "table", "lineage graph: the project graph", vim.inspect(project)) then
    return
  end
  local by_id = {}
  for _, node in ipairs(project.nodes) do
    by_id[node.id] = node
  end

  --- What the server sends beyond the walk: dropped before comparing.
  local function as_walk(nodes, edges)
    local n = vim.tbl_map(function(node)
      local copy = vim.deepcopy(node)
      copy.compiledUri = nil
      return copy
    end, nodes)
    local e = vim.tbl_map(function(edge)
      return { from = edge.from, to = edge.to }
    end, edges)
    return n, e
  end

  for _, start in ipairs({
    { model = "customers", column = "customer_id" },
    { model = "customers" },
    { model = "stg_orders", column = "order_id" },
    { model = "orders", column = "order_id" },
  }) do
    local label = ("lineage graph: %s.%s"):format(start.model, start.column or "*")
    local walked
    column_lineage.walk(project, start, function(uri, cb)
      client:exec_cmd(
        { command = "dbt.columnLineage", arguments = { { uri = uri } } },
        { bufnr = bufnr },
        function(err, result)
          cb((not err and result ~= vim.NIL) and result or nil)
        end
      )
    end, { depth = 3, max_requests = 1000 }, function(result)
      walked = result
    end)
    H.wait(label .. " [the walk ends]", 20000, function()
      return walked ~= nil
    end)
    local served = H.exec_cmd(client, bufnr, lineage_graph.command, {
      uri = by_id["model:" .. start.model].uri,
      column = start.column,
      direction = "both",
      depth = 3,
    }, label, 20000)
    if walked and type(served) == "table" then
      local nodes, edges = as_walk(served.nodes, served.edges)
      H.check(
        vim.deep_equal(nodes, walked.data.nodes),
        label .. ": the same nodes",
        vim.inspect({ served = nodes, walked = walked.data.nodes }):sub(1, 1500)
      )
      H.check(
        vim.deep_equal(edges, walked.data.edges),
        label .. ": the same edges",
        vim.inspect({ served = edges, walked = walked.data.edges }):sub(1, 1500)
      )
      H.check(served.start == walked.start, label .. ": the same start", served.start)
      local missing = vim.deepcopy(served.missing or {})
      local expected = vim.deepcopy(walked.missing)
      table.sort(missing)
      table.sort(expected)
      H.check(vim.deep_equal(missing, expected), label .. ": the same missing models", vim.inspect(missing))
    end
  end

  local fetched
  lineage_graph.fetch(bufnr, { column = "customer_id", direction = "up" }, function(err, result)
    fetched = { err = err, result = result }
  end)
  H.wait("lineage graph: fetch answers", 20000, function()
    return fetched ~= nil
  end)
  H.check(
    fetched and fetched.err == nil and type(fetched.result) == "table" and #fetched.result.nodes > 0,
    "lineage graph: fetch hands the graph to its callback",
    vim.inspect(fetched):sub(1, 400)
  )
end
