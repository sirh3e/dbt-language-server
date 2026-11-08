--- DAG colours as extmarks. Node and edge colours are painted once per
--- layout; the selection is a separate namespace on top, so moving it
--- never rebuilds the grid.

local highlight = require("cogitator.highlight")

local M = {}

local STATIC_NS = vim.api.nvim_create_namespace("cogitator.dag.static")
local SELECTION_NS = vim.api.nvim_create_namespace("cogitator.dag.selection")
local SELECTION_PRIORITY = 5000

M.STATIC_NS, M.SELECTION_NS = STATIC_NS, SELECTION_NS

local function mark(buf, ns, ranges, group, priority)
  for _, r in ipairs(ranges or {}) do
    vim.api.nvim_buf_set_extmark(buf, ns, r[1], r[2], {
      end_col = r[3],
      hl_group = group,
      priority = priority,
      strict = false,
    })
  end
end

local function node_group(node)
  if node.kind == "model" and (not node.uri or node.uri == vim.NIL) then
    return "DbtDagDim"
  end
  return highlight.group_for(node)
end

--- Paint every node and edge.
---@param buf integer
---@param boxes table<string, cogitator.layout.Box>
---@param decor cogitator.canvas.Decor
function M.static(buf, boxes, decor)
  vim.api.nvim_buf_clear_namespace(buf, STATIC_NS, 0, -1)
  for id, ranges in pairs(decor.boxes) do
    mark(buf, STATIC_NS, ranges, node_group(boxes[id].node))
  end
  for _, ranges in pairs(decor.edges) do
    mark(buf, STATIC_NS, ranges, "DbtDagEdge")
  end
end

--- Paint the selected node and the edges touching it.
---@param buf integer
---@param id string
---@param decor cogitator.canvas.Decor
---@param edge_indexes integer[] indexes into decor.edges
function M.selection(buf, id, decor, edge_indexes)
  vim.api.nvim_buf_clear_namespace(buf, SELECTION_NS, 0, -1)
  for _, i in ipairs(edge_indexes) do
    mark(buf, SELECTION_NS, decor.edges[i], "DbtDagSelectedEdge", SELECTION_PRIORITY)
  end
  mark(buf, SELECTION_NS, decor.boxes[id], "DbtDagSelected", SELECTION_PRIORITY)
end

return M
