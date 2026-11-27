--- dbt.columnLineageGraph: the column lineage around a model column,
--- walked by the server in one request over its project-wide column graph
--- and lineage cache. The answer is a {nodes, edges} payload the DAG view
--- and the column dialog draw (ids `model:<m>::<col>`, `source:<s>.<t>::<col>`,
--- `unknown:<name>::<col>`), plus `start`, `cached`, `stale` (models still
--- being computed), `missing`, edge `kind`s and model columns' `compiledUri`.
--- The plugin walks no lineage itself any more.

local actions = require("cogitator.actions")

local M = {}

M.command = "dbt.columnLineageGraph"

---@class cogitator.lineage_graph.Opts
---@field uri string? the start model's file; default the buffer's
---@field column string? the start column; nil = every column of the model
---@field direction ("up"|"down"|"both")? default "both"
---@field depth integer? hops each way, default 3

---@class cogitator.lineage_graph.Result
---@field nodes table[]
---@field edges {from: string, to: string, kind?: string}[]
---@field start string? the id to select first
---@field cached boolean every lineage came from the server's cache
---@field stale string[]? models whose lineage is still being computed
---@field missing string[] models whose lineage could not be read

--- Whether `client` serves the command.
---@param client vim.lsp.Client?
---@return boolean
function M.supported(client)
  local provider = client and client.server_capabilities and client.server_capabilities.executeCommandProvider
  return provider ~= nil and vim.tbl_contains(provider.commands or {}, M.command)
end

--- The command's argument for `bufnr`'s model (pure).
---@param uri string
---@param opts cogitator.lineage_graph.Opts?
---@return table
function M.argument(uri, opts)
  opts = opts or {}
  return {
    uri = uri,
    column = opts.column,
    direction = opts.direction or "both",
    depth = opts.depth and math.max(1, opts.depth) or nil,
  }
end

--- Asks the server for the graph around `bufnr`'s model; `callback(err,
--- result)` runs once, on the event loop. A `null` answer (no model) is
--- `(nil, nil)`; an unsupported server is an error.
---@param bufnr integer
---@param opts cogitator.lineage_graph.Opts?
---@param callback fun(err: any, result: cogitator.lineage_graph.Result?)
function M.fetch(bufnr, opts, callback)
  local client = actions.client_for(bufnr, true)
  if not M.supported(client) then
    return callback({ message = "cogitator: the server does not serve " .. M.command }, nil)
  end
  ---@cast client vim.lsp.Client
  local arg = M.argument((opts and opts.uri) or vim.uri_from_bufnr(bufnr), opts)
  actions.exec_with(client, bufnr, M.command, arg, function(err, result)
    if err then
      return callback(err, nil)
    end
    callback(nil, result ~= vim.NIL and result or nil)
  end)
end

--- The file of `model` in a dbt.projectGraph payload, nil when it has none.
---@param project {nodes: table[]}
---@param model string
---@return string?
function M.model_uri(project, model)
  for _, node in ipairs(project and project.nodes or {}) do
    if node.id == "model:" .. model and type(node.uri) == "string" and node.uri ~= "" then
      return node.uri
    end
  end
  return nil
end

--- The winbar note for an answer: what was still being computed.
---@param result cogitator.lineage_graph.Result
---@return string?
function M.note(result)
  local stale = type(result.stale) == "table" and #result.stale or 0
  if stale > 0 then
    return ("%d model%s still being computed"):format(stale, stale == 1 and "" or "s")
  end
  return nil
end

return M
