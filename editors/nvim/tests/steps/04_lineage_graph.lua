-- dbt.columnLineageGraph, the server's walk over its column graph, which
-- every column-lineage view draws (the plugin walks nothing itself):
-- the answer around a jaffle_shop column, the helpers, and fetch reaching
-- its callback as (err, result) with the start model's uri.

return function(ctx)
  local H, client, bufnr = ctx.H, ctx.client, ctx.bufnr
  local lineage_graph = require("cogitator.lineage_graph")

  H.check(lineage_graph.supported(client), "lineage graph: the server advertises the command")
  H.check(not lineage_graph.supported(nil), "lineage graph: no client, no command")
  local arg = lineage_graph.argument("file:///m.sql", { column = "id", depth = 0 })
  H.check(
    arg.direction == "both" and arg.depth == 1 and arg.column == "id",
    "lineage graph: the argument defaults to both ways, at least one hop",
    vim.inspect(arg)
  )
  H.check(
    lineage_graph.note({ stale = { "a", "b" } }) == "2 models still being computed"
      and lineage_graph.note({ stale = {} }) == nil,
    "lineage graph: the winbar note says what is still being computed"
  )

  local project = H.exec_cmd(client, bufnr, "dbt.projectGraph", vim.empty_dict(), "lineage graph: project")
  if not H.check(type(project) == "table", "lineage graph: the project graph", vim.inspect(project)) then
    return
  end
  local customers = lineage_graph.model_uri(project, "customers")
  H.check(
    type(customers) == "string",
    "lineage graph: a model's uri from the project graph",
    tostring(customers)
  )
  H.check(lineage_graph.model_uri(project, "no_such_model") == nil, "lineage graph: no model, no uri")

  local served = H.exec_cmd(client, bufnr, lineage_graph.command, {
    uri = customers,
    column = "customer_id",
    direction = "both",
    depth = 3,
  }, "lineage graph: customers.customer_id", 20000)
  if not H.check(type(served) == "table", "lineage graph: an answer", vim.inspect(served)) then
    return
  end
  local ids = {}
  for _, node in ipairs(served.nodes) do
    ids[node.id] = node
  end
  H.check(
    served.start == "model:customers::customer_id",
    "lineage graph: starts on the asked column",
    served.start
  )
  H.check(
    ids["model:stg_customers::customer_id"] ~= nil,
    "lineage graph: upstream reaches the staging column",
    vim.inspect(vim.tbl_keys(ids))
  )
  H.check(
    vim.iter(served.edges):all(function(e)
      return ids[e.from] ~= nil and ids[e.to] ~= nil and (e.kind == "direct" or e.kind == "derived")
    end),
    "lineage graph: every edge joins two nodes of the answer and says how",
    vim.inspect(served.edges):sub(1, 600)
  )
  H.check(
    vim.iter(served.edges):any(function(e)
      return e.from == "model:stg_customers::customer_id" and e.to == "model:customers::customer_id"
    end),
    "lineage graph: the staging column feeds the mart's",
    vim.inspect(served.edges):sub(1, 600)
  )

  -- fetch: the start model's uri, and (err, result) in the callback
  local fetched
  lineage_graph.fetch(bufnr, { uri = customers, column = "customer_id", depth = 3 }, function(err, result)
    fetched = { err = err, result = result }
  end)
  H.wait("lineage graph: fetch answers", 20000, function()
    return fetched ~= nil
  end)
  H.check(
    fetched
      and fetched.err == nil
      and fetched.result
      and fetched.result.start == "model:customers::customer_id",
    "lineage graph: fetch hands (nil, result) for the asked model",
    vim.inspect(fetched):sub(1, 300)
  )
end
