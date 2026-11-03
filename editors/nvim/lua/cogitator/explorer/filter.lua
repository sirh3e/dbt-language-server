--- The explorer's live filter: smart-case plain-substring matching and
--- the `/` getcharstr prompt loop.

local M = {}

--- Smart-case plain-substring match: a lowercase filter ignores case.
---@param filter string
---@param name string
---@return boolean
function M.matches(filter, name)
  if filter == "" then
    return true
  end
  if filter:lower() == filter then
    return name:lower():find(filter, 1, true) ~= nil
  end
  return name:find(filter, 1, true) ~= nil
end

--- Re-filter the node list, redraw the center, and re-select line 1.
---@param explorer table the cogitator.explorer module
function M.apply(explorer)
  local s = explorer._state
  s.filtered = vim.tbl_filter(function(n)
    return M.matches(s.filter, n.name)
  end, s.nodes)
  require("cogitator.explorer.render").center(s)
  s.selected = nil -- force a refetch even when line 1 holds the same model
  explorer._select_current()
end

--- Live filter: printable keys narrow the list on every keystroke, <BS>
--- deletes, <CR> accepts, <Esc> clears and leaves.
---@param explorer table the cogitator.explorer module
function M.prompt(explorer)
  local s = explorer._state
  local bs = vim.api.nvim_replace_termcodes("<BS>", true, false, true)
  while true do
    vim.cmd.redraw()
    local ok, ch = pcall(vim.fn.getcharstr)
    if not ok or ch == "\27" then
      s.filter = ""
      M.apply(explorer)
      return
    elseif ch == "\r" or ch == "\n" then
      return
    elseif ch == bs then
      s.filter = s.filter:sub(1, -2)
      M.apply(explorer)
    elseif #ch == 1 and ch:match("[%w%p ]") then
      s.filter = s.filter .. ch
      M.apply(explorer)
    else
      return
    end
  end
end

return M
