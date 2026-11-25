--- Drawing the picker: the parent, current and preview columns, the
--- breadcrumb header and the key-hint footer, from the state it is given.

local match = require("cogitator.picker.match")
local valid = require("cogitator.picker.window").valid

local M = {}

M.NS = vim.api.nvim_create_namespace("cogitator.picker")
local NS = M.NS

M.KEYS = {
  { "j  k", "move down / up (counts work: 3j)" },
  { "l  <CR>", "enter a group, or choose the item" },
  { "h  <BS>", "back to the parent level" },
  { "gg  G", "first / last item" },
  { "<C-d>  <C-u>", "half a page down / up" },
  { "J  K", "scroll the preview" },
  { "/", "filter as you type (<Esc> clears it)" },
  { "?", "these keys" },
  { "q  <Esc>", "close" },
}

--- The status bar's hints: key, what it does.
local HINTS = {
  { "h", "back" },
  { "j/k", "move" },
  { "l", "open" },
  { "/", "filter" },
  { "J/K", "preview" },
  { "?", "keys" },
  { "q", "quit" },
}

local function set_lines(buf, lines)
  vim.bo[buf].modifiable = true
  vim.api.nvim_buf_set_lines(buf, 0, -1, false, lines)
  vim.bo[buf].modifiable = false
end

local function draw_list(buf, items, width, active, empty)
  local lines, all_marks = {}, {}
  for i, item in ipairs(items) do
    local line, marks = match.format_line(item, width)
    lines[i], all_marks[i] = line, marks
  end
  if #lines == 0 then
    lines = { "  " .. (empty or "· nothing here ·") }
  end
  set_lines(buf, lines)
  vim.api.nvim_buf_clear_namespace(buf, NS, 0, -1)
  for row, marks in ipairs(all_marks) do
    for _, m in ipairs(marks) do
      vim.api.nvim_buf_set_extmark(buf, NS, row - 1, m.col, { end_col = m.stop, hl_group = m.hl })
    end
  end
  if #items == 0 then
    vim.api.nvim_buf_set_extmark(
      buf,
      NS,
      0,
      0,
      { end_row = 0, end_col = #lines[1], hl_group = "DbtPickerDetail" }
    )
  end
  if active then
    vim.api.nvim_buf_set_extmark(buf, NS, active - 1, 0, { line_hl_group = "DbtPickerParent", priority = 10 })
  end
end

local function level(s)
  return s.stack[#s.stack]
end

local function current_item(s)
  local l = level(s)
  return l.shown[l.cursor]
end

--- The preview pane's content for an item.
---@return string[] lines
---@return string? syntax
local function preview_of(s, item)
  if s.help then
    local lines = { "  keys", "" }
    for _, k in ipairs(M.KEYS) do
      lines[#lines + 1] = ("  %-14s %s"):format(k[1], k[2])
    end
    return lines
  end
  if not item then
    return { "" }
  end
  local fn = item.preview or (not item.children and s.opts.preview) or nil
  if fn then
    local ok, out = pcall(fn, item)
    if not ok then
      return { "preview failed: " .. tostring(out) }
    end
    if type(out) == "table" and out.lines then
      return out.lines, out.syntax
    end
    return out or { "" }
  end
  local lines = { "  " .. item.label }
  if item.detail then
    lines[#lines + 1] = "  " .. item.detail
  end
  if item.disabled then
    lines[#lines + 1] = ""
    lines[#lines + 1] = "  unavailable: " .. item.disabled
  end
  return lines
end

local function breadcrumb(s)
  local parts = {}
  for _, l in ipairs(s.stack) do
    parts[#parts + 1] = l.label
  end
  local l = level(s)
  local text = " " .. table.concat(parts, "  ›  ")
  if l.filter ~= "" then
    text = text .. "   /" .. l.filter
  end
  local count = ("%d/%d "):format(#l.shown == 0 and 0 or l.cursor, #l.shown)
  return text, count
end

--- Redraw all panes from the state `s` (nil: nothing is open).
function M.render(s)
  if not s or not valid(s.wins.current) then
    return
  end
  local l = level(s)
  local g = s.geom
  -- parent column: the level above with the entered group lit
  local parent = s.stack[#s.stack - 1]
  if parent then
    draw_list(s.bufs.parent, parent.shown, g.parent_w, parent.cursor)
    pcall(vim.api.nvim_win_set_cursor, s.wins.parent, { math.max(1, parent.cursor), 0 })
  else
    draw_list(
      s.bufs.parent,
      { { label = s.opts.title, icon = "◆", icon_hl = "DbtPickerTitle", hl = "DbtPickerTitle" } },
      g.parent_w,
      1
    )
  end
  -- current column
  draw_list(s.bufs.current, l.shown, g.current_w, nil, l.filter ~= "" and "· no match ·" or nil)
  if #l.shown > 0 then
    vim.api.nvim_win_set_cursor(s.wins.current, { l.cursor, 0 })
  end
  -- preview column: a group shows its children, a leaf its preview
  local item = current_item(s)
  vim.bo[s.bufs.preview].syntax = ""
  if item and item.children and not s.help then
    draw_list(s.bufs.preview, match.visible(item.children, l.filter), g.preview_w)
  else
    local lines, syntax = preview_of(s, item)
    vim.api.nvim_buf_clear_namespace(s.bufs.preview, NS, 0, -1)
    set_lines(s.bufs.preview, lines)
    if syntax then
      vim.bo[s.bufs.preview].syntax = syntax
    end
  end
  pcall(vim.api.nvim_win_set_cursor, s.wins.preview, { 1, 0 })
  -- header and footer
  local text, count = breadcrumb(s)
  local pad = math.max(1, g.width - vim.fn.strdisplaywidth(text) - #count)
  set_lines(s.bufs.header, { text .. string.rep(" ", pad) .. count })
  vim.api.nvim_buf_clear_namespace(s.bufs.header, NS, 0, -1)
  vim.api.nvim_buf_set_extmark(s.bufs.header, NS, 0, 0, { end_col = #text, hl_group = "DbtPickerTitle" })
  local bar, marks = " ", {}
  for _, h in ipairs(HINTS) do
    marks[#marks + 1] = { #bar, #bar + #h[1] }
    bar = bar .. h[1] .. " " .. h[2] .. "   "
  end
  set_lines(s.bufs.status, { bar })
  vim.api.nvim_buf_clear_namespace(s.bufs.status, NS, 0, -1)
  vim.api.nvim_buf_set_extmark(s.bufs.status, NS, 0, 0, { end_col = #bar, hl_group = "DbtPickerDetail" })
  for _, m in ipairs(marks) do
    vim.api.nvim_buf_set_extmark(
      s.bufs.status,
      NS,
      0,
      m[1],
      { end_col = m[2], hl_group = "DbtPickerKey", priority = 200 }
    )
  end
end

return M
