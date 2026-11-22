--- Payload transforms for the DAG view: give any {nodes, edges} graph
--- its layers, and cut a graph down to the neighbourhood of one node.
--- Pure — tables in, new tables out; nothing here touches a buffer.

local M = {}

--- Copy `nodes` with `layer` set to the longest path from a root (0 for
--- roots). Nodes on a cycle, which no topological order reaches, go one
--- layer past the deepest other node, so a cycle never hangs the layout.
---@param nodes table[] each with an `id`
---@param edges {from: string, to: string}[] parent → child
---@return table[] nodes the same nodes (shallow copies) with `layer`
function M.relayer(nodes, edges)
  local known, indegree, children = {}, {}, {}
  for _, node in ipairs(nodes) do
    known[node.id] = true
    indegree[node.id] = 0
  end
  for _, edge in ipairs(edges) do
    if known[edge.from] and known[edge.to] and edge.from ~= edge.to then
      indegree[edge.to] = indegree[edge.to] + 1
      children[edge.from] = children[edge.from] or {}
      table.insert(children[edge.from], edge.to)
    end
  end
  local layer, queue = {}, {}
  for _, node in ipairs(nodes) do
    if indegree[node.id] == 0 then
      layer[node.id] = 0
      queue[#queue + 1] = node.id
    end
  end
  local head, deepest = 1, 0
  while head <= #queue do
    local id = queue[head]
    head = head + 1
    for _, child in ipairs(children[id] or {}) do
      layer[child] = math.max(layer[child] or 0, layer[id] + 1)
      deepest = math.max(deepest, layer[child])
      indegree[child] = indegree[child] - 1
      if indegree[child] == 0 then
        queue[#queue + 1] = child
      end
    end
  end
  local out = {}
  for _, node in ipairs(nodes) do
    local copy = vim.deepcopy(node)
    copy.layer = indegree[node.id] == 0 and layer[node.id] or (deepest + 1)
    out[#out + 1] = copy
  end
  return out
end

--- The part of a graph within `depth` hops of `id`, upstream and
--- downstream (not sideways: a parent's other children stay out), with
--- fresh layers. An unknown id gives the whole graph back unchanged.
---@param data {nodes: table[], edges: {from: string, to: string}[]}
---@param id string
---@param depth integer hops each way, at least 1
---@return {nodes: table[], edges: {from: string, to: string}[]}
function M.focus(data, id, depth)
  local exists = false
  for _, node in ipairs(data.nodes) do
    exists = exists or node.id == id
  end
  if not exists then
    return data
  end
  local up, down = {}, {}
  for _, edge in ipairs(data.edges) do
    up[edge.to] = up[edge.to] or {}
    table.insert(up[edge.to], edge.from)
    down[edge.from] = down[edge.from] or {}
    table.insert(down[edge.from], edge.to)
  end
  local keep = { [id] = true }
  for _, links in ipairs({ up, down }) do
    local frontier = { id }
    for _ = 1, math.max(1, depth) do
      local next_frontier = {}
      for _, at in ipairs(frontier) do
        for _, other in ipairs(links[at] or {}) do
          if not keep[other] then
            keep[other] = true
            next_frontier[#next_frontier + 1] = other
          end
        end
      end
      frontier = next_frontier
    end
  end
  local nodes, edges = {}, {}
  for _, node in ipairs(data.nodes) do
    if keep[node.id] then
      nodes[#nodes + 1] = node
    end
  end
  for _, edge in ipairs(data.edges) do
    if keep[edge.from] and keep[edge.to] then
      edges[#edges + 1] = edge
    end
  end
  return { nodes = M.relayer(nodes, edges), edges = edges }
end

return M
