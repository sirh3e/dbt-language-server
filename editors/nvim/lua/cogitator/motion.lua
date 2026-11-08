--- Vim-motion plumbing shared by the graph views.

local M = {}

--- A buffer-local normal-mode map that doesn't wait for longer mappings.
---@param buf integer
---@param lhs string
---@param fn fun()
---@param desc string?
function M.map(buf, lhs, fn, desc)
  vim.keymap.set("n", lhs, fn, { buffer = buf, nowait = true, desc = desc })
end

--- Wrap `fn` to run once per [count] (default once).
---@param fn fun()
---@return fun()
function M.repeated(fn)
  return function()
    for _ = 1, vim.v.count1 do
      fn()
    end
  end
end

--- Scroll another window by `lines` (negative = up) without focusing it.
---@param win integer
---@param lines integer
function M.scroll(win, lines)
  if not vim.api.nvim_win_is_valid(win) then
    return
  end
  local key = vim.keycode(lines > 0 and "<C-e>" or "<C-y>")
  vim.api.nvim_win_call(win, function()
    vim.cmd.normal({ args = { math.abs(lines) .. key }, bang = true })
  end)
end

return M
