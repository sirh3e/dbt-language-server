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

  -- the drawing: the same picture the server's dbt.dagText draws of the same
  -- graph (crates/cogitator-analysis/src/features/dag_text, snapshot diamond)
  local layout = require("cogitator.layout")
  local function drawn(names, edges)
    local nodes, es = {}, {}
    for _, n in ipairs(names) do
      nodes[#nodes + 1] = { id = n, name = n, kind = "model", tags = {}, columns = {} }
    end
    for _, e in ipairs(edges) do
      es[#es + 1] = { from = e[1], to = e[2] }
    end
    local data = { nodes = dag_data.relayer(nodes, es), edges = es }
    local boxes, w, h = layout.layout(data, {}, false)
    local lines = layout.draw(boxes, data.edges, w + 2, h + 1)
    while #lines > 0 and lines[#lines] == "" do
      table.remove(lines)
    end
    return lines
  end
  local diamond = drawn(
    { "src", "left", "right", "mart" },
    { { "src", "left" }, { "src", "right" }, { "left", "mart" }, { "right", "mart" } }
  )
  H.check(
    vim.deep_equal(diamond, {
      "",
      " ╭─────╮       ╭──────╮        ╭──────╮",
      " │ src │───┼──▸│ left │────┼──▸│ mart │",
      " ╰─────╯   │   ╰──────╯    │   ╰──────╯",
      "           │               │",
      "           │   ╭───────╮   │",
      "           ╰──▸│ right │───╯",
      "               ╰───────╯",
    }),
    "layout: the diamond draws as dbt.dagText draws it",
    table.concat(diamond, "\n")
  )
  -- a cycle's nodes sit one layer past an empty one: they are still drawn
  local looped =
    table.concat(drawn({ "root", "a", "b" }, { { "root", "a" }, { "a", "b" }, { "b", "a" } }), "\n")
  H.check(
    looped:find("│ a │", 1, true) and looped:find("│ b │", 1, true),
    "layout: a cycle past an empty layer is drawn",
    looped
  )
end
