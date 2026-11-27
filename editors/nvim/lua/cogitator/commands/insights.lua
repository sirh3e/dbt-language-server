--- The model's history and cost: :DbtDiffAt (dbt.diffAt), :DbtCostReport
--- (dbt.costReport), :DbtDagText (dbt.dagText) and :DbtProfileColumn
--- (dbt.profileColumn). Rendering is pure (`diff_at_lines`,
--- `cost_report_lines`, `dag_text_lines`, `profile_lines`) so the suite
--- tests it without a server.

local shared = require("cogitator.commands.shared")

local M = {}

local VIEWS = { sql = true, compiled = true, lineage = true }

--- `[rev] [sql|compiled|lineage]` in either order → {rev?, what?}.
---@param args string
---@return {rev?: string, what?: string}
function M.diff_at_args(args)
  local out = {}
  for word in (args or ""):gmatch("%S+") do
    if VIEWS[word] then
      out.what = word
    else
      out.rev = word
    end
  end
  return out
end

--- The lines of a dbt.diffAt answer: the note, the column changes (the
--- lineage view), then the diff — or a line saying nothing differs.
---@param result table
---@return string[]
function M.diff_at_lines(result)
  local lines = {}
  if type(result.note) == "string" then
    lines[#lines + 1] = "-- " .. result.note
  end
  local columns = result.columns
  if type(columns) == "table" then
    if type(columns.unknown) == "string" then
      lines[#lines + 1] = "-- lineage unknown: " .. columns.unknown
    end
    for _, name in ipairs(columns.added or {}) do
      lines[#lines + 1] = "-- + column " .. name
    end
    for _, name in ipairs(columns.removed or {}) do
      lines[#lines + 1] = "-- - column " .. name
    end
    for _, changed in ipairs(columns.changed or {}) do
      lines[#lines + 1] = ("-- ~ column %s: %s → %s"):format(
        changed.name,
        table.concat(changed.before or {}, ", "),
        table.concat(changed.after or {}, ", ")
      )
    end
  end
  if result.identical then
    lines[#lines + 1] = ("-- %s: no %s difference between %s and now"):format(
      tostring(result.model),
      tostring(result.what),
      tostring(result.rev)
    )
    return lines
  end
  vim.list_extend(lines, vim.split((result.diff or ""):gsub("\n$", ""), "\n"))
  return lines
end

local function scratch(lines, filetype)
  vim.cmd.vsplit()
  local buf = vim.api.nvim_create_buf(false, true)
  vim.api.nvim_buf_set_lines(buf, 0, -1, false, lines)
  vim.bo[buf].filetype = filetype
  vim.bo[buf].modifiable = false
  vim.api.nvim_win_set_buf(0, buf)
  vim.keymap.set("n", "q", vim.cmd.close, { buffer = buf, nowait = true })
end

local function failed(command, err)
  vim.notify(command .. " failed" .. (err and (": " .. err.message) or ""), vim.log.levels.ERROR)
end

--- :DbtDiffAt [rev] [sql|compiled|lineage] — this model against itself at
--- a git revision (default HEAD) or `last-run`, as a diff in a split.
---@param opts? {args?: string}
function M.diff_at(opts)
  local parsed = M.diff_at_args(opts and opts.args or "")
  local bufnr = vim.api.nvim_get_current_buf()
  local client = require("cogitator.actions").client_for(bufnr)
  if not client then
    return
  end
  local arg = shared.build_argument(bufnr, false)
  arg.rev, arg.what = parsed.rev, parsed.what
  client:exec_cmd({ command = "dbt.diffAt", arguments = { arg } }, { bufnr = bufnr }, function(err, result)
    if err or type(result) ~= "table" then
      return failed("dbt.diffAt", err)
    end
    scratch(M.diff_at_lines(result), result.identical and "sql" or "diff")
  end)
end

--- The lines of a dbt.costReport answer: the server's text, as it is.
---@param result table
---@return string[]
function M.cost_report_lines(result)
  return vim.split((result.text or "no report"):gsub("\n$", ""), "\n")
end

--- :DbtCostReport — the last run's models by cost, the critical path and
--- the "make it incremental" suggestions, in a split.
function M.cost_report()
  shared.exec("dbt.costReport", false, function(err, result)
    if err or type(result) ~= "table" then
      return failed("dbt.costReport", err)
    end
    scratch(M.cost_report_lines(result), "text")
  end)
end

--- `[depth]` → the dbt.dagText argument beyond the uri: a depth draws the
--- current model's neighbourhood, none the whole project.
---@param args string
---@return {depth?: integer}?
function M.dag_text_args(args)
  local word = vim.trim(args or "")
  if word == "" then
    return {}
  end
  local depth = tonumber(word)
  if not depth or depth < 1 or depth % 1 ~= 0 then
    return nil
  end
  return { depth = depth }
end

--- The lines of a dbt.dagText answer: the server's drawing, as it is.
---@param result table
---@return string[]
function M.dag_text_lines(result)
  return vim.split((result.text or "no DAG"):gsub("\n$", ""), "\n")
end

--- :DbtDagText [depth] — the project DAG drawn as text (what :DbtDag draws,
--- to copy or keep), or with a depth this model's neighbourhood, in a split.
---@param opts? {args?: string}
function M.dag_text(opts)
  local extra = M.dag_text_args(opts and opts.args or "")
  if not extra then
    vim.notify(":DbtDagText takes a depth, a whole number of hops (1 or more)", vim.log.levels.ERROR)
    return
  end
  local bufnr = vim.api.nvim_get_current_buf()
  local client = require("cogitator.actions").client_for(bufnr)
  if not client then
    return
  end
  local arg = shared.build_argument(bufnr, false)
  arg.depth = extra.depth
  client:exec_cmd({ command = "dbt.dagText", arguments = { arg } }, { bufnr = bufnr }, function(err, result)
    if err or type(result) ~= "table" then
      return failed("dbt.dagText", err)
    end
    scratch(M.dag_text_lines(result), "text")
  end)
end

--- The lines of a dbt.profileColumn answer (the hover's markdown).
---@param result table
---@return string[]
function M.profile_lines(result)
  return vim.split(result.hover or "", "\n")
end

--- :DbtProfileColumn — profile the output column under the cursor
--- (warehouse.profile must be on); the profile opens in a float and
--- shows on later hovers.
function M.profile_column()
  vim.notify("profiling the column under the cursor…", vim.log.levels.INFO)
  shared.exec("dbt.profileColumn", true, function(err, result)
    if err or type(result) ~= "table" then
      return failed("dbt.profileColumn", err)
    end
    vim.lsp.util.open_floating_preview(M.profile_lines(result), "markdown", { border = "rounded" })
  end)
end

return M
