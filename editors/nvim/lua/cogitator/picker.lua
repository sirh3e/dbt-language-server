--- A yazi-style picker: three columns — the parent level, the current
--- level, and a preview of what is under the cursor — with a breadcrumb
--- header and a key-hint footer. Walked with vim motions: j/k move,
--- l (or <CR>) enters a group or chooses an item, h goes back up,
--- gg/G jump, / filters as you type, J/K scroll the preview, ? shows the
--- keys, q/<Esc> close. Every dialog of the plugin goes through here:
--- `select` is a drop-in for vim.ui.select, `input` for vim.ui.input,
--- and `open` takes a tree (the code-action menu, the :Dbt menu).
---
--- Pure helpers (`format_line`, `matches`, `visible`) are exposed for
--- the test suite; the rest is window plumbing.

local prompt = require("cogitator.prompt")

local M = {}

local NS = vim.api.nvim_create_namespace("cogitator.picker")

---@class cogitator.picker.Item
---@field label string
---@field icon? string one glyph shown before the label
---@field icon_hl? string highlight of the icon
---@field hl? string highlight of the label
---@field detail? string dim text at the right edge
---@field children? cogitator.picker.Item[] makes the item a group
---@field preview? fun(item: cogitator.picker.Item): (string[]|{lines: string[], syntax?: string})
---@field disabled? string why the item cannot be chosen
---@field value? any

---@class cogitator.picker.Opts
---@field title string the root of the breadcrumb
---@field items cogitator.picker.Item[]
---@field on_choose fun(item: cogitator.picker.Item, path: cogitator.picker.Item[])
---@field on_cancel? fun()
---@field preview? fun(item: cogitator.picker.Item): (string[]|{lines: string[], syntax?: string})
---@field filter? string an initial filter

---@class cogitator.picker.Level
---@field items cogitator.picker.Item[]
---@field label string the breadcrumb part
---@field filter string
---@field shown cogitator.picker.Item[] items after the filter
---@field cursor integer 1-based row in `shown`

---@type table?
M._state = nil

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

--- Does an item (or, for a group, any of its leaves) match every word of
--- the filter? Case-insensitive substring per word.
---@param item cogitator.picker.Item
---@param filter string
---@return boolean
function M.matches(item, filter)
  local words = vim.split(vim.trim(filter):lower(), "%s+", { trimempty = true })
  if #words == 0 then
    return true
  end
  local function hit(it)
    local text = (it.label .. " " .. (it.detail or "")):lower()
    for _, w in ipairs(words) do
      if not text:find(w, 1, true) then
        return false
      end
    end
    return true
  end
  local function walk(it)
    if hit(it) then
      return true
    end
    for _, child in ipairs(it.children or {}) do
      if walk(child) then
        return true
      end
    end
    return false
  end
  return walk(item)
end

--- The items of a level the filter keeps.
---@param items cogitator.picker.Item[]
---@param filter string
---@return cogitator.picker.Item[]
function M.visible(items, filter)
  return vim.tbl_filter(function(it)
    return M.matches(it, filter)
  end, items)
end

