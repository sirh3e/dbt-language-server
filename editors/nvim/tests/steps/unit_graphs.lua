-- Pure graph transforms: relayering, focus, and the column-lineage walk
-- over a fake dbt.columnLineage — no server involved.

return function(ctx)
  local H = ctx.H
  local dag_data = require("cogitator.dag_data")
  local column_lineage = require("cogitator.column_lineage")

  local function layers(nodes)
    local out = {}
    for _, node in ipairs(nodes) do
      out[node.id] = node.layer
    end
    return out
  end

  local chain = {
    nodes = { { id = "a" }, { id = "b" }, { id = "c" }, { id = "d" }, { id = "x" } },
    edges = {
      { from = "a", to = "b" },
      { from = "b", to = "c" },
      { from = "a", to = "c" },
      { from = "c", to = "d" },
    },
  }
  local l = layers(dag_data.relayer(chain.nodes, chain.edges))
  H.check(
    l.a == 0 and l.b == 1 and l.c == 2 and l.d == 3 and l.x == 0,
    "dag_data: layers are the longest path from a root",
    vim.inspect(l)
  )
  local cyc = layers(dag_data.relayer({ { id = "p" }, { id = "q" }, { id = "r" } }, {
    { from = "p", to = "q" },
    { from = "q", to = "p" },
    { from = "r", to = "r" },
  }))
  H.check(
    cyc.p == 1 and cyc.q == 1 and cyc.r == 0,
    "dag_data: a cycle lands past the deepest layer instead of hanging",
    vim.inspect(cyc)
  )

  local focused = dag_data.focus(chain, "b", 1)
  local kept = vim.tbl_map(function(n)
    return n.id
  end, focused.nodes)
  table.sort(kept)
  H.check(
    vim.deep_equal(kept, { "a", "b", "c" }) and #focused.edges == 3,
    "dag_data: focus keeps one hop up and down, and the edges between",
    vim.inspect(kept)
  )
  H.check(dag_data.focus(chain, "nope", 1) == chain, "dag_data: focus on an unknown id is the whole graph")

  -- raw.t.id → stg.id → customers.customer_id, customers → (no child)
  local project = {
    nodes = {
      { id = "source:raw", name = "raw", kind = "source", uri = "file:///raw.yml" },
      { id = "model:stg", name = "stg", kind = "model", uri = "file:///stg.sql" },
      { id = "model:customers", name = "customers", kind = "model", uri = "file:///customers.sql" },
      { id = "model:seed_like", name = "seed_like", kind = "model", uri = vim.NIL },
    },
    edges = {
      { from = "source:raw", to = "model:stg" },
      { from = "model:stg", to = "model:customers" },
      { from = "model:seed_like", to = "model:customers" },
    },
  }
  local lineages = {
    ["file:///stg.sql"] = {
      columns = {
        { name = "id", sources = { { kind = "source", source = "raw", table = "t", column = "id" } } },
        { name = "unused", sources = {} },
      },
    },
    ["file:///customers.sql"] = {
      columns = {
        {
          name = "customer_id",
          sources = {
            { kind = "model", name = "stg", column = "id" },
            { kind = "model", name = "seed_like", column = "k" },
          },
        },
      },
    },
  }
  local asked = {}
  local function fetch(uri, cb)
    asked[#asked + 1] = uri
    cb(lineages[uri])
  end

  local result
  column_lineage.walk(
    project,
    { model = "stg", column = "id" },
    fetch,
    { depth = 3, max_requests = 10 },
    function(r)
      result = r
    end
  )
  local ids = vim.tbl_map(function(n)
    return n.id
  end, result.data.nodes)
  table.sort(ids)
  local edges = vim.tbl_map(function(e)
    return e.from .. " -> " .. e.to
  end, result.data.edges)
  table.sort(edges)
  H.check(
    vim.deep_equal(ids, {
      "model:customers::customer_id",
      "model:stg::id",
      "source:raw.t::id",
    })
      and vim.deep_equal(edges, {
        "model:stg::id -> model:customers::customer_id",
        "source:raw.t::id -> model:stg::id",
      }),
    "column_lineage: walks upstream to the source and downstream to the consumer",
    vim.inspect({ ids, edges })
  )
  H.check(
    result.start == "model:stg::id",
    "column_lineage: the walk starts on the asked column",
    result.start
  )
  local layer = layers(result.data.nodes)
  H.check(
    layer["source:raw.t::id"] < layer["model:stg::id"]
      and layer["model:stg::id"] < layer["model:customers::customer_id"],
    "column_lineage: columns are layered upstream → downstream",
    vim.inspect(layer)
  )

  column_lineage.walk(project, { model = "customers" }, fetch, { depth = 1, max_requests = 10 }, function(r)
    result = r
  end)
  local names = vim.tbl_map(function(n)
    return n.name
  end, result.data.nodes)
  table.sort(names)
  H.check(
    vim.deep_equal(names, { "customers.customer_id", "seed_like.k", "stg.id" }) and not result.truncated,
    "column_lineage: depth 1 stops at the direct feeders; a fileless model is a leaf",
    vim.inspect(names)
  )

  asked = {}
  column_lineage.walk(project, { model = "stg" }, fetch, { depth = 3, max_requests = 1 }, function(r)
    result = r
  end)
  H.check(
    result.truncated and #asked == 1,
    "column_lineage: the request budget stops the walk and says so",
    vim.inspect({ result.truncated, asked })
  )
end
