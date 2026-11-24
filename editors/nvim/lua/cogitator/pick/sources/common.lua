--- What the sources share: a graph node as an item and its preview, and
--- a source of the graph's nodes of some kinds.

local graph_lib = require("cogitator.pick.graph")
local paint = require("cogitator.pick.paint")
local preview = require("cogitator.pick.preview")

local M = {}

local KIND_TAG = {
  model = "model",
  source = "source",
  exposure = "exposure",
  metric = "metric",
  semantic_model = "semantic",
  test = "test",
  analysis = "analysis",
}

--- A graph node as a picker item: its name in its kind's colour, the
--- kind, its tags. `prefix` parts (an arrow and a depth) go before it.
---@param node table
---@param label string?
---@param prefix {[1]: string, [2]: string?}[]?
function M.node_item(node, label, prefix)
  local uri = node.uri ~= vim.NIL and node.uri or nil
  local parts = vim.list_extend(
    vim.deepcopy(prefix or {}),
    paint.node_parts(label or node.name, node.kind, node.tags, uri, KIND_TAG[node.kind])
  )
  return paint.paint({
    name = node.name,
    uri = uri,
    value = node,
  }, parts)
end

--- A node's file, under a header saying what it is.
function M.node_preview(item, done)
  local node = item.value
  local header = { ("-- %s %s"):format(node.kind, node.name) }
  if node.tags and #node.tags > 0 then
    header[#header + 1] = "-- tags: " .. table.concat(node.tags, ", ")
  end
  if node.columns and #node.columns > 0 then
    local names = {}
    for _, c in ipairs(node.columns) do
      names[#names + 1] = c.name
    end
    header[#header + 1] = "-- columns: " .. table.concat(names, ", ")
  end
  preview.file(item, header, done)
end

--- Nodes of the kinds in `kinds` (all when nil).
---@param ctx cogitator.pick.Ctx
---@param title string
---@param kinds table<string, boolean>?
function M.nodes(ctx, title, kinds)
  return {
    title = title,
    empty = "no " .. title .. " in this project yet — is the project loaded?",
    items = function(done)
      graph_lib.get(ctx.client, ctx.bufnr, function(graph)
        local items = {}
        for _, node in ipairs(graph and graph.nodes or {}) do
          if not kinds or kinds[node.kind] then
            items[#items + 1] = M.node_item(node)
          end
        end
        table.sort(items, function(a, b)
          return a.name < b.name
        end)
        done(items)
      end)
    end,
    preview = M.node_preview,
  }
end

return M
