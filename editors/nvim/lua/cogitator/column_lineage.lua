--- Column lineage across models. dbt.columnLineage answers one hop for
--- one model (each output column ← the model/source columns it reads);
--- this walks it upstream through the models it names and downstream
--- through the project graph's children, and returns the result as a
--- {nodes, edges} payload the DAG view draws like the model graph.
---
--- The graph assembly is pure (`graph`, `ref_node`, `consumers`); only
--- `walk` does I/O, through the `fetch` function it is handed, one
--- request at a time and at most `max_requests` of them.

local dag_data = require("cogitator.dag_data")

local M = {}

---@class cogitator.column_lineage.Node: cogitator.layout.Node
---@field model string? the owning model's name (model columns only)
---@field column string

--- The column node for one entry of dbt.columnLineage's `sources`.
---@param ref {kind: "model"|"source"|"unknown", name?: string, source?: string, table?: string, column: string}
---@param by_id table<string, table> project-graph nodes by id
---@return cogitator.column_lineage.Node
function M.ref_node(ref, by_id)
  if ref.kind == "model" then
    return M.model_column(ref.name, ref.column, by_id)
  elseif ref.kind == "source" then
    local owner = by_id["source:" .. ref.source]
    return {
      id = ("source:%s.%s::%s"):format(ref.source, ref.table, ref.column),
      name = ("%s.%s.%s"):format(ref.source, ref.table, ref.column),
      kind = "source",
      uri = owner and owner.uri or vim.NIL,
      tags = {},
      columns = {},
      column = ref.column,
    }
  end
  return {
    id = ("unknown:%s::%s"):format(ref.name, ref.column),
    name = ("%s.%s"):format(ref.name, ref.column),
    kind = "unknown",
    uri = vim.NIL,
    tags = {},
    columns = {},
    column = ref.column,
  }
end

--- The column node of a model's column.
---@param model string
---@param column string
---@param by_id table<string, table>
---@return cogitator.column_lineage.Node
function M.model_column(model, column, by_id)
  local owner = by_id["model:" .. model]
  return {
    id = ("model:%s::%s"):format(model, column),
    name = ("%s.%s"):format(model, column),
    kind = "model",
    uri = owner and owner.uri or vim.NIL,
    tags = {},
    columns = {},
    model = model,
    column = column,
  }
end

