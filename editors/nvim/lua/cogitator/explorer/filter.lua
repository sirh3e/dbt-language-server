--- The explorer's live filter: fzf's syntax (fuzzy, 'exact, ^prefix,
--- suffix$, !not, a | b) and the `/` getcharstr prompt loop.

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

--- Re-filter the node list with fzf's syntax (cogitator.pick.query):
--- best match first, the matched characters remembered for highlighting.
--- An empty filter keeps every node in its order.
---@param explorer table the cogitator.explorer module
function M.apply(explorer)
  local s = explorer._state
  local names = {}
  for i, n in ipairs(s.nodes) do
    names[i] = n.name
  end
  s.filtered, s.positions = {}, {}
  for _, m in ipairs(require("cogitator.pick.query").rank(names, s.filter)) do
    local node = s.nodes[m.index]
    s.filtered[#s.filtered + 1] = node
    s.positions[node.name] = m.positions
  end
  require("cogitator.explorer.render").center(s)
  s.selected = nil -- force a refetch even when line 1 holds the same model
  explorer._select_current()
end

--- Live filter: printable keys narrow the list on every keystroke, <BS>
--- deletes, <CR> accepts, <Esc> clears and leaves.
---@param explorer table the cogitator.explorer module
function M.prompt(explorer)
  require("cogitator.prompt").run(explorer._state.filter, function(text)
    explorer._state.filter = text
    M.apply(explorer)
  end)
end

return M
