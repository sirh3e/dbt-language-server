--- workspace/executeCommand wrappers (spec §1.6, §5.1): every dbt.* command
--- takes one argument object { uri, position? }.

local M = {}

local function client_for(bufnr)
  return vim.lsp.get_clients({ bufnr = bufnr, name = "cogitator" })[1]
end

--- The single argument object of spec §1.6 (exposed for the test suite).
function M.build_argument(bufnr, with_position)
  local arg = { uri = vim.uri_from_bufnr(bufnr) }
  if with_position then
    local pos = vim.api.nvim_win_get_cursor(0)
    arg.position = { line = pos[1] - 1, character = pos[2] }
  end
  return arg
end

local function exec(command, with_position, handler)
  local bufnr = vim.api.nvim_get_current_buf()
  local client = client_for(bufnr)
  if not client then
    vim.notify("cogitator is not attached to this buffer", vim.log.levels.ERROR)
    return
  end
  client:exec_cmd({
    command = command,
    arguments = { M.build_argument(bufnr, with_position) },
  }, { bufnr = bufnr }, handler)
end

function M.run()
  exec("dbt.run", false)
end

function M.test()
  exec("dbt.test", false)
end

function M.compile()
  exec("dbt.compile", false) -- success opens the compiled SQL via showDocument
end

function M.lineage()
  exec("dbt.showLineage", false) -- result arrives as a window/showMessage
end

function M.schema()
  local bufnr = vim.api.nvim_get_current_buf()
  local client = client_for(bufnr)
  if not client then
    vim.notify("cogitator is not attached to this buffer", vim.log.levels.ERROR)
    return
  end
  client:exec_cmd({
    command = "dbt.goToSchema",
    arguments = { M.build_argument(bufnr, true) },
  }, { bufnr = bufnr }, function(err, result)
    if err then
      vim.notify("dbt.goToSchema failed: " .. err.message, vim.log.levels.ERROR)
      return
    end
    if not result or result == vim.NIL or not result.uri then
      vim.notify("No schema entry found for this model.", vim.log.levels.INFO)
      return
    end
    vim.lsp.util.show_document(result, client.offset_encoding, { focus = true })
  end)
end

--- Lens-aware wrapper: macro call-site lenses carry command == "" and are
--- informational only (spec §5.2).
function M.run_lens()
  local bufnr = vim.api.nvim_get_current_buf()
  local row = vim.api.nvim_win_get_cursor(0)[1] - 1
  for _, lens in ipairs(vim.lsp.codelens.get(bufnr)) do
    if lens.range.start.line == row and lens.command and lens.command.command == "" then
      vim.notify(lens.command.title .. " (informational lens)", vim.log.levels.INFO)
      return
    end
  end
  vim.lsp.codelens.run()
end

return M
