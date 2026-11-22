--- The built-in fzf-style picker, for when no picker plugin is installed:
--- a prompt, a ranked list with the matched characters highlighted, and
--- a preview, in floats. Typing filters on every keystroke (fzf's syntax,
--- cogitator.pick.query); items and previews arrive asynchronously, the
--- list says "loading" until then.
---
--- Keys in the prompt: <C-j>/<C-n>/<Down> and <C-k>/<C-p>/<Up> move,
--- <Tab> marks and moves on, <CR> opens, <C-v>/<C-x>/<C-t> in a vsplit /
--- split / tab, <C-g> shows it in the navigator, <C-y> yanks, <C-r> runs
--- with dbt, <C-d>/<C-u> scroll the preview, <Esc>/<C-c> close.

local query = require("cogitator.pick.query")
local ui = require("cogitator.ui")

local M = { name = "builtin" }

local NS = vim.api.nvim_create_namespace("cogitator.pick")
local SHOWN = 2000
local PREVIEW_DEBOUNCE_MS = 40

vim.api.nvim_set_hl(0, "CogitatorPickMatch", { default = true, link = "Special" })
vim.api.nvim_set_hl(0, "CogitatorPickMark", { default = true, link = "WarningMsg" })

function M.available()
  return true
end

---@class cogitator.pick.BuiltinState
---@field source table
---@field run fun(action: string, items: table[])
---@field items table[]? nil while loading
---@field texts string[]
---@field matches cogitator.pick.Match[]
---@field query string
---@field cursor integer 1-based into matches
---@field marked table<integer, boolean> by item index
---@field wins table<string, integer>
---@field bufs table<string, integer>
---@field gen integer preview staleness guard
---@field timer uv.uv_timer_t
---@field closing boolean?
---@type cogitator.pick.BuiltinState?
M._state = nil

local function valid(win)
  return win and vim.api.nvim_win_is_valid(win)
end

--- Close every window; `cancelled` tells the source nobody chose.
function M.close(cancelled)
  local s = M._state
  if not s or s.closing then
    return
  end
  s.closing = true
  M._state = nil
  s.timer:stop()
  s.timer:close()
  vim.cmd("stopinsert")
  for _, win in pairs(s.wins) do
    if valid(win) then
      pcall(vim.api.nvim_win_close, win, true)
    end
  end
  for _, buf in pairs(s.bufs) do
    if vim.api.nvim_buf_is_valid(buf) then
      pcall(vim.api.nvim_buf_delete, buf, { force = true })
    end
  end
  if cancelled and s.source.on_cancel then
    s.source.on_cancel()
  end
end

local function title(s)
  if not s.items then
    return ("%s · loading…"):format(s.source.title)
  end
  local marked = vim.tbl_count(s.marked)
  return ("> %s %d/%d%s"):format(
    s.source.title,
    #s.matches,
    #s.items,
    marked > 0 and (" · %d marked"):format(marked) or ""
  )
end

--- The item under the cursor.
---@return table?
function M.current()
  local s = M._state
  local m = s and s.matches[s.cursor]
  return m and s.items[m.index]
end

local function render_preview(s)
  if not s.wins.preview then
    return
  end
  local item = M.current()
  s.gen = s.gen + 1
  local gen = s.gen
  s.timer:stop()
  if not item then
    ui.set_lines(s.bufs.preview, {})
    return
  end
  s.timer:start(
    PREVIEW_DEBOUNCE_MS,
    0,
    vim.schedule_wrap(function()
      if M._state ~= s or s.gen ~= gen then
        return
      end
      s.source.preview(item, function(p)
        if M._state ~= s or s.gen ~= gen or not vim.api.nvim_buf_is_valid(s.bufs.preview) then
          return
        end
        ui.set_lines(s.bufs.preview, p.lines or {})
        vim.bo[s.bufs.preview].filetype = p.filetype or ""
        require("cogitator.tshl").preview(s.bufs.preview, p)
        if valid(s.wins.preview) then
          local row = math.max(1, math.min(p.lnum or 1, vim.api.nvim_buf_line_count(s.bufs.preview)))
          pcall(vim.api.nvim_win_set_cursor, s.wins.preview, { row, 0 })
          if p.lnum then
            vim.api.nvim_win_call(s.wins.preview, function()
              vim.cmd("normal! zz")
            end)
          end
        end
      end)
    end)
  )
