--- :DbtSyntaxTree — the server's tree-sitter-dbt tree of the current
--- buffer (dbt.syntaxTree) in a side split, like :InspectTree. The two
--- follow each other: the source cursor selects the innermost node, the
--- tree cursor highlights its node's range in the source. <CR> jumps
--- there, q closes; edits re-fetch the tree after a pause.

local actions = require("cogitator.actions")

local M = {}

local NS = vim.api.nvim_create_namespace("cogitator.syntax")
local GROUP = "CogitatorSyntax"
local REFETCH_MS = 250

---@class cogitator.syntax.State
---@field src integer the source buffer
---@field buf integer the tree buffer
---@field win integer the tree window
---@field client vim.lsp.Client
---@field nodes table[] dbt.syntaxTree's nodes, one per tree line
---@field timer uv.uv_timer_t?
---@type cogitator.syntax.State?
M._state = nil

--- One tree line: indentation by depth, the field, the kind and its range.
---@param node table
---@return string
function M.line(node)
  local r = node.range
  local kind = node.missing and ("MISSING " .. node.kind) or node.kind
  return ("%s%s%s [%d:%d - %d:%d]"):format(
    string.rep("  ", node.depth),
    node.field ~= nil and node.field ~= vim.NIL and (node.field .. ": ") or "",
    kind,
    r.start.line + 1,
    r.start.character,
    r["end"].line + 1,
    r["end"].character
  )
end

--- The innermost node (last in pre-order) whose range holds the position.
---@param nodes table[]
---@param line integer 0-based
---@param character integer UTF-16
---@return integer? index into nodes
function M.innermost(nodes, line, character)
  local function before(a, b)
    return a.line < b.line or (a.line == b.line and a.character <= b.character)
  end
  local pos = { line = line, character = character }
  local found
  for i, node in ipairs(nodes) do
    if before(node.range.start, pos) and before(pos, node.range["end"]) then
      found = i
    end
  end
  return found
end

local function encoding()
  local s = M._state
  return s and s.client.offset_encoding or "utf-16"
end

