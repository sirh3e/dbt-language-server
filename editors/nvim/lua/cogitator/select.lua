--- Expand and shrink the visual selection along the server's
--- textDocument/selectionRange chain (word → expression → Jinja block →
--- CTE → statement → document). Expanding keeps a per-buffer stack, so
--- shrinking retraces exactly the ranges expanding went through.

local actions = require("cogitator.actions")

local M = {}

local TIMEOUT_MS = 1000

--- buffer → the ranges selected so far, innermost first
---@type table<integer, table[]>
M._stack = {}
--- client id → true once a request timed out (no more waiting on it)
---@type table<integer, boolean>
M._unanswered = {}
--- cancels the selectionRange request in flight, if any
---@type fun()?
M._pending = nil

--- An LSP position as a 0-based (row, byte column) of the buffer.
---@param bufnr integer
---@param pos {line: integer, character: integer}
---@param encoding string
---@return integer row, integer col
local function to_byte(bufnr, pos, encoding)
  local line = vim.api.nvim_buf_get_lines(bufnr, pos.line, pos.line + 1, false)[1] or ""
  local ok, col = pcall(vim.str_byteindex, line, encoding, pos.character, false)
  return pos.line, ok and col or #line
end

--- Does range `outer` contain `inner` (both {start, end} of {line, character})?
---@return boolean
function M.contains(outer, inner)
  local function le(a, b)
    return a.line < b.line or (a.line == b.line and a.character <= b.character)
  end
  return le(outer.start, inner.start) and le(inner["end"], outer["end"])
end

--- Two ranges cover the same text.
---@return boolean
function M.same(a, b)
  return M.contains(a, b) and M.contains(b, a)
end

--- The ranges of a SelectionRange chain, innermost first.
---@param chain table?
---@return table[]
function M.flatten(chain)
  local ranges = {}
  while type(chain) == "table" and chain.range do
    ranges[#ranges + 1] = chain.range
    chain = chain.parent
  end
  return ranges
end

--- The first range of `ranges` that strictly grows `current` (or the
--- innermost one when nothing is selected yet).
---@param ranges table[]
---@param current table?
---@return table?
function M.next_larger(ranges, current)
  for _, range in ipairs(ranges) do
    if not current or (M.contains(range, current) and not M.same(range, current)) then
      return range
    end
  end
end

--- Select an LSP range visually (end exclusive → inclusive).
---@param bufnr integer
---@param range table
---@param encoding string
local function select_range(bufnr, range, encoding)
  local srow, scol = to_byte(bufnr, range.start, encoding)
  local erow, ecol = to_byte(bufnr, range["end"], encoding)
  if ecol == 0 and erow > srow then
    erow = erow - 1
    ecol = #(vim.api.nvim_buf_get_lines(bufnr, erow, erow + 1, false)[1] or "")
  end
  ecol = math.max(ecol - 1, 0)
  if vim.fn.mode():match("^[vV\22]") then
    vim.cmd("normal! \27")
  end
  vim.api.nvim_buf_set_mark(bufnr, "<", srow + 1, scol, {})
  vim.api.nvim_buf_set_mark(bufnr, ">", erow + 1, ecol, {})
  vim.cmd("normal! gv")
end

--- The current selection as an LSP range (nil outside visual mode).
---@param bufnr integer
---@param encoding string
---@return table?
local function current_range(bufnr, encoding)
  local mode = vim.fn.mode()
  if not mode:match("^[vV\22]") then
    return nil
  end
  local a, b = vim.fn.getpos("v"), vim.fn.getpos(".")
  if a[2] > b[2] or (a[2] == b[2] and a[3] > b[3]) then
    a, b = b, a
  end
  local function lsp(row, col)
    local line = vim.api.nvim_buf_get_lines(bufnr, row - 1, row, false)[1] or ""
    local ok, char = pcall(vim.str_utfindex, line, encoding, math.min(col, #line), false)
    return { line = row - 1, character = ok and char or col }
  end
  local stop = b[3]
  if mode == "V" then
    a[3], stop = 1, #(vim.api.nvim_buf_get_lines(bufnr, b[2] - 1, b[2], false)[1] or "") + 1
  end
  return { start = lsp(a[2], a[3] - 1), ["end"] = lsp(b[2], stop) }
end

--- Expand the selection (from normal mode: select the innermost range).
--- Asks the server in the background: the selection changes when the
--- answer comes, and only if nothing moved meanwhile.
function M.expand()
  local bufnr = vim.api.nvim_get_current_buf()
  local client = actions.client_for(bufnr)
  if not client then
    return
  end
  if not client:supports_method("textDocument/selectionRange") then
    vim.notify("this cogitator does not answer textDocument/selectionRange", vim.log.levels.WARN)
    return
  end
  if M._unanswered[client.id] then
    vim.notify("selection ranges timed out on this server before; not waiting again", vim.log.levels.WARN)
    return
  end
  local encoding = client.offset_encoding or "utf-16"
  local current = current_range(bufnr, encoding)
  local cursor = vim.api.nvim_win_get_cursor(0)
  local line = vim.api.nvim_buf_get_lines(bufnr, cursor[1] - 1, cursor[1], false)[1] or ""
  local ok, character = pcall(vim.str_utfindex, line, encoding, cursor[2], false)
  local win = vim.api.nvim_get_current_win()
  local tick = vim.api.nvim_buf_get_changedtick(bufnr)
  local mode = vim.fn.mode()
  -- the answer applies only to the text, window and selection it was
  -- asked about; anything else moved on, and it is dropped
  local function stale()
    if not vim.api.nvim_buf_is_valid(bufnr) or vim.api.nvim_get_current_win() ~= win then
      return true
    end
    if vim.api.nvim_win_get_buf(win) ~= bufnr or vim.api.nvim_buf_get_changedtick(bufnr) ~= tick then
      return true
    end
    local now = vim.api.nvim_win_get_cursor(win)
    return now[1] ~= cursor[1] or now[2] ~= cursor[2] or vim.fn.mode() ~= mode
  end
  if M._pending then
    M._pending()
  end
  M._pending = actions.request(client, bufnr, "textDocument/selectionRange", {
    textDocument = { uri = vim.uri_from_bufnr(bufnr) },
    positions = { { line = cursor[1] - 1, character = ok and character or cursor[2] } },
  }, { timeout_ms = TIMEOUT_MS, stale = stale }, function(err, result, reason)
    M._pending = nil
    if reason == "timeout" then
      M._unanswered[client.id] = true
      vim.notify("textDocument/selectionRange timed out", vim.log.levels.WARN)
      return
    end
    if stale() then
      return
    end
    if err or type(result) ~= "table" then
      vim.notify("no selection range here", vim.log.levels.INFO)
      return
    end
    local range = M.next_larger(M.flatten(result[1]), current)
    if not range then
      return
    end
    local stack = current and M._stack[bufnr] or {}
    if current and #stack == 0 then
      stack[1] = current
    end
    stack[#stack + 1] = range
    M._stack[bufnr] = stack
    select_range(bufnr, range, encoding)
  end)
end

--- Shrink back to the range selected before the last expand.
function M.shrink()
  local bufnr = vim.api.nvim_get_current_buf()
  local stack = M._stack[bufnr] or {}
  if #stack < 2 then
    if vim.fn.mode():match("^[vV\22]") then
      vim.cmd("normal! \27")
    end
    M._stack[bufnr] = {}
    return
  end
  table.remove(stack)
  local client = actions.client_for(bufnr, true)
  select_range(bufnr, stack[#stack], client and client.offset_encoding or "utf-16")
end

return M
