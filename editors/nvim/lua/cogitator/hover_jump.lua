--- From a hover to what it is about. The server ends the hover of a
--- ref(), source() or macro with `Definition: [file:line](file://…#L<n>)`;
--- in a floating window holding such a link (`K` twice focuses Neovim's
--- hover, and noice's), <CR> and gd close the float and open it. Only the
--- float's text is read, so any hover renderer works.

local M = {}

--- The first `file://…#L<n>` link in `lines` at or after `from` (1-based;
--- wrapping round), or nil (pure; exposed for the suite).
---@param lines string[]
---@param from integer?
---@return {uri: string, line: integer}?
function M.link_in(lines, from)
  local n = #lines
  from = math.max(1, math.min(from or 1, math.max(n, 1)))
  for k = 0, n - 1 do
    local i = ((from - 1 + k) % n) + 1
    local uri, line = lines[i]:match("%((file://[^)#%s]+)#L(%d+)%)")
    if uri then
      return { uri = uri, line = tonumber(line) }
    end
  end
  return nil
end

--- Close the float and open the link, in the window the hover came from.
---@param win integer the float
---@param link {uri: string, line: integer}
local function jump(win, link)
  if vim.api.nvim_win_is_valid(win) then
    vim.api.nvim_win_close(win, true)
  end
  -- back to a normal window: the one before the float, else the first
  local target = vim.fn.win_getid(vim.fn.winnr("#"))
  if target == 0 or vim.api.nvim_win_get_config(target).relative ~= "" then
    for _, w in ipairs(vim.api.nvim_tabpage_list_wins(0)) do
      if vim.api.nvim_win_get_config(w).relative == "" then
        target = w
        break
      end
    end
  end
  if target ~= 0 and vim.api.nvim_win_is_valid(target) then
    vim.api.nvim_set_current_win(target)
  end
  vim.cmd.edit(vim.fn.fnameescape(vim.uri_to_fname(link.uri)))
  pcall(vim.api.nvim_win_set_cursor, 0, { link.line, 0 })
end

--- Give the float `win` its keys when its text holds a definition link.
---@param win integer
function M.attach(win)
  if not vim.api.nvim_win_is_valid(win) or vim.api.nvim_win_get_config(win).relative == "" then
    return
  end
  local buf = vim.api.nvim_win_get_buf(win)
  if vim.b[buf].cogitator_hover_jump then
    return
  end
  local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
  if not M.link_in(lines) then
    return
  end
  vim.b[buf].cogitator_hover_jump = true
  local function go()
    local here = vim.api.nvim_get_current_win()
    local row = vim.api.nvim_win_get_cursor(here)[1]
    local link = M.link_in(vim.api.nvim_buf_get_lines(buf, 0, -1, false), row)
    if link then
      jump(here, link)
    end
  end
  for _, lhs in ipairs({ "<CR>", "gd" }) do
    vim.keymap.set("n", lhs, go, { buffer = buf, nowait = true, desc = "dbt: go to the definition" })
  end
end

local installed = false

--- Watch for hover floats; idempotent.
function M.setup()
  if installed then
    return
  end
  installed = true
  -- WinEnter comes before the float shows its buffer, and a renderer
  -- (noice) may fill the buffer after opening it: look a tick later
  vim.api.nvim_create_autocmd({ "WinEnter", "BufWinEnter" }, {
    group = vim.api.nvim_create_augroup("CogitatorHoverJump", { clear = true }),
    callback = function()
      local win = vim.api.nvim_get_current_win()
      vim.schedule(function()
        M.attach(win)
      end)
    end,
  })
end

return M
