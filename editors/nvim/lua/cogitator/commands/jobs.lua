--- dbt jobs: run, test, compile, parse, freshness, preview, a selection,
--- cancelling one, and the code lens on a line.

local actions = require("cogitator.actions")
local shared = require("cogitator.commands.shared")

local client_for, exec = actions.client_for, shared.exec

local M = {}

function M.run()
  exec("dbt.run", false)
end

function M.test()
  exec("dbt.test", false)
end

function M.compile()
  exec("dbt.compile", false) -- success opens the compiled SQL via showDocument
end

function M.parse()
  exec("dbt.parse", false) -- project-wide; success refreshes the server context
end

function M.source_freshness()
  exec("dbt.sourceFreshness", false) -- project-wide; sources.json feeds the lenses
end

--- :DbtPreview [limit] — `dbt show` for the current model; the result
--- table opens via showDocument when the run succeeds.
function M.preview(opts)
  local bufnr = vim.api.nvim_get_current_buf()
  local client = client_for(bufnr)
  if not client then
    return
  end
  local arg = shared.build_argument(bufnr, false)
  local limit = tonumber(opts and opts.args or "")
  if limit then
    arg.limit = limit
  end
  client:exec_cmd({ command = "dbt.preview", arguments = { arg } }, { bufnr = bufnr })
end

---@type table<integer, string> tokens of dbt jobs this session started
M._jobs = {}

--- :DbtRunSelection — run dbt with an arbitrary selector (tag:nightly,
--- state:modified+, …); the returned token feeds :DbtCancel.
function M.run_selection(opts)
  local bufnr = vim.api.nvim_get_current_buf()
  local client = client_for(bufnr)
  if not client then
    return
  end
  local function start(selector)
    if not selector or selector == "" then
      return
    end
    client:exec_cmd({
      command = "dbt.runSelection",
      arguments = { { uri = vim.uri_from_bufnr(bufnr), selector = selector } },
    }, { bufnr = bufnr }, function(err, result)
      if err then
        vim.notify(err.message, vim.log.levels.ERROR)
      elseif type(result) == "table" and result.token then
        M._jobs[result.token] = "dbt run: " .. selector
        vim.notify(("dbt job %d started (%s)"):format(result.token, selector), vim.log.levels.INFO)
      end
    end)
  end
  if opts and opts.args and opts.args ~= "" then
    start(opts.args)
  else
    require("cogitator.picker").input({ prompt = "dbt selector: " }, start)
  end
end

--- :DbtCancel — pick a running job this session started and cancel it.
function M.cancel()
  local bufnr = vim.api.nvim_get_current_buf()
  local client = client_for(bufnr)
  if not client then
    return
  end
  local tokens = vim.tbl_keys(M._jobs)
  if #tokens == 0 then
    vim.notify("no dbt jobs started from this session", vim.log.levels.INFO)
    return
  end
  table.sort(tokens)
  require("cogitator.pick").select(tokens, {
    prompt = "Cancel dbt job",
    format_item = function(token)
      return ("%d — %s"):format(token, M._jobs[token])
    end,
  }, function(token)
    if not token then
      return
    end
    client:exec_cmd({
      command = "dbt.cancel",
      arguments = { { uri = vim.uri_from_bufnr(bufnr), token = token } },
    }, { bufnr = bufnr }, function(err)
      if err then
        vim.notify(err.message, vim.log.levels.ERROR)
      else
        M._jobs[token] = nil
      end
    end)
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
