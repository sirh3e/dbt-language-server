--- A yazi-style picker: three columns — the parent level, the current
--- level, and a preview of what is under the cursor — with a breadcrumb
--- header and a key-hint footer. Walked with vim motions: j/k move,
--- l (or <CR>) enters a group or chooses an item, h goes back up,
--- gg/G jump, / filters as you type (fzf's syntax), J/K scroll the
--- preview, ? shows the keys, q/<Esc> close. Every dialog of the plugin
--- goes through here: `select` is a drop-in for vim.ui.select, `input`
--- for vim.ui.input, and `open` takes a tree (the code-action menu,
--- the :Dbt menu).
---
--- Pure helpers (`format_line`, `matches`, `visible`) are exposed for
--- the test suite; the rest is window plumbing.
local match = require("cogitator.picker.match")
local view = require("cogitator.picker.view")
local window = require("cogitator.picker.window")

local M = {}

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

M.KEYS = view.KEYS
M.matches = match.matches
M.visible = match.visible
M.format_line = match.format_line
M.select = require("cogitator.picker.dialogs").select
M.input = require("cogitator.picker.dialogs").input

--- The level the cursor is on.
function M.level()
  return M._state.stack[#M._state.stack]
end
local level = M.level

--- Redraw all panes from the state.
function M._render()
  view.render(M._state)
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
    -- a group the filter kept by its own name opens whole; one kept by
    -- its children opens on the children that matched
    local filter = match.matches({ label = item.label, detail = item.detail }, l.filter) and "" or l.filter
    local shown = match.visible(item.children, filter)
    s.stack[#s.stack + 1] =
      { items = item.children, label = item.label, filter = filter, shown = shown, cursor = 1 }
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
  l.shown = match.visible(l.items, text)
  l.cursor = 1
  for i, it in ipairs(l.shown) do
    if it == keep then
      l.cursor = i
    end
  end
  M._render()
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
    if window.valid(win) then
      vim.api.nvim_win_close(win, true)
    end
  end
  for _, buf in pairs(s.bufs) do
    if vim.api.nvim_buf_is_valid(buf) then
      vim.api.nvim_buf_delete(buf, { force = true })
    end
  end
  if window.valid(s.prev_win) then
    vim.api.nvim_set_current_win(s.prev_win)
  end
  if not chosen and s.opts.on_cancel then
    s.opts.on_cancel()
  end
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
  root.shown = match.visible(opts.items, root.filter)
  for i, it in ipairs(root.shown) do
    if not it.disabled then
      root.cursor = i
      break
    end
  end
  s.stack = { root }
  local g = window.geometry(opts)
  s.geom = g
  for _, name in ipairs({ "header", "status", "parent", "current", "preview" }) do
    local buf = vim.api.nvim_create_buf(false, true)
    vim.bo[buf].bufhidden = "wipe"
    vim.bo[buf].filetype = "cogitator_picker"
    s.bufs[name] = buf
  end
  local top = g.row + 1
  s.wins.header = window.float(s.bufs.header, {
    row = g.row,
    col = g.col,
    width = g.width + 2,
    height = 1,
    focusable = false,
  }, false)
  s.wins.parent = window.float(s.bufs.parent, {
    row = top,
    col = g.col,
    width = g.parent_w,
    height = g.height,
    border = "rounded",
    focusable = false,
  }, false)
  s.wins.preview = window.float(s.bufs.preview, {
    row = top,
    col = g.col + g.parent_w + g.current_w + 4,
    width = g.preview_w,
    height = g.height,
    border = "rounded",
    focusable = false,
  }, false)
  s.wins.current = window.float(s.bufs.current, {
    row = top,
    col = g.col + g.parent_w + 2,
    width = g.current_w,
    height = g.height,
    border = "rounded",
  }, true)
  s.wins.status = window.float(s.bufs.status, {
    row = top + g.height + 2,
    col = g.col,
    width = g.width + 2,
    height = 1,
    focusable = false,
  }, false)
  vim.api.nvim_set_current_win(s.wins.current)
  vim.wo[s.wins.current].cursorline = true
  vim.wo[s.wins.preview].wrap = true
  require("cogitator.picker.keys").attach(s.bufs.current)
  s.group = vim.api.nvim_create_augroup("CogitatorPicker", { clear = true })
  vim.api.nvim_create_autocmd("CursorMoved", {
    group = s.group,
    buffer = s.bufs.current,
    callback = function()
      local st = M._state
      if not st or not window.valid(st.wins.current) then
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

return M
