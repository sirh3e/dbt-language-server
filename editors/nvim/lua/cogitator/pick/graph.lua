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
  -- a properties file: the entry the cursor is in documents a node
  if vim.bo[bufnr].filetype == "yaml" then
    local win = vim.fn.bufwinid(bufnr)
    local row = win ~= -1 and vim.api.nvim_win_get_cursor(win)[1] or 1
    local name = M.yaml_entry(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false), row)
    for _, node in ipairs(name and graph.nodes or {}) do
      if node.name == name then
        return node
      end
    end
  end
  return nil
end

local ENTRY_LISTS = { models = true, seeds = true, snapshots = true, analyses = true, exposures = true }

--- The entry of a properties file that line `row` (1-based) is in: the
--- `- name: x` of the `models:` (seeds:, snapshots:…) list above it, at
--- that list's own indentation, so a column's `- name:` is not taken for
--- the model's.
---@param lines string[]
---@param row integer
---@return string?
function M.yaml_entry(lines, row)
  -- the list the cursor is in, and the indentation of its items
  local list_row
  for r = math.min(row, #lines), 1, -1 do
    local key = lines[r]:match("^([%w_]+):%s*$")
    if key then
      if ENTRY_LISTS[key] then
        list_row = r
      end
      break
    end
  end
  if not list_row then
    return nil
  end
  local item_indent
  for r = list_row + 1, #lines do
    local indent = lines[r]:match("^(%s*)%- ")
    if indent then
      item_indent = #indent
      break
    end
  end
  for r = math.min(row, #lines), list_row + 1, -1 do
    local indent, name = lines[r]:match("^(%s*)%-%s+name:%s*[\"']?([%w_%.%-]+)[\"']?%s*$")
    if indent and #indent == item_indent then
      return name
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
