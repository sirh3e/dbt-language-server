--- DAG colours as extmarks. Node and edge colours are painted once per
--- layout — a large graph in slices, so the editor never waits for it;
--- the selection is a separate namespace on top, so moving it never
--- rebuilds the grid.

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

--- A graph with more ranges than this paints in slices, so the editor
--- answers keys while a large DAG is being coloured.
M.SYNC_LIMIT = 2000
--- How long one slice may take before it yields to the event loop.
M.SLICE_MS = 4

--- buffer → the generation of the paint that may still write to it; an
--- older paint stops at its next slice
---@type table<integer, integer>
M._generation = {}

--- Stop a paint in progress on `buf` (a newer redraw or the view closing).
---@param buf integer
function M.cancel(buf)
  M._generation[buf] = (M._generation[buf] or 0) + 1
end

--- Paint every node and edge without making the editor wait: a small
--- graph as `static` does, a large one in slices of `SLICE_MS`, the lines
--- `win` shows first. `on_done` runs once everything is painted (not when
--- a newer paint took over).
---@param buf integer
---@param win integer?
---@param boxes table<string, cogitator.layout.Box>
---@param decor cogitator.canvas.Decor
---@param on_done fun()?
function M.static_async(buf, win, boxes, decor, on_done)
  M.cancel(buf)
  local jobs = {}
  for id, ranges in pairs(decor.boxes) do
    local group = node_group(boxes[id].node)
    for _, r in ipairs(ranges or {}) do
      jobs[#jobs + 1] = { r, group }
    end
  end
  for _, ranges in pairs(decor.edges) do
    for _, r in ipairs(ranges or {}) do
      jobs[#jobs + 1] = { r, "DbtDagEdge" }
    end
  end
  if #jobs <= M.SYNC_LIMIT then
    M.static(buf, boxes, decor)
    if on_done then
      on_done()
    end
    return
  end
  vim.api.nvim_buf_clear_namespace(buf, STATIC_NS, 0, -1)
  -- what is on screen first
  if win and vim.api.nvim_win_is_valid(win) then
    local top, bottom = vim.api.nvim_win_call(win, function()
      return vim.fn.line("w0") - 1, vim.fn.line("w$") - 1
    end)
    local seen, rest = {}, {}
    for _, job in ipairs(jobs) do
      local row = job[1][1]
      if row >= top and row <= bottom then
        seen[#seen + 1] = job
      else
        rest[#rest + 1] = job
      end
    end
    vim.list_extend(seen, rest)
    jobs = seen
  end
  local generation = M._generation[buf]
  local next_job = 1
  local function slice()
    if M._generation[buf] ~= generation or not vim.api.nvim_buf_is_valid(buf) then
      return
    end
    local deadline = vim.uv.hrtime() + M.SLICE_MS * 1e6
    while next_job <= #jobs do
      local job = jobs[next_job]
      next_job = next_job + 1
      vim.api.nvim_buf_set_extmark(buf, STATIC_NS, job[1][1], job[1][2], {
        end_col = job[1][3],
        hl_group = job[2],
        strict = false,
      })
      if next_job % 64 == 0 and vim.uv.hrtime() > deadline then
        break
      end
    end
    if next_job <= #jobs then
      vim.defer_fn(slice, 0)
    elseif on_done then
      on_done()
    end
  end
  slice()
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
