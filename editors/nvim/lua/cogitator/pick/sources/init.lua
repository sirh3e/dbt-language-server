--- What the pickers list: every source is a function of the buffer it was
--- opened from, returning a cogitator.pick.Source. Items come from
--- dbt.projectGraph (cached, cogitator.pick.graph), workspace/symbol and
--- the buffer itself; nothing here waits on the server. One table of every
--- source, gathered from a module per origin.

---@class cogitator.pick.Ctx
---@field client vim.lsp.Client
---@field bufnr integer

local M = {}

for _, part in ipairs({ "common", "graph", "symbols", "project", "index" }) do
  for name, value in pairs(require("cogitator.pick.sources." .. part)) do
    assert(M[name] == nil, "cogitator.pick.sources: " .. name .. " is defined twice")
    M[name] = value
  end
end

return M
