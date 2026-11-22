--- Which picker shows a source: setup({ picker = "auto" | "fzf-lua" |
--- "snacks" | "telescope" | "builtin" }). "auto" takes the first one
--- installed in that order; one that is asked for but missing falls back
--- to the built-in picker, saying so once.

local M = {}

M.AUTO_ORDER = { "fzf-lua", "snacks", "telescope" }

local MODULES = {
  ["fzf-lua"] = "cogitator.pick.backends.fzf_lua",
  snacks = "cogitator.pick.backends.snacks",
  telescope = "cogitator.pick.backends.telescope",
  builtin = "cogitator.pick.backends.builtin",
}

M.NAMES = vim.tbl_keys(MODULES)
table.sort(M.NAMES)

---@param name string
---@return table backend {name, available(), open(source, items, run)}
function M.get(name)
  return require(MODULES[name])
end

local warned = {}

--- The backend to use for `name`.
---@param name string
---@return table
function M.resolve(name)
  if name == nil or name == "auto" then
    for _, candidate in ipairs(M.AUTO_ORDER) do
      local backend = M.get(candidate)
      if backend.available() then
        return backend
      end
    end
    return M.get("builtin")
  end
  if not MODULES[name] then
    error(("cogitator: unknown picker %q (auto, %s)"):format(name, table.concat(M.NAMES, ", ")), 0)
  end
  local backend = M.get(name)
  if backend.available() then
    return backend
  end
  if not warned[name] then
    warned[name] = true
    vim.notify(
      ("cogitator: picker %q is not installed; using the built-in one"):format(name),
      vim.log.levels.WARN
    )
  end
  return M.get("builtin")
end

return M
