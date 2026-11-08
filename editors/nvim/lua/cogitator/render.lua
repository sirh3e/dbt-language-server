--- Shared label/line rendering for graph entries and tests — nil- and
--- vim.NIL-safe everywhere (the graph view used to trust `uri`).

local M = {}

---@class cogitator.GraphEntry
---@field name string
---@field kind string model, source, exposure, metric, semantic_model, test or analysis
---@field uri string|vim.NIL|nil null for dangling refs / fileless sources

--- "name  [source]  (missing)" — the shared pane label.
---@param e cogitator.GraphEntry
---@return string
function M.entry_label(e)
  local label = e.name
  if e.kind == "source" then
    label = label .. "  [source]"
  end
  if not e.uri or e.uri == vim.NIL then
    label = label .. "  (missing)"
  end
  return label
end

--- A side pane's lines: labels, or the placeholder when empty.
---@param entries cogitator.GraphEntry[]|nil
---@return string[]
function M.pane_lines(entries)
  if not entries or #entries == 0 then
    return { "· none ·" }
  end
  return vim.tbl_map(M.entry_label, entries)
end

local PANE_NS = vim.api.nvim_create_namespace("cogitator.pane")

--- Fill a side pane with entry labels, each in its resource colour.
---@param buf integer
---@param entries cogitator.GraphEntry[]|nil
function M.fill_pane(buf, entries)
  local lines = M.pane_lines(entries)
  require("cogitator.ui").set_lines(buf, lines)
  vim.api.nvim_buf_clear_namespace(buf, PANE_NS, 0, -1)
  local highlight = require("cogitator.highlight")
  for i, e in ipairs(entries or {}) do
    local missing = not e.uri or e.uri == vim.NIL
    vim.api.nvim_buf_set_extmark(buf, PANE_NS, i - 1, 0, {
      end_col = #lines[i],
      hl_group = (e.kind == "model" and missing) and "DbtDagDim" or (highlight.group_for(e) .. "Fg"),
    })
  end
end

---@class cogitator.ModelTest
---@field name string
---@field column string|vim.NIL|nil
---@field version integer|vim.NIL|nil

--- "unique on customer_id (v2)" — the shared test label.
---@param t cogitator.ModelTest
---@return string
function M.test_label(t)
  local label = t.name
  if t.column and t.column ~= vim.NIL then
    label = label .. " on " .. t.column
  end
  if t.version and t.version ~= vim.NIL then
    label = label .. (" (v%d)"):format(t.version)
  end
  return label
end

return M
