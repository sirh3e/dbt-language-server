--- Shared LSP action plumbing: the one client lookup (was copied in
--- eight places), executeCommand dispatch, and file navigation.

local M = {}

--- The cogitator client attached to a buffer, or nil (with a notify
--- unless silent).
---@param bufnr integer
---@param silent boolean? suppress the "not attached" notification
---@return vim.lsp.Client?
function M.client_for(bufnr, silent)
  local client = vim.lsp.get_clients({ bufnr = bufnr, name = "cogitator" })[1]
  if not client and not silent then
    vim.notify("cogitator is not attached to this buffer", vim.log.levels.ERROR)
  end
  return client
end

--- Run a dbt.* command on an explicit client (views keep theirs in
--- state); the handler is optional for fire-and-forget commands.
---@param client vim.lsp.Client
---@param bufnr integer
---@param command string e.g. "dbt.modelInfo"
---@param arg table the single argument object of spec §1.6
---@param handler? fun(err: lsp.ResponseError?, result: any)
function M.exec_with(client, bufnr, command, arg, handler)
  client:exec_cmd({ command = command, arguments = { arg } }, { bufnr = bufnr }, handler)
end

--- Run a dbt.* command against the current buffer's client.
---@param command string
---@param arg table
---@param handler? fun(err: lsp.ResponseError?, result: any)
---@return boolean dispatched false when no client is attached
function M.exec(command, arg, handler)
  local bufnr = vim.api.nvim_get_current_buf()
  local client = M.client_for(bufnr)
  if not client then
    return false
  end
  M.exec_with(client, bufnr, command, arg, handler)
  return true
end

--- Open the file behind a uri; warns on entries without one. Returns
--- whether a file was opened (callers close their view first).
---@param uri string|nil|vim.NIL
---@return boolean opened
function M.open_uri(uri)
  if not uri or uri == vim.NIL then
    vim.notify("no file on disk for this entry", vim.log.levels.WARN)
    return false
  end
  vim.cmd.edit(vim.uri_to_fname(uri --[[@as string]]))
  return true
end

return M
