--- What every :Dbt* command wrapper shares: the one argument object of
--- spec §1.6 and running a dbt.* command on the current buffer.

local actions = require("cogitator.actions")

local M = {}

--- The single argument object of spec §1.6 (exposed for the test suite).
---@param bufnr integer
---@param with_position boolean include the cursor as a 0-based position
---@return {uri: string, position?: {line: integer, character: integer}}
function M.build_argument(bufnr, with_position)
  local arg = { uri = vim.uri_from_bufnr(bufnr) }
  if with_position then
    local pos = vim.api.nvim_win_get_cursor(0)
    arg.position = { line = pos[1] - 1, character = pos[2] }
  end
  return arg
end

--- Run `command` with the current buffer's argument object.
---@param command string
---@param with_position boolean
---@param handler? fun(err: lsp.ResponseError?, result: any)
function M.exec(command, with_position, handler)
  local bufnr = vim.api.nvim_get_current_buf()
  local client = actions.client_for(bufnr)
  if not client then
    return
  end
  client:exec_cmd({
    command = command,
    arguments = { M.build_argument(bufnr, with_position) },
  }, { bufnr = bufnr }, handler)
end

return M