--- A UTF-16 (or the client's encoding) position as a byte column.
local function byte_col(buf, pos)
  local text = vim.api.nvim_buf_get_lines(buf, pos.line, pos.line + 1, false)[1] or ""
  return require("cogitator.positions").byte_col(text, pos.character, encoding())
end

local function render(result)
  local s = M._state
  s.nodes = result.nodes or {}
  local lines = vim.tbl_map(M.line, s.nodes)
  if result.truncated then
    lines[#lines + 1] = "… (more nodes than the server sends)"
  end
  require("cogitator.ui").set_lines(s.buf, lines)
  vim.api.nvim_buf_clear_namespace(s.buf, NS, 0, -1)
  for i, node in ipairs(s.nodes) do
    if node.error or node.missing then
      vim.api.nvim_buf_set_extmark(s.buf, NS, i - 1, 0, { line_hl_group = "DiagnosticError" })
    end
  end
end

local function fetch()
  local s = M._state
  if not s or s.client:is_stopped() or not vim.api.nvim_buf_is_valid(s.src) then
    return
  end
  actions.exec_with(
    s.client,
    s.src,
    "dbt.syntaxTree",
    { uri = vim.uri_from_bufnr(s.src) },
    function(err, result)
      if M._state ~= s then
        return
      end
      if err or type(result) ~= "table" then
        vim.notify(
          "dbt.syntaxTree failed" .. (err and (": " .. err.message) or " (an older server without it?)"),
          vim.log.levels.WARN
        )
        return
      end
      render(result)
      M.follow_source()
    end
  )
end

--- Select the tree line of the node under the source cursor.
function M.follow_source()
  local s = M._state
  if not (s and vim.api.nvim_win_is_valid(s.win)) then
    return
  end
  local src_win = vim.fn.bufwinid(s.src)
  if src_win == -1 then
    return
  end
  local row, col = unpack(vim.api.nvim_win_get_cursor(src_win))
  local text = vim.api.nvim_buf_get_lines(s.src, row - 1, row, false)[1] or ""
  local ok, character = pcall(vim.str_utfindex, text, encoding(), math.min(col, #text), false)
  local index = M.innermost(s.nodes, row - 1, ok and character or col)
  if index then
    vim.api.nvim_win_set_cursor(s.win, { index, s.nodes[index].depth * 2 })
  end
end

--- Highlight the source range of the tree line under the tree cursor.
local function show_range()
  local s = M._state
  if not s then
    return
  end
  vim.api.nvim_buf_clear_namespace(s.src, NS, 0, -1)
  local node = s.nodes[vim.api.nvim_win_get_cursor(s.win)[1]]
  if not node then
    return
  end
  local r = node.range
  vim.api.nvim_buf_set_extmark(s.src, NS, r.start.line, byte_col(s.src, r.start), {
    end_row = r["end"].line,
    end_col = byte_col(s.src, r["end"]),
    hl_group = "Visual",
    strict = false,
  })
end

--- <CR>: jump to the node's start in the source.
local function jump()
  local s = M._state
  local node = s.nodes[vim.api.nvim_win_get_cursor(s.win)[1]]
  local src_win = vim.fn.bufwinid(s.src)
  if node and src_win ~= -1 then
    vim.api.nvim_set_current_win(src_win)
    vim.api.nvim_win_set_cursor(src_win, { node.range.start.line + 1, byte_col(s.src, node.range.start) })
  end
end

function M.close()
  local s = M._state
  if not s then
    return
  end
  M._state = nil
  pcall(vim.api.nvim_del_augroup_by_name, GROUP)
  if s.timer then
    s.timer:stop()
    s.timer:close()
  end
  if vim.api.nvim_buf_is_valid(s.src) then
    vim.api.nvim_buf_clear_namespace(s.src, NS, 0, -1)
  end
  if vim.api.nvim_win_is_valid(s.win) then
    pcall(vim.api.nvim_win_close, s.win, true)
  end
end

--- :DbtSyntaxTree — open the view for the current buffer, or close it.
function M.toggle()
  if M._state then
    return M.close()
  end
  M.open()
end

function M.open()
  local src = vim.api.nvim_get_current_buf()
  local client = actions.client_for(src)
  if not client then
    return
  end
  local src_win = vim.api.nvim_get_current_win()
  vim.cmd("botright vnew")
  local buf, win = vim.api.nvim_get_current_buf(), vim.api.nvim_get_current_win()
  vim.bo[buf].buftype = "nofile"
  vim.bo[buf].bufhidden = "wipe"
  vim.bo[buf].swapfile = false
  vim.bo[buf].modifiable = false
  vim.api.nvim_buf_set_name(buf, "cogitator://syntax/" .. buf)
  vim.wo[win].wrap = false
  vim.wo[win].cursorline = true
  vim.wo[win].number = false
  M._state = { src = src, buf = buf, win = win, client = client, nodes = {} }

  local motion = require("cogitator.motion")
  motion.map(buf, "q", M.close, "close the syntax tree")
  motion.map(buf, "<CR>", jump, "jump to the node in the source")

  local group = vim.api.nvim_create_augroup(GROUP, { clear = true })
  vim.api.nvim_create_autocmd("CursorMoved", { group = group, buffer = buf, callback = show_range })
  vim.api.nvim_create_autocmd("CursorMoved", { group = group, buffer = src, callback = M.follow_source })
  vim.api.nvim_create_autocmd({ "TextChanged", "TextChangedI" }, {
    group = group,
    buffer = src,
    callback = function()
      local s = M._state
      if not s then
        return
      end
      s.timer = s.timer or vim.uv.new_timer()
      s.timer:stop()
      s.timer:start(REFETCH_MS, 0, vim.schedule_wrap(fetch))
    end,
  })
  vim.api.nvim_create_autocmd("BufWipeout", {
    group = group,
    buffer = buf,
    callback = function()
      if M._state and M._state.buf == buf then
        M.close()
      end
    end,
  })
  vim.api.nvim_set_current_win(src_win)
  fetch()
end

return M
