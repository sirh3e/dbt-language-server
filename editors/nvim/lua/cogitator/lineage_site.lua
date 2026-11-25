--- :DbtLineageSite — the project's column-lineage page (dbt.lineageSite
--- with write = true): the server writes target/cogitator-lineage/index.html
--- and the page opens in the browser. Nothing blocks: the answer arrives
--- in a callback.

local actions = require("cogitator.actions")

local M = {}

--- The message for a dbt.lineageSite answer (exposed for the test suite).
---@param result table|nil
---@return string text, integer level
function M.message(result)
  if type(result) ~= "table" or type(result.written) ~= "table" then
    return "dbt.lineageSite: no dbt project is loaded, so there is no page to write", vim.log.levels.WARN
  end
  local c = result.counts or {}
  return string.format(
    "lineage page: %s — %d of %d columns traced, %d undetermined, %d models declined",
    result.written.path,
    c.columnsTraced or 0,
    c.columns or 0,
    c.columnsUndetermined or 0,
    c.modelsDeclined or 0
  ),
    vim.log.levels.INFO
end

--- Writes the page and opens it with vim.ui.open.
function M.open()
  local bufnr = vim.api.nvim_get_current_buf()
  local client = actions.client_for(bufnr)
  if not client then
    return
  end
  client:exec_cmd({
    command = "dbt.lineageSite",
    arguments = { { uri = vim.uri_from_bufnr(bufnr), write = true } },
  }, { bufnr = bufnr }, function(err, result)
    if err then
      vim.notify("dbt.lineageSite failed: " .. err.message, vim.log.levels.ERROR)
      return
    end
    local text, level = M.message(result)
    vim.notify(text, level)
    if level == vim.log.levels.INFO then
      local _, open_err = vim.ui.open(result.written.path)
      if open_err then
        vim.notify("could not open the page: " .. open_err .. " (open it yourself)", vim.log.levels.WARN)
      end
    end
  end)
end

return M
