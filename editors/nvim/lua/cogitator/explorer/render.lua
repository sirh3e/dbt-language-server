--- Explorer geometry and pane rendering: pure drawing over the state, no
--- LSP traffic here.
---
---   ┌ upstream ┐┌ model ──────┐┌ downstream ┐
---   └──────────┘└─────────────┘└────────────┘
---   ┌ dbt SQL ───────────┐┌ compiled SQL ───┐
---   └────────────────────┘└─────────────────┘

local card = require("cogitator.explorer.card")
local render_lib = require("cogitator.render")
local ui = require("cogitator.ui")

local M = {}

---@class cogitator.explorer.Rect
---@field row integer
---@field col integer
---@field width integer
---@field height integer

--- Every pane's rectangle (inner size; each float adds a one-cell border).
--- Top: upstream | model | downstream, the model wider; bottom: the dbt
--- SQL and the compiled SQL side by side, as wide as the top row together.
---@param columns integer? editor width (default &columns)
---@param lines integer? editor height (default &lines)
---@return table<"left"|"center"|"right"|"raw"|"compiled", cogitator.explorer.Rect>
function M.geometry(columns, lines)
  columns = columns or vim.o.columns
  lines = lines or vim.o.lines
  local total = math.min(columns - 4, 180)
  local side = math.floor((total - 4) / 4)
  local center = total - 2 * (side + 2)
  local usable = math.max(16, lines - 6)
  local top_h = math.max(8, math.floor(usable * 0.45))
  local bottom_h = math.max(6, usable - top_h - 2)
  local row = math.max(0, math.floor((lines - (top_h + bottom_h + 4)) / 2) - 1)
  local col = math.max(0, math.floor((columns - total) / 2))
  local half = math.floor((total - 2) / 2)
  local bottom_row = row + top_h + 2
  return {
    left = { row = row, col = col, width = side, height = top_h },
    center = { row = row, col = col + side + 2, width = center, height = top_h },
    right = { row = row, col = col + side + 2 + center + 2, width = side, height = top_h },
    raw = { row = bottom_row, col = col, width = half, height = bottom_h },
    compiled = { row = bottom_row, col = col + half + 2, width = total - half - 2, height = bottom_h },
  }
end

local MARK_NS = vim.api.nvim_create_namespace("cogitator.explorer.marks")
local CARD_NS = vim.api.nvim_create_namespace("cogitator.explorer.card")
vim.api.nvim_set_hl(0, "CogitatorExplorerMark", { default = true, link = "WarningMsg" })
vim.api.nvim_set_hl(0, "CogitatorExplorerHeading", { default = true, link = "Title" })

--- Fill the upstream/downstream panes from the selection's modelInfo, the
--- marked entries flagged. Side cursors survive a refresh of the same
--- model and reset otherwise.
---@param s cogitator.explorer.State
--- The kinds a side pane leaves out until `.`: tests check a model, they
--- are not its lineage, and the card lists them already.
M.CHECK_KINDS = { test = true, generic_test = true, unit_test = true }

--- A side pane's entries: every neighbour while `.` shows every kind,
--- else the lineage — models, sources, seeds… — with the tests counted.
---@param s cogitator.explorer.State
---@param pane "left"|"right"
---@return cogitator.GraphEntry[] shown
---@return table<string, integer> hidden kind → count
function M.side_entries(s, pane)
  local info = s.info
  local all = info and (pane == "left" and info.upstream or info.downstream) or {}
  local selected = s.selected and s.node_of and s.node_of[s.selected]
  -- a macro's or a test's neighbours are what it is about: all of them
  if s.show_all or (selected and selected.kind and selected.kind ~= "model") then
    return all, {}
  end
  local shown, hidden = {}, {}
  for _, e in ipairs(all) do
    local kind = e.kind or "model"
    if M.CHECK_KINDS[kind] then
      hidden[kind] = (hidden[kind] or 0) + 1
    else
      shown[#shown + 1] = e
    end
  end
  return shown, hidden
end

--- " (1 · +2 tests)": what a side pane lists, and what `.` would add.
---@param count integer
---@param hidden table<string, integer>
---@return string
function M.side_count(count, hidden)
  local kinds = vim.tbl_keys(hidden)
  table.sort(kinds)
  local extra = {}
  for _, kind in ipairs(kinds) do
    local n = hidden[kind]
    local word = kind:gsub("_", " ")
    extra[#extra + 1] = ("+%d %s%s"):format(n, word, n == 1 and "" or "s")
  end
  if #extra == 0 then
    return ("(%d)"):format(count)
  end
  return ("(%d · %s)"):format(count, table.concat(extra, ", "))
end

function M.sides(s)
  local same_model = s.sides_for == s.selected
  local info = s.info
  local panes = {}
  local hidden_of = {}
  for _, pane in ipairs({ "left", "right" }) do
    if info then
      panes[pane], hidden_of[pane] = M.side_entries(s, pane)
    else
      hidden_of[pane] = {}
    end
  end
  for _, pane in ipairs({ "left", "right" }) do
    local entries = panes[pane]
    local win, buf = s.wins[pane], s.bufs[pane]
    if win and vim.api.nvim_win_is_valid(win) then
      local row = same_model and vim.api.nvim_win_get_cursor(win)[1] or 1
      render_lib.fill_pane(buf, entries)
      vim.api.nvim_buf_clear_namespace(buf, MARK_NS, 0, -1)
      for i, e in ipairs(entries or {}) do
        if s.marked[e.name] then
          vim.api.nvim_buf_set_extmark(buf, MARK_NS, i - 1, 0, {
            virt_text = { { "● ", "CogitatorExplorerMark" } },
            virt_text_pos = "inline",
          })
        end
      end
      vim.api.nvim_win_set_cursor(win, { math.min(row, vim.api.nvim_buf_line_count(buf)), 0 })
      local arrow = pane == "left" and "upstream ⯇" or "⯈ downstream"
      ui.retitle(win, (" %s %s "):format(arrow, M.side_count(entries and #entries or 0, hidden_of[pane])))
    end
  end
  s.sides_for = s.selected
end

--- The centre title: what it is, and the trail of models walked to get
--- here (the history), yazi's breadcrumb.
---@param s cogitator.explorer.State
---@return string
function M.center_title(s)
  local node = s.selected and s.node_of[s.selected]
  local title = node and node.kind or "model"
  local marked = vim.tbl_count(s.marked or {})
  local selection = require("cogitator.explorer.select").title(s)
  if selection then
    title = title .. " · " .. selection
  elseif marked > 0 then
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

--- Draw the model card in the centre.
---@param s cogitator.explorer.State
function M.center(s)
  local win, buf = s.wins.center, s.bufs.center
  if not (win and vim.api.nvim_win_is_valid(win)) then
    return
  end
  local view = card.view(s, vim.api.nvim_win_get_width(win))
  s.card_jumps = view.jumps
  local row = vim.api.nvim_win_get_cursor(win)[1]
  ui.set_lines(buf, view.lines)
  vim.api.nvim_buf_clear_namespace(buf, CARD_NS, 0, -1)
  for _, line in ipairs(view.headings) do
    vim.api.nvim_buf_set_extmark(buf, CARD_NS, line - 1, 0, {
      end_row = line,
      hl_group = "CogitatorExplorerHeading",
      strict = false,
    })
  end
  if s.card_for ~= s.selected then
    row = 1
    s.card_for = s.selected
  end
  vim.api.nvim_win_set_cursor(win, { math.min(row, #view.lines), 0 })
  ui.retitle(win, M.center_title(s))
end

return M
