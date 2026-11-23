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
    local cogitator = require("cogitator")
    local config = cogitator.config or require("cogitator.config").build({})
    local why = require("cogitator.bootstrap").why_not_attached(bufnr, config, cogitator.server_found ~= false)
    vim.notify("cogitator: " .. why, vim.log.levels.WARN)
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

--- An LSP request that never makes the editor wait: `on_done(err, result)`
--- runs once, on the event loop, with the answer — or with
--- `(nil, nil, "timeout")` after `opts.timeout_ms`, or not at all when
--- `opts.stale()` says the answer no longer applies (the buffer changed,
--- the cursor moved) or the request was cancelled. A late answer after a
--- timeout is dropped.
---@param client vim.lsp.Client
---@param bufnr integer
---@param method string
---@param params table
---@param opts {timeout_ms?: integer, stale?: fun(): boolean}
---@param on_done fun(err: lsp.ResponseError?, result: any, reason?: "timeout")
---@return fun() cancel stops waiting and cancels the request on the server
function M.request(client, bufnr, method, params, opts, on_done)
  local finished = false
  local timer
  local request_id
  local function finish()
    finished = true
    if timer then
      timer:stop()
      timer:close()
      timer = nil
    end
  end
  local ok
  ok, request_id = client:request(method, params, function(err, result)
    if finished then
      return
    end
    finish()
    if opts.stale and opts.stale() then
      return
    end
    on_done(err, result)
  end, bufnr)
  if not ok then
    finish()
    on_done({ code = -32603, message = "the request could not be sent" }, nil)
    return function() end
  end
  if opts.timeout_ms and not finished then
    timer = vim.uv.new_timer()
    timer:start(
      opts.timeout_ms,
      0,
      vim.schedule_wrap(function()
        if finished then
          return
        end
        finish()
        on_done(nil, nil, "timeout")
      end)
    )
  end
  return function()
    if finished then
      return
    end
    finish()
    if request_id then
      pcall(client.cancel_request, client, request_id)
    end
  end
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
