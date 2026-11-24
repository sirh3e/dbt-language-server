--- :DbtNode [kind:]name — any node's card (dbt.nodeInfo) in a float.

local actions = require("cogitator.actions")

local client_for = actions.client_for

local M = {}

--- `[kind:]name` → kind (or nil) and name.
---@param args string
---@return string? kind, string name
function M.parse_node(args)
  args = vim.trim(args or "")
  local kind, name = args:match("^([%w_]+):(.+)$")
  if kind then
    return kind, vim.trim(name)
  end
  return nil, args
end

--- The kinds :DbtNode tries, in order, when none is given.
M.NODE_KINDS =
  { "model", "macro", "test", "generic_test", "source", "exposure", "metric", "semantic_model", "analysis" }

--- Show a dbt.nodeInfo card in a float; <CR> on a line with a target
--- opens it, q / <Esc> close.
---@param info table
local function node_float(info)
  local ui = require("cogitator.ui")
  local width = math.min(90, math.max(50, vim.o.columns - 10))
  local card = require("cogitator.explorer.card").node(info, width - 2)
  local height = math.min(#card.lines, math.max(5, vim.o.lines - 8))
  local buf, win = ui.float({
    row = math.max(0, math.floor((vim.o.lines - height) / 2) - 1),
    col = math.max(0, math.floor((vim.o.columns - width) / 2)),
    width = width,
    height = height,
    title = (info.kind or "node") .. " · " .. (info.name or info.model or "?"),
    cursorline = true,
  })
  ui.set_lines(buf, card.lines)
  for _, line in ipairs(card.headings) do
    vim.api.nvim_buf_add_highlight(buf, -1, "Title", line - 1, 0, -1)
  end
  vim.api.nvim_set_current_win(win)
  local function close()
    if vim.api.nvim_win_is_valid(win) then
      vim.api.nvim_win_close(win, true)
    end
  end
  for _, lhs in ipairs({ "q", "<Esc>" }) do
    vim.keymap.set("n", lhs, close, { buffer = buf, nowait = true })
  end
  vim.keymap.set("n", "<CR>", function()
    local target = card.jumps[vim.api.nvim_win_get_cursor(win)[1]]
    if not target then
      target = info.uri and info.uri ~= vim.NIL and { uri = info.uri, range = info.range } or nil
    end
    if not target then
      return
    end
    close()
    if require("cogitator.actions").open_uri(target.uri) and target.range and target.range ~= vim.NIL then
      pcall(
        vim.api.nvim_win_set_cursor,
        0,
        { target.range.start.line + 1, target.range.start.character or 0 }
      )
    end
  end, { buffer = buf, nowait = true })
  return buf, win
end

--- :DbtNode [kind:]name — a node's card (dbt.nodeInfo) in a float: a
--- macro's signature, callers and calls; a test's refs; a generic test's
--- places in YAML; a model's neighbours. Without a kind, the kinds are
--- tried in order; without a name, the current buffer's node.
---@param opts {args: string}
function M.node(opts)
  local bufnr = vim.api.nvim_get_current_buf()
  local client = client_for(bufnr)
  if not client then
    return
  end
  local kind, name = M.parse_node(opts and opts.args or "")
  if name == "" then
    name = vim.fn.fnamemodify(vim.api.nvim_buf_get_name(bufnr), ":t:r")
  end
  if name == "" then
    vim.notify("usage: :DbtNode [kind:]name   e.g. :DbtNode macro:cents_to_dollars", vim.log.levels.INFO)
    return
  end
  local kinds = kind and { kind } or M.NODE_KINDS
  local function try(i)
    local k = kinds[i]
    if not k then
      vim.notify(("no node named %s (tried %s)"):format(name, table.concat(kinds, ", ")), vim.log.levels.WARN)
      return
    end
    client:exec_cmd({
      command = "dbt.nodeInfo",
      arguments = { { node = { kind = k, name = name }, quiet = #kinds > 1 } },
    }, { bufnr = bufnr }, function(err, result)
      if err then
        vim.notify("dbt.nodeInfo failed: " .. err.message, vim.log.levels.ERROR)
      elseif type(result) == "table" then
        node_float(result)
      else
        try(i + 1)
      end
    end)
  end
  try(1)
end

return M
