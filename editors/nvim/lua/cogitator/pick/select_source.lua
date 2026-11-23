--- dbt selections in a node finder (spec §5.6): `with_selection` makes a
--- source that lists graph nodes also take dbt's selection syntax. While
--- the query is a selection (cogitator.pick.selection), the server
--- evaluates it (dbt.select, as `dbt ls --select … --exclude …`) and the
--- list is its answer: models, seeds, snapshots, sources, tests,
--- exposures… with the counts and any error in the prompt's title.

local actions = require("cogitator.actions")
local preview = require("cogitator.pick.preview")
local selection = require("cogitator.pick.selection")

local M = {}

local TAG = {
  model = "model",
  seed = "seed",
  snapshot = "snapshot",
  source = "source",
  test = "test",
  unit_test = "unit test",
  analysis = "analysis",
  exposure = "exposure",
  metric = "metric",
  semantic_model = "semantic",
}

--- One node of dbt.select's answer as a picker item. `name` is what the
--- navigator focuses: the graph's own name where the node is in
--- dbt.projectGraph (`model:<stem>`), else the node's.
---@param node table
---@return cogitator.pick.Item
function M.item(node)
  local listed = node.version and node.version ~= vim.NIL and (node.name .. ".v" .. node.version) or node.name
  if node.resourceType == "source" and node.graphId and node.graphId ~= vim.NIL then
    listed = node.graphId:gsub("^source:", "") .. "." .. node.name
  end
  local tags = (node.tags and #node.tags > 0) and ("  #" .. table.concat(node.tags, " #")) or ""
  local graph_id = node.graphId ~= vim.NIL and node.graphId or nil
  local range = node.range ~= vim.NIL and node.range or nil
  return {
    text = ("%s  [%s]%s"):format(listed, TAG[node.resourceType] or node.resourceType, tags),
    -- what selects it alone, for marks, yank and run
    selector = node.resourceType == "source" and ("source:" .. listed) or listed,
    name = graph_id and graph_id:match("^[%w_]+:(.+)$") or node.name,
    uri = node.uri ~= vim.NIL and node.uri or nil,
    lnum = range and range.start.line + 1 or nil,
    col = range and range.start.character or nil,
    value = {
      kind = node.resourceType,
      name = node.name,
      unique_id = node.uniqueId,
      graph_id = graph_id,
      in_graph = graph_id ~= nil,
      uri = node.uri ~= vim.NIL and node.uri or nil,
      range = range,
      tags = node.tags,
    },
  }
end

--- What dbt.select answered, as the live picker takes it.
---@param result table?
---@param err table?
---@return {items: cogitator.pick.Item[], status: string, error: table?, ids: string[]}
function M.answer(result, err)
  if err or type(result) ~= "table" then
    return {
      items = {},
      status = "the server did not answer",
      error = { message = err and err.message or "the server did not answer" },
      ids = {},
    }
  end
  local items, ids = {}, {}
  for _, node in ipairs(result.nodes or {}) do
    items[#items + 1] = M.item(node)
    ids[#ids + 1] = node.uniqueId
  end
  local e = result.error ~= vim.NIL and result.error or nil
  return { items = items, status = selection.summary(result.counts), error = e, ids = ids }
end

--- Ask the server: `query` as typed (it splits off an inline --exclude
--- itself, so an error's span is into the text the user sees).
---@param ctx cogitator.pick.Ctx
---@param query string
---@param with_vocabulary boolean
---@param done fun(result: table?, err: table?)
function M.ask(ctx, query, with_vocabulary, done)
  actions.exec_with(ctx.client, ctx.bufnr, "dbt.select", {
    uri = vim.uri_from_bufnr(ctx.bufnr),
    select = query,
    vocabulary = with_vocabulary or nil,
  }, done)
end

--- `source` (a node finder), also taking dbt selections. `on_answer`
--- sees every answer (the navigator keeps the last one's ids).
---@param source cogitator.pick.Source
---@param ctx cogitator.pick.Ctx
---@param on_answer fun(query: string, answer: table)?
---@return cogitator.pick.Source
--- The last vocabulary each client answered, for the command line's
--- completion, which cannot wait for one.
---@type table<integer, table>
M._vocab = {}

--- The vocabulary at hand for `client` (asked for in the background
--- when there is none yet: the next completion has it).
---@param ctx cogitator.pick.Ctx
---@return table?
function M.vocabulary(ctx)
  local id = ctx.client.id
  if not M._vocab[id] then
    M._vocab[id] = false
    M.ask(ctx, "", true, function(_, result)
      if type(result) == "table" and type(result.vocabulary) == "table" then
        M._vocab[id] = result.vocabulary
      else
        M._vocab[id] = nil
      end
    end)
  end
  return M._vocab[id] or nil
end

function M.with_selection(source, ctx, on_answer)
  local vocab
  source.live_title = "dbt select"
  source.is_live = selection.is_selection
  source.live = function(query, done)
    M.ask(ctx, query, vocab == nil, function(err, result)
      if type(result) == "table" and type(result.vocabulary) == "table" then
        vocab = result.vocabulary
        M._vocab[ctx.client.id] = vocab
      end
      local answer = M.answer(result, err)
      if on_answer then
        on_answer(query, answer)
      end
      done(answer)
    end)
  end
  source.complete = function(query, col, done)
    if vocab then
      done(selection.complete(query, col, vocab))
      return
    end
    M.ask(ctx, "", true, function(_, result)
      vocab = type(result) == "table" and type(result.vocabulary) == "table" and result.vocabulary or nil
      done(vocab and selection.complete(query, col, vocab) or nil)
    end)
  end
  local base_preview = source.preview
  source.preview = function(item, done)
    if item.value and item.value.unique_id then
      local v = item.value
      local header = { ("-- %s %s"):format(v.kind:gsub("_", " "), v.unique_id) }
      if v.tags and #v.tags > 0 then
        header[#header + 1] = "-- tags: " .. table.concat(v.tags, ", ")
      end
      preview.file(item, header, done)
    elseif base_preview then
      base_preview(item, done)
    else
      done({ lines = {} })
    end
  end
  return source
end

--- :DbtFind select / :DbtSelect: every node, and dbt selections, with
--- `ctx.query` typed in already.
---@param ctx cogitator.pick.Ctx
function M.source(ctx)
  local source = require("cogitator.pick.sources").nodes(ctx, "nodes")
  source.query = ctx.query
  return M.with_selection(source, ctx)
end

return M
