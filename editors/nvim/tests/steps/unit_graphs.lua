-- Pure graph transforms: relayering and focus — no server involved. (The
-- column-lineage walk is the server's: dbt.columnLineageGraph.)

return function(ctx)
  local H = ctx.H
  local dag_data = require("cogitator.dag_data")

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
end
