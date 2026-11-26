--- workspace/executeCommand wrappers (spec §1.6, §5.1): every dbt.* command
--- takes one argument object { uri, position? }. One table of every :Dbt*
--- command, gathered from a module per concern; callers (and the suite,
--- which may replace a function here) use this table, never a submodule.

local M = {}

for _, part in ipairs({
  "shared",
  "jobs",
  "lists",
  "analysis",
  "views",
  "node",
  "split",
  "navigation",
  "insights",
}) do
  for name, value in pairs(require("cogitator.commands." .. part)) do
    assert(M[name] == nil, "cogitator.commands: " .. name .. " is defined twice")
    M[name] = value
  end
end

return M
