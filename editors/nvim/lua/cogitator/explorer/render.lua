--- Explorer geometry and pane rendering: pure drawing over the state,
--- no LSP traffic here.

local render_lib = require("cogitator.render")
local ui = require("cogitator.ui")

local M = {}

---@class cogitator.explorer.Geometry
---@field row integer
---@field col integer
---@field side integer side-pane width
---@field center integer center-pane width
---@field list_h integer list row count
---@field prev_h integer preview row count
---@field prev_row integer
---@field prev_w integer

--- All float geometry in one place: three columns on top, one
--- full-width preview below.
---@return cogitator.explorer.Geometry
function M.geometry()
  local total = math.min(vim.o.columns - 4, 140)
  local side = math.floor((total - 4) / 4)
  local center = total - 2 * (side + 2)
  local usable = math.max(14, vim.o.lines - 6)
  local list_h = math.max(8, math.floor(usable * 0.55))
  local prev_h = math.max(5, usable - list_h - 2)
  local row = math.max(1, math.floor((vim.o.lines - (list_h + prev_h + 2)) / 2) - 1)
  local col = math.max(0, math.floor((vim.o.columns - total) / 2))
  return {
    row = row,
    col = col,
    side = side,
    center = center,
    list_h = list_h,
    prev_h = prev_h,
    prev_row = row + list_h + 2,
    prev_w = 2 * side + center + 4,
  }
end

--- Fill the upstream/downstream panes from the selection's modelInfo.
---@param s cogitator.explorer.State
function M.sides(s)
  local info = s.info
  ui.set_lines(s.bufs.left, render_lib.pane_lines(info and info.upstream))
  ui.set_lines(s.bufs.right, render_lib.pane_lines(info and info.downstream))
  for _, pane in ipairs({ "left", "right" }) do
    if vim.api.nvim_win_is_valid(s.wins[pane]) then
      vim.api.nvim_win_set_cursor(s.wins[pane], { 1, 0 })
    end
  end
end

--- The center title: match count, or the live filter text.
---@param s cogitator.explorer.State
---@return string
function M.center_title(s)
  local title = (" models %d/%d "):format(#s.filtered, #s.nodes)
  if s.filter ~= "" then
    title = (" models /%s "):format(s.filter)
  end
  return title
end

--- Redraw the center list for the current filter state.
---@param s cogitator.explorer.State
function M.center(s)
  local lines = {}
  for _, n in ipairs(s.filtered) do
    lines[#lines + 1] = n.name
  end
  if #lines == 0 then
    lines = { "· no match ·" }
  end
  ui.set_lines(s.bufs.center, lines)
  vim.api.nvim_win_set_cursor(s.wins.center, { 1, 0 })
  ui.retitle(s.wins.center, M.center_title(s))
end

return M