--- The output columns of a lineage that read `model.column`.
---@param lineage {columns: {name: string, sources: table[]}[]}?
---@param model string
---@param column string
---@return string[]
function M.consumers(lineage, model, column)
  local found = {}
  for _, entry in ipairs(lineage and lineage.columns or {}) do
    for _, ref in ipairs(entry.sources or {}) do
      if ref.kind == "model" and ref.name == model and ref.column == column then
        found[#found + 1] = entry.name
        break
      end
    end
  end
  return found
end

--- A growing graph: add nodes and edges once each, then `result()`.
---@return table
function M.graph()
  local g = { nodes = {}, order = {}, edges = {}, seen = {} }
  function g.node(node)
    if not g.nodes[node.id] then
      g.nodes[node.id] = node
      g.order[#g.order + 1] = node.id
    end
    return node.id
  end
  function g.edge(from, to)
    local key = from .. "\0" .. to
    if not g.seen[key] then
      g.seen[key] = true
      g.edges[#g.edges + 1] = { from = from, to = to }
    end
  end
  function g.result()
    local nodes = {}
    for _, id in ipairs(g.order) do
      nodes[#nodes + 1] = g.nodes[id]
    end
    return { nodes = dag_data.relayer(nodes, g.edges), edges = g.edges }
  end
  return g
end

---@class cogitator.column_lineage.Opts
---@field depth integer hops to follow each way (at least 1)
---@field max_requests integer dbt.columnLineage calls at most

---@class cogitator.column_lineage.Result
---@field data {nodes: table[], edges: table[]} the payload to draw
---@field start string? the id to select first
---@field requests integer how many lineages were fetched
---@field truncated boolean the request budget ran out
---@field missing string[] models whose lineage could not be read

--- Walk the column lineage around `start` and hand the graph to `done`.
---@param project {nodes: table[], edges: {from: string, to: string}[]} dbt.projectGraph
---@param start {model: string, column?: string} no column = every column
---@param fetch fun(uri: string, cb: fun(lineage: table?))
---@param opts cogitator.column_lineage.Opts
---@param done fun(result: cogitator.column_lineage.Result)
function M.walk(project, start, fetch, opts, done)
  local by_id, children = {}, {}
  for _, node in ipairs(project.nodes) do
    by_id[node.id] = node
  end
  for _, edge in ipairs(project.edges) do
    children[edge.from] = children[edge.from] or {}
    table.insert(children[edge.from], edge.to)
  end

  local g = M.graph()
  local cache, missing = {}, {}
  local requests, truncated = 0, false
  local tasks, queued, head = {}, {}, 1
  local start_id

  local function push(task)
    local key = table.concat({ task.dir, task.model, task.column or "*" }, "|")
    if not queued[key] then
      queued[key] = true
      tasks[#tasks + 1] = task
    end
  end

  --- The lineage of a model (cached), nil when it has none or no file.
  local function lineage_of(model, cb)
    if cache[model] ~= nil then
      return cb(cache[model] or nil)
    end
    local node = by_id["model:" .. model]
    local uri = node and node.uri
    if not uri or uri == vim.NIL or uri == "" then
      cache[model] = false
      return cb(nil)
    end
    if requests >= opts.max_requests then
      truncated = true
      return cb(nil)
    end
    requests = requests + 1
    fetch(uri, function(lineage)
      local ok = type(lineage) == "table" and type(lineage.columns) == "table"
      cache[model] = ok and lineage or false
      if not ok then
        missing[#missing + 1] = model
      end
      cb(ok and lineage or nil)
    end)
  end

  local run_next

  local function upstream(task, lineage)
    for _, entry in ipairs(lineage and lineage.columns or {}) do
      if task.column == nil or entry.name == task.column then
        local here = g.node(M.model_column(task.model, entry.name, by_id))
        if task.dir == "start" then
          start_id = start_id or here
          push({ dir = "down", model = task.model, column = entry.name, depth = opts.depth })
        end
        for _, ref in ipairs(entry.sources or {}) do
          g.edge(g.node(M.ref_node(ref, by_id)), here)
          if ref.kind == "model" and task.depth > 1 then
            push({ dir = "up", model = ref.name, column = ref.column, depth = task.depth - 1 })
          end
        end
      end
    end
  end

  local function downstream(task)
    local from = g.node(M.model_column(task.model, task.column, by_id))
    local kids = vim.tbl_filter(function(id)
      return by_id[id] and by_id[id].kind == "model"
    end, children["model:" .. task.model] or {})
    local i = 0
    local function next_child()
      i = i + 1
      if i > #kids then
        return run_next()
      end
      local child = by_id[kids[i]].name
      lineage_of(child, function(lineage)
        for _, name in ipairs(M.consumers(lineage, task.model, task.column)) do
          g.edge(from, g.node(M.model_column(child, name, by_id)))
          if task.depth > 1 then
            push({ dir = "down", model = child, column = name, depth = task.depth - 1 })
          end
        end
        next_child()
      end)
    end
    next_child()
  end

  run_next = function()
    local task = tasks[head]
    if not task then
      if start.column and not start_id then
        -- a column the lineage does not list (select *, a declined
        -- template) still gets its node and its consumers
        start_id = g.node(M.model_column(start.model, start.column, by_id))
        push({ dir = "down", model = start.model, column = start.column, depth = opts.depth })
        return run_next()
      end
      return done({
        data = g.result(),
        start = start_id,
        requests = requests,
        truncated = truncated,
        missing = missing,
      })
    end
    head = head + 1
    if task.dir == "down" then
      return downstream(task)
    end
    lineage_of(task.model, function(lineage)
      upstream(task, lineage)
      run_next()
    end)
  end

  push({ dir = "start", model = start.model, column = start.column, depth = opts.depth })
  run_next()
end

return M