--- One list row: " icon label ··· detail ›", padded to `width` cells.
--- Returns the text and the byte ranges to highlight.
---@param item cogitator.picker.Item
---@param width integer
---@return string line
---@return {col: integer, stop: integer, hl: string}[] marks
function M.format_line(item, width)
  local icon = item.icon or (item.children and "▸" or "•")
  local arrow = item.children and " ›" or ""
  local detail = item.detail and (" " .. item.detail) or ""
  local head = " " .. icon .. " "
  local room = width - vim.fn.strdisplaywidth(head) - vim.fn.strdisplaywidth(detail .. arrow) - 1
  local label = item.label
  if vim.fn.strdisplaywidth(label) > room and room > 1 then
    label = vim.fn.strcharpart(label, 0, room - 1) .. "…"
  end
  local pad = math.max(1, room - vim.fn.strdisplaywidth(label) + 1)
  local line = head .. label .. string.rep(" ", pad) .. detail .. arrow
  local marks = {}
  local function mark(col, stop, hl)
    if hl and stop > col then
      marks[#marks + 1] = { col = col, stop = stop, hl = hl }
    end
  end
  mark(
    1,
    1 + #icon,
    item.disabled and "DbtPickerDisabled" or (item.icon_hl or (item.children and "DbtPickerGroup"))
  )
  local lstart = #head
  mark(
    lstart,
    lstart + #label,
    item.disabled and "DbtPickerDisabled" or (item.hl or (item.children and "DbtPickerGroup"))
  )
  local dstart = lstart + #label + pad
  mark(dstart, dstart + #detail, "DbtPickerDetail")
  mark(dstart + #detail, #line, "DbtPickerGroup")
  return line, marks
end

local function valid(win)
  return win and vim.api.nvim_win_is_valid(win)
end

local function set_lines(buf, lines)
  vim.bo[buf].modifiable = true
  vim.api.nvim_buf_set_lines(buf, 0, -1, false, lines)
  vim.bo[buf].modifiable = false
end

local function draw_list(buf, items, width, active, empty)
  local lines, all_marks = {}, {}
  for i, item in ipairs(items) do
    local line, marks = M.format_line(item, width)
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

local function level()
  return M._state.stack[#M._state.stack]
end

local function current_item()
  local l = level()
  return l.shown[l.cursor]
end

--- The preview pane's content for an item.
---@return string[] lines
---@return string? syntax
local function preview_of(item)
  local s = M._state
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

local function breadcrumb()
  local parts = {}
  for _, l in ipairs(M._state.stack) do
    parts[#parts + 1] = l.label
  end
  local l = level()
  local text = " " .. table.concat(parts, "  ›  ")
  if l.filter ~= "" then
    text = text .. "   /" .. l.filter
  end
  local count = ("%d/%d "):format(#l.shown == 0 and 0 or l.cursor, #l.shown)
  return text, count
end

--- Redraw all panes from the state.
function M._render()
  local s = M._state
  if not s or not valid(s.wins.current) then
    return
  end
  local l = level()
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
  local item = current_item()
  vim.bo[s.bufs.preview].syntax = ""
  if item and item.children and not s.help then
    draw_list(s.bufs.preview, M.visible(item.children, l.filter), g.preview_w)
  else
    local lines, syntax = preview_of(item)
    vim.api.nvim_buf_clear_namespace(s.bufs.preview, NS, 0, -1)
    set_lines(s.bufs.preview, lines)
    if syntax then
      vim.bo[s.bufs.preview].syntax = syntax
    end
  end
  pcall(vim.api.nvim_win_set_cursor, s.wins.preview, { 1, 0 })
  -- header and footer
  local text, count = breadcrumb()
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

local function clamp(row)
  local n = #level().shown
  return math.max(1, math.min(row, math.max(n, 1)))
end

--- Move the cursor by `delta` rows (wraps like yazi does at the edges).
---@param delta integer
function M.move(delta)
  local l = level()
  if #l.shown == 0 then
    return
  end
  l.cursor = ((l.cursor - 1 + delta) % #l.shown) + 1
  M._render()
end

---@param row integer 1-based; negative counts from the end
function M.jump(row)
  local l = level()
  l.cursor = clamp(row < 0 and (#l.shown + row + 1) or row)
  M._render()
end

--- Enter the group under the cursor, or choose the item.
function M.enter()
  local s = M._state
  local l = level()
  local item = l.shown[l.cursor]
  if not item then
    return
  end
  if item.children then
    local shown = M.visible(item.children, l.filter)
    s.stack[#s.stack + 1] =
      { items = item.children, label = item.label, filter = l.filter, shown = shown, cursor = 1 }
    s.help = false
    M._render()
    return
  end
  if item.disabled then
    vim.notify(item.label .. ": " .. item.disabled, vim.log.levels.WARN)
    return
  end
  local path = {}
  for i = 1, #s.stack - 1 do
    local lv = s.stack[i]
    path[#path + 1] = lv.shown[lv.cursor]
  end
  path[#path + 1] = item
  local on_choose = s.opts.on_choose
  M.close(true)
  on_choose(item, path)
end

--- Back to the parent level (a no-op at the root).
function M.back()
  local s = M._state
  if #s.stack <= 1 then
    return
  end
  table.remove(s.stack)
  s.help = false
  M._render()
end

--- Set the current level's filter (and refilter).
---@param text string
function M.set_filter(text)
  local l = level()
  local keep = l.shown[l.cursor]
  l.filter = text
  l.shown = M.visible(l.items, text)
  l.cursor = 1
  for i, it in ipairs(l.shown) do
    if it == keep then
      l.cursor = i
    end
  end
  M._render()
end

local function scroll_preview(lines)
  local win = M._state.wins.preview
  if valid(win) then
    local key = vim.keycode(lines > 0 and "<C-e>" or "<C-y>")
    vim.api.nvim_win_call(win, function()
      vim.cmd.normal({ args = { math.abs(lines) .. key }, bang = true })
    end)
  end
end

--- Close the picker; `chosen` suppresses the on_cancel callback.
---@param chosen boolean?
function M.close(chosen)
  local s = M._state
  if not s then
    return
  end
  M._state = nil
  pcall(vim.api.nvim_del_augroup_by_id, s.group)
  for _, win in pairs(s.wins) do
    if valid(win) then
      vim.api.nvim_win_close(win, true)
    end
  end
  for _, buf in pairs(s.bufs) do
    if vim.api.nvim_buf_is_valid(buf) then
      vim.api.nvim_buf_delete(buf, { force = true })
    end
  end
  if valid(s.prev_win) then
    vim.api.nvim_set_current_win(s.prev_win)
  end
  if not chosen and s.opts.on_cancel then
    s.opts.on_cancel()
  end
end

local function geometry(opts)
  local width = math.min(vim.o.columns - 4, 150)
  local count = #opts.items
  for _, it in ipairs(opts.items) do
    count = math.max(count, #(it.children or {}))
  end
  local height = math.max(8, math.min(vim.o.lines - 8, math.max(count, 14)))
  local parent_w = math.max(14, math.floor(width * 0.2))
  local current_w = math.max(24, math.floor(width * 0.36))
  local preview_w = width - parent_w - current_w - 4
  local row = math.max(0, math.floor((vim.o.lines - height - 5) / 2) - 1)
  local col = math.max(0, math.floor((vim.o.columns - width - 2) / 2))
  return {
    width = width,
    height = height,
    parent_w = parent_w,
    current_w = current_w,
    preview_w = preview_w,
    row = row,
    col = col,
  }
end

local function float(buf, cfg, focus)
  local win = vim.api.nvim_open_win(
    buf,
    focus,
    vim.tbl_extend("force", {
      relative = "editor",
      style = "minimal",
      zindex = 60,
    }, cfg)
  )
  vim.wo[win].winhighlight =
    "NormalFloat:DbtPickerNormal,FloatBorder:DbtPickerBorder,CursorLine:DbtPickerSelected"
  vim.wo[win].wrap = false
  return win
end

local function map(buf, lhs, fn)
  vim.keymap.set("n", lhs, fn, { buffer = buf, nowait = true, silent = true })
end

local function attach_keys(buf)
  local function counted(fn)
    return function()
      fn(vim.v.count1)
    end
  end
  for _, lhs in ipairs({ "j", "<Down>", "<C-n>" }) do
    map(buf, lhs, counted(M.move))
  end
  for _, lhs in ipairs({ "k", "<Up>", "<C-p>" }) do
    map(
      buf,
      lhs,
      counted(function(n)
        M.move(-n)
      end)
    )
  end
  for _, lhs in ipairs({ "l", "<Right>", "<CR>" }) do
    map(buf, lhs, M.enter)
  end
  for _, lhs in ipairs({ "h", "<Left>", "<BS>", "-" }) do
    map(buf, lhs, M.back)
  end
  map(buf, "gg", function()
    M.jump(vim.v.count > 0 and vim.v.count or 1)
  end)
  map(buf, "G", function()
    M.jump(vim.v.count > 0 and vim.v.count or -1)
  end)
  map(buf, "<C-d>", function()
    M.move(math.max(1, math.floor(M._state.geom.height / 2)))
  end)
  map(buf, "<C-u>", function()
    M.move(-math.max(1, math.floor(M._state.geom.height / 2)))
  end)
  map(
    buf,
    "J",
    counted(function(n)
      scroll_preview(5 * n)
    end)
  )
  map(
    buf,
    "K",
    counted(function(n)
      scroll_preview(-5 * n)
    end)
  )
  map(buf, "?", function()
    M._state.help = not M._state.help
    M._render()
  end)
  map(buf, "/", function()
    M.set_filter("")
    prompt.run("", function(text)
      if M._state then
        M.set_filter(text)
      end
    end)
  end)
  map(buf, "q", function()
    M.close()
  end)
  map(buf, "<Esc>", function()
    if M._state and level().filter ~= "" then
      M.set_filter("")
    else
      M.close()
    end
  end)
end

--- Open a picker over a tree of items.
---@param opts cogitator.picker.Opts
function M.open(opts)
  M.close()
  local s = {
    opts = opts,
    prev_win = vim.api.nvim_get_current_win(),
    wins = {},
    bufs = {},
    help = false,
  }
  M._state = s
  local root = { items = opts.items, label = opts.title, filter = opts.filter or "", cursor = 1 }
  root.shown = M.visible(opts.items, root.filter)
  for i, it in ipairs(root.shown) do
    if not it.disabled then
      root.cursor = i
      break
    end
  end
  s.stack = { root }
  local g = geometry(opts)
  s.geom = g
  for _, name in ipairs({ "header", "status", "parent", "current", "preview" }) do
    local buf = vim.api.nvim_create_buf(false, true)
    vim.bo[buf].bufhidden = "wipe"
    vim.bo[buf].filetype = "cogitator_picker"
    s.bufs[name] = buf
  end
  local top = g.row + 1
  s.wins.header = float(s.bufs.header, {
    row = g.row,
    col = g.col,
    width = g.width + 2,
    height = 1,
    focusable = false,
  }, false)
  s.wins.parent = float(s.bufs.parent, {
    row = top,
    col = g.col,
    width = g.parent_w,
    height = g.height,
    border = "rounded",
    focusable = false,
  }, false)
  s.wins.preview = float(s.bufs.preview, {
    row = top,
    col = g.col + g.parent_w + g.current_w + 4,
    width = g.preview_w,
    height = g.height,
    border = "rounded",
    focusable = false,
  }, false)
  s.wins.current = float(s.bufs.current, {
    row = top,
    col = g.col + g.parent_w + 2,
    width = g.current_w,
    height = g.height,
    border = "rounded",
  }, true)
  s.wins.status = float(s.bufs.status, {
    row = top + g.height + 2,
    col = g.col,
    width = g.width + 2,
    height = 1,
    focusable = false,
  }, false)
  vim.api.nvim_set_current_win(s.wins.current)
  vim.wo[s.wins.current].cursorline = true
  vim.wo[s.wins.preview].wrap = true
  attach_keys(s.bufs.current)
  s.group = vim.api.nvim_create_augroup("CogitatorPicker", { clear = true })
  vim.api.nvim_create_autocmd("CursorMoved", {
    group = s.group,
    buffer = s.bufs.current,
    callback = function()
      local st = M._state
      if not st or not valid(st.wins.current) then
        return
      end
      local row = vim.api.nvim_win_get_cursor(st.wins.current)[1]
      local l = level()
      if #l.shown > 0 and row ~= l.cursor then
        l.cursor = row
        M._render()
      end
    end,
  })
  vim.api.nvim_create_autocmd("WinLeave", {
    group = s.group,
    buffer = s.bufs.current,
    callback = function()
      vim.schedule(function()
        local st = M._state
        if st == s and vim.api.nvim_get_current_win() ~= st.wins.current then
          M.close()
        end
      end)
    end,
  })
  vim.api.nvim_create_autocmd("VimResized", {
    group = s.group,
    callback = function()
      M.close()
    end,
  })
  M._render()
end

local function enabled()
  local config = package.loaded["cogitator"] and require("cogitator").config
  return not (config and config.ui and config.ui.picker == false)
end

--- A drop-in for vim.ui.select. Extra opts: `detail(item) -> string`,
--- `preview(item) -> lines` and `icon(item) -> string`.
---@param items any[]
---@param opts {prompt?: string, format_item?: fun(item: any): string, detail?: fun(item: any): string?, preview?: fun(item: any): any, icon?: fun(item: any): string?}
---@param on_choice fun(item: any?, idx: integer?)
function M.select(items, opts, on_choice)
  opts = opts or {}
  if not enabled() then
    return vim.ui.select(items, opts, on_choice)
  end
  local format = opts.format_item or tostring
  local entries = {}
  for i, value in ipairs(items) do
    entries[i] = {
      label = format(value),
      detail = opts.detail and opts.detail(value) or nil,
      icon = opts.icon and opts.icon(value) or nil,
      value = value,
      index = i,
      preview = opts.preview and function()
        return opts.preview(value)
      end or nil,
    }
  end
  M.open({
    title = (opts.prompt or "select"):gsub("[:%s]+$", ""),
    items = entries,
    on_choose = function(item)
      on_choice(item.value, item.index)
    end,
    on_cancel = function()
      on_choice(nil, nil)
    end,
  })
end

--- A drop-in for vim.ui.input: a one-line float under a titled border.
--- <CR> accepts; <Esc> leaves insert mode, then <Esc> or q cancels;
--- <Tab> completes when `completion` is set (e.g. "dir").
---@param opts {prompt?: string, default?: string, completion?: string}
---@param on_confirm fun(text: string?)
function M.input(opts, on_confirm)
  opts = opts or {}
  if not enabled() then
    return vim.ui.input(opts, on_confirm)
  end
  local prev = vim.api.nvim_get_current_win()
  local buf = vim.api.nvim_create_buf(false, true)
  vim.bo[buf].bufhidden = "wipe"
  vim.bo[buf].filetype = "cogitator_input"
  vim.api.nvim_buf_set_lines(buf, 0, -1, false, { opts.default or "" })
  local title = (opts.prompt or "input"):gsub("[:%s]+$", "")
  local width = math.max(40, math.min(vim.o.columns - 8, math.max(60, #title + 8, #(opts.default or "") + 4)))
  local win = float(buf, {
    row = math.floor((vim.o.lines - 3) / 2) - 1,
    col = math.floor((vim.o.columns - width - 2) / 2),
    width = width,
    height = 1,
    border = "rounded",
    title = " " .. title .. " ",
    title_pos = "left",
    footer = " <CR> accept · <Esc> normal · q cancel" .. (opts.completion and " · <Tab> complete " or " "),
    footer_pos = "right",
  }, true)
  local done = false
  local function finish(text)
    if done then
      return
    end
    done = true
    vim.cmd.stopinsert()
    if vim.api.nvim_win_is_valid(win) then
      vim.api.nvim_win_close(win, true)
    end
    if vim.api.nvim_win_is_valid(prev) then
      vim.api.nvim_set_current_win(prev)
    end
    on_confirm(text)
  end
  local function accept()
    finish(vim.api.nvim_buf_get_lines(buf, 0, 1, false)[1] or "")
  end
  local o = { buffer = buf, nowait = true, silent = true }
  vim.keymap.set({ "n", "i" }, "<CR>", accept, o)
  vim.keymap.set("n", "<Esc>", function()
    finish(nil)
  end, o)
  vim.keymap.set("n", "q", function()
    finish(nil)
  end, o)
  vim.keymap.set("i", "<C-c>", function()
    finish(nil)
  end, o)
  if opts.completion then
    vim.keymap.set("i", "<Tab>", function()
      local line = vim.api.nvim_get_current_line()
      local matches = vim.fn.getcompletion(line, opts.completion)
      if #matches > 0 then
        vim.fn.complete(1, matches)
      end
    end, o)
  end
  vim.api.nvim_create_autocmd("WinLeave", {
    buffer = buf,
    once = true,
    callback = function()
      vim.schedule(function()
        finish(nil)
      end)
    end,
  })
  vim.cmd.startinsert({ bang = true })
end

return M
