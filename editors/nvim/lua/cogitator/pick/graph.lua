--- dbt.projectGraph, kept per client: the pickers and the navigator read
--- it without a round trip after the first. It is dropped when the server
--- reports finished work ($/progress end), so the next reader asks again.

local actions = require("cogitator.actions")

local M = {}

---@type table<integer, {graph: table}|{waiting: fun(graph: table?)[]}>
M._cache = {}

--- Forget a client's graph (or every client's).
---@param client_id integer?
function M.invalidate(client_id)
  if client_id then
    M._cache[client_id] = nil
  else
    M._cache = {}
  end
end

--- The project graph of `client`, then `done(graph | nil)`.
---@param client vim.lsp.Client
---@param bufnr integer
---@param done fun(graph: table?)
function M.get(client, bufnr, done)
  local entry = M._cache[client.id]
  if entry and entry.graph then
    return done(entry.graph)
  end
  if entry and entry.waiting then
    table.insert(entry.waiting, done)
    return
  end
  entry = { waiting = { done } }
  M._cache[client.id] = entry
  actions.exec_with(
    client,
    bufnr,
    "dbt.projectGraph",
    { uri = vim.uri_from_bufnr(bufnr) },
    function(err, result)
      local graph = (not err and type(result) == "table" and type(result.nodes) == "table") and result or nil
      if M._cache[client.id] == entry then
        M._cache[client.id] = graph and { graph = graph } or nil
      end
      for _, waiter in ipairs(entry.waiting) do
        waiter(graph)
      end
    end
  )
end

--- parents and children of every node id.
---@param graph table
---@return table<string, string[]> parents, table<string, string[]> children
function M.adjacency(graph)
  local parents, children = {}, {}
  for _, e in ipairs(graph.edges or {}) do
    parents[e.to] = parents[e.to] or {}
    table.insert(parents[e.to], e.from)
    children[e.from] = children[e.from] or {}
    table.insert(children[e.from], e.to)
  end
  return parents, children
end

--- The node a buffer's file is, if any.
---@param graph table
---@param bufnr integer
---@return table?
function M.node_of_buffer(graph, bufnr)
  local path = vim.api.nvim_buf_get_name(bufnr)
  if path == "" then
    return nil
  end
  local want = vim.fs.normalize(vim.fn.fnamemodify(path, ":p"))
  for _, node in ipairs(graph.nodes) do
    if node.uri and node.uri ~= vim.NIL and vim.fs.normalize(vim.uri_to_fname(node.uri)) == want then
      return node
    end
  end
  return nil
end

--- Every node reachable from `start` along `edges_of`, nearest first,
--- each with its distance.
---@param start string node id
---@param edges_of table<string, string[]>
---@return {id: string, depth: integer}[]
function M.walk(start, edges_of)
  local out, seen, queue = {}, { [start] = true }, { { id = start, depth = 0 } }
  local head = 1
  while head <= #queue do
    local at = queue[head]
    head = head + 1
    for _, next_id in ipairs(edges_of[at.id] or {}) do
      if not seen[next_id] then
        seen[next_id] = true
        local step = { id = next_id, depth = at.depth + 1 }
        out[#out + 1] = step
        queue[#queue + 1] = step
      end
    end
  end
  return out
end

return M