end

local function render_list(s)
  local lines = {}
  if not s.items then
    lines = { "  loading…" }
  elseif #s.matches == 0 then
    lines = { "  · no match ·" }
  else
    for k = 1, math.min(#s.matches, SHOWN) do
      local m = s.matches[k]
      lines[k] = (s.marked[m.index] and "● " or "  ") .. s.texts[m.index]
    end
  end
  ui.set_lines(s.bufs.list, lines)
  vim.api.nvim_buf_clear_namespace(s.bufs.list, NS, 0, -1)
  if s.items then
    for k = 1, math.min(#s.matches, SHOWN) do
      local m = s.matches[k]
      local text = s.texts[m.index]
      for _, pos in ipairs(m.positions) do
        local start = vim.str_byteindex(text, "utf-32", pos, false)
        local stop = vim.str_byteindex(text, "utf-32", pos + 1, false)
        vim.api.nvim_buf_set_extmark(s.bufs.list, NS, k - 1, start + 2, {
          end_col = stop + 2,
          hl_group = "CogitatorPickMatch",
          strict = false,
        })
      end
      if s.marked[m.index] then
        vim.api.nvim_buf_set_extmark(
          s.bufs.list,
          NS,
          k - 1,
          0,
          { end_col = 2, hl_group = "CogitatorPickMark", strict = false }
        )
      end
    end
  end
  s.cursor = math.max(1, math.min(s.cursor, math.max(1, math.min(#s.matches, SHOWN))))
  if valid(s.wins.list) then
    pcall(vim.api.nvim_win_set_cursor, s.wins.list, { s.cursor, 0 })
  end
  if valid(s.wins.prompt) then
    ui.retitle(s.wins.prompt, " " .. title(s) .. " ")
  end
  render_preview(s)
end

--- Re-rank for the current query (also the test hook for typing).
---@param text string?
function M._set_query(text)
  local s = M._state
  if not s then
    return
  end
  if text then
    s.query = text
    if vim.api.nvim_buf_is_valid(s.bufs.prompt) then
      vim.api.nvim_buf_set_lines(s.bufs.prompt, 0, -1, false, { text })
    end
  end
  s.matches = s.items and query.rank(s.texts, s.query) or {}
  s.cursor = 1
  render_list(s)
end

--- Move the cursor by `delta` (wraps).
function M._move(delta)
  local s = M._state
  if not s or #s.matches == 0 then
    return
  end
  local n = math.min(#s.matches, SHOWN)
  s.cursor = (s.cursor - 1 + delta) % n + 1
  if valid(s.wins.list) then
    vim.api.nvim_win_set_cursor(s.wins.list, { s.cursor, 0 })
  end
  render_preview(s)
end

--- Mark or unmark the current item, then move on.
function M._toggle_mark()
  local s = M._state
  local m = s and s.matches[s.cursor]
  if not m then
    return
  end
  s.marked[m.index] = not s.marked[m.index] or nil
  local cursor = s.cursor
  render_list(s)
  s.cursor = cursor
  M._move(1)
end

--- Close and run `action` on the marked items, or the current one.
---@param action string
function M._accept(action)
  local s = M._state
  if not s or not s.items then
    return
  end
  local chosen = {}
  for i, item in ipairs(s.items) do
    if s.marked[i] then
      chosen[#chosen + 1] = item
    end
  end
  if #chosen == 0 then
    chosen = { M.current() }
  end
  M.close(false)
  if #chosen > 0 then
    vim.schedule(function()
      s.run(action, chosen)
    end)
  elseif s.source.on_cancel then
    s.source.on_cancel()
  end
end

local function scroll_preview(lines)
  local s = M._state
  if s and valid(s.wins.preview) then
    require("cogitator.motion").scroll(s.wins.preview, lines)
  end
end

local function keymaps(s)
  local buf = s.bufs.prompt
  local function map(lhs, fn)
    vim.keymap.set({ "i", "n" }, lhs, fn, { buffer = buf, nowait = true })
  end
  for _, lhs in ipairs({ "<C-j>", "<C-n>", "<Down>" }) do
    map(lhs, function()
      M._move(1)
    end)
  end
  for _, lhs in ipairs({ "<C-k>", "<C-p>", "<Up>" }) do
    map(lhs, function()
      M._move(-1)
    end)
  end
  map("<Tab>", M._toggle_mark)
  for action, lhs in pairs(require("cogitator.pick.actions").KEYS) do
    map(lhs, function()
      M._accept(action)
    end)
  end
  map("<C-d>", function()
    scroll_preview(10)
  end)
  map("<C-u>", function()
    scroll_preview(-10)
  end)
  for _, lhs in ipairs({ "<Esc>", "<C-c>" }) do
    map(lhs, function()
      M.close(true)
    end)
  end
  vim.keymap.set("n", "q", function()
    M.close(true)
  end, { buffer = buf, nowait = true })
end

--- Open the picker: at once, in a loading state until the items arrive.
---@param source table
---@param run fun(action: string, items: table[])
---@param items table[]? already fetched (a fallback from another backend)
function M.open(source, run, items)
  M.close(true)
  local width = math.min(vim.o.columns - 4, 150)
  local height = math.min(vim.o.lines - 6, 32)
  local row = math.max(0, math.floor((vim.o.lines - height) / 2) - 1)
  local col = math.max(0, math.floor((vim.o.columns - width) / 2))
  local has_preview = source.preview ~= nil
  local list_w = has_preview and math.floor(width * 0.45) or width
  local s = {
    source = source,
    run = run,
    items = nil,
    texts = {},
    matches = {},
    query = "",
    cursor = 1,
    marked = {},
    wins = {},
    bufs = {},
    gen = 0,
    timer = vim.uv.new_timer(),
  }
  M._state = s
  s.bufs.list, s.wins.list = ui.float({
    row = row,
    col = col,
    width = list_w,
    height = height - 3,
    title = source.title,
    cursorline = true,
  })
  if has_preview then
    s.bufs.preview, s.wins.preview = ui.float({
      row = row,
      col = col + list_w + 2,
      width = width - list_w - 2,
      height = height,
      title = "preview",
    })
  end
  s.bufs.prompt, s.wins.prompt = ui.float({
    row = row + height - 1,
    col = col,
    width = list_w,
    height = 1,
    title = title(s),
  })
  vim.bo[s.bufs.prompt].modifiable = true
  keymaps(s)
  vim.api.nvim_create_autocmd("TextChangedI", {
    buffer = s.bufs.prompt,
    callback = function()
      if M._state == s then
        s.query = vim.api.nvim_buf_get_lines(s.bufs.prompt, 0, 1, false)[1] or ""
        M._set_query()
      end
    end,
  })
  vim.api.nvim_create_autocmd("WinLeave", {
    buffer = s.bufs.prompt,
    callback = function()
      vim.schedule(function()
        if M._state == s then
          M.close(true)
        end
      end)
    end,
  })
  vim.api.nvim_set_current_win(s.wins.prompt)
  vim.cmd("startinsert")
  render_list(s)

  local function arrived(list)
    if M._state ~= s then
      return
    end
    s.items = list
    for i, item in ipairs(list) do
      s.texts[i] = item.text
    end
    M._set_query()
  end
  if items then
    arrived(items)
  else
    source.items(arrived)
  end
end

return M
