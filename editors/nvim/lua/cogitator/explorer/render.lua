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
--- Side cursors survive a refresh of the same model and reset otherwise.
---@param s cogitator.explorer.State
function M.sides(s)
  local same_model = s.sides_for == s.selected
  local info = s.info
  local panes = { left = info and info.upstream, right = info and info.downstream }
  for pane, entries in pairs(panes) do
    local win = s.wins[pane]
    local row = (same_model and vim.api.nvim_win_is_valid(win)) and vim.api.nvim_win_get_cursor(win)[1] or 1
    render_lib.fill_pane(s.bufs[pane], entries)
    if vim.api.nvim_win_is_valid(win) then
      vim.api.nvim_win_set_cursor(win, { math.min(row, vim.api.nvim_buf_line_count(s.bufs[pane])), 0 })
    end
  end
  s.sides_for = s.selected
end

--- The center title: match count or the live filter, then the trail of
--- models walked to get here (the history), yazi's breadcrumb.
---@param s cogitator.explorer.State
---@return string
function M.center_title(s)
  local what = s.show_all and "nodes" or "models"
  local title = ("%s %d/%d"):format(what, #s.filtered, #s.nodes)
  if s.filter ~= "" then
    title = ("%s /%s"):format(what, s.filter)
  end
  local marked = vim.tbl_count(s.marked or {})
  if marked > 0 then
    title = title .. (" · %d marked"):format(marked)
  end
  local trail = {}
  local back = s.history and s.history.back or {}
  for i = math.max(1, #back - 2), #back do
    trail[#trail + 1] = back[i]
  end
  if #trail > 0 and s.selected then
    trail[#trail + 1] = s.selected
    title = title .. " · " .. table.concat(trail, " › ")
  end
  return " " .. title .. " "
end

local NS = vim.api.nvim_create_namespace("cogitator.explorer.center")
vim.api.nvim_set_hl(0, "CogitatorExplorerMatch", { default = true, link = "Special" })
vim.api.nvim_set_hl(0, "CogitatorExplorerMark", { default = true, link = "WarningMsg" })

--- Redraw the center list for the current filter state: a mark column,
--- the matched characters highlighted.
---@param s cogitator.explorer.State
---@param keep_cursor boolean? stay on the same row (a mark toggled)
function M.center(s, keep_cursor)
  local lines = {}
  local marked = s.marked or {}
  for _, n in ipairs(s.filtered) do
    local kind = (s.show_all and n.kind ~= "model") and ("  [" .. n.kind .. "]") or ""
    lines[#lines + 1] = (marked[n.name] and "● " or "  ") .. n.name .. kind
  end
  if #lines == 0 then
    lines = { "· no match ·" }
  end
  local row = keep_cursor and vim.api.nvim_win_get_cursor(s.wins.center)[1] or 1
  ui.set_lines(s.bufs.center, lines)
  vim.api.nvim_buf_clear_namespace(s.bufs.center, NS, 0, -1)
  for i, n in ipairs(s.filtered) do
    if marked[n.name] then
      vim.api.nvim_buf_set_extmark(
        s.bufs.center,
        NS,
        i - 1,
        0,
        { end_col = 2, hl_group = "CogitatorExplorerMark", strict = false }
      )
    end
    for _, pos in ipairs((s.positions or {})[n.name] or {}) do
      local start = vim.str_byteindex(n.name, "utf-32", pos, false) + 2
      local stop = vim.str_byteindex(n.name, "utf-32", pos + 1, false) + 2
      vim.api.nvim_buf_set_extmark(s.bufs.center, NS, i - 1, start, {
        end_col = stop,
        hl_group = "CogitatorExplorerMatch",
        strict = false,
      })
    end
  end
  vim.api.nvim_win_set_cursor(s.wins.center, { math.min(row, #lines), 0 })
  ui.retitle(s.wins.center, M.center_title(s))
end

return M
