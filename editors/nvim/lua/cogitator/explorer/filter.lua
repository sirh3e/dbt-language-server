--- Smart-case substring matching, for :DbtDag's search. (The explorer
--- chooses its model with the fuzzy picker, cogitator.pick.)

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

return M
