--- :DbtSyncYaml — the schema-YAML sync (dbt.syncYaml): the plan in a
--- scratch split (a summary per model, then each file's diff), then the
--- edit applied on confirm. The server sends the edit as
--- workspace/applyEdit, which Neovim applies by itself.

local actions = require("cogitator.actions")

local M = {}

--- The dbt.syncYaml argument of `:DbtSyncYaml` words (exposed for the
--- test suite). `%` (or `this`) narrows to the current buffer's model or
--- the models of its entries; `prune`, `types`, `reorder` and `folder`
--- are flags; anything else is a model name.
---@param words string[]
---@param uri string the current buffer's uri
---@return table
function M.arguments(words, uri)
  local arg = {}
  local models = {}
  local flags = {
    prune = "prune",
    types = "inheritTypes",
    reorder = "reorder",
  }
  for _, word in ipairs(words) do
    if flags[word] then
      arg[flags[word]] = true
    elseif word == "folder" then
      arg.newEntries = "folder"
    elseif word == "%" or word == "this" then
      arg.uri = uri
    else
      models[#models + 1] = word
    end
  end
  if #models > 0 then
    arg.models = models
  end
  return arg
end

local function from_text(from)
  if type(from) ~= "table" then
    return "?"
  end
  if from.kind == "source" then
    return ("source %s.%s.%s"):format(from.source, from.table, from.column)
  end
  return ("%s.%s"):format(tostring(from.name), tostring(from.column))
end

--- The preview lines of a dbt.syncYaml answer (exposed for the test
--- suite): a line per change, stale column and skip of each model, then
--- every file's unified diff.
---@param result table
---@return string[]
function M.lines(result)
  local lines = {
    ("# dbt.syncYaml: %d change(s) in %d file(s)"):format(result.changes or 0, #(result.files or {})),
  }
  for _, model in ipairs(result.models or {}) do
    local body = {}
    if model.createdEntry then
      body[#body + 1] = "  + entry"
    end
    for _, column in ipairs(model.added or {}) do
      body[#body + 1] = "  + column " .. column
    end
    for _, inherited in ipairs(model.inherited or {}) do
      body[#body + 1] = ("  ~ %s description from %s"):format(inherited.column, from_text(inherited.from))
    end
    for _, typed in ipairs(model.typed or {}) do
      body[#body + 1] = ("  ~ %s data_type %s"):format(typed.column, typed.dataType)
    end
    for _, column in ipairs(model.pruned or {}) do
      body[#body + 1] = "  - column " .. column
    end
    if model.reordered then
      body[#body + 1] = "  ~ columns reordered"
    end
    local pruned = {}
    for _, column in ipairs(model.pruned or {}) do
      pruned[column] = true
    end
    for _, column in ipairs(model.stale or {}) do
      if not pruned[column] then
        body[#body + 1] = ("  ! stale column %s (prune removes it)"):format(column)
      end
    end
    for _, skip in ipairs(model.skipped or {}) do
      local column = skip.column ~= vim.NIL and skip.column or nil
      body[#body + 1] = ("  ? %s%s"):format(column and (column .. ": ") or "", skip.reason)
    end
    if #body > 0 then
      lines[#lines + 1] = "# " .. model.model
      vim.list_extend(lines, body)
    end
  end
  for _, file in ipairs(result.files or {}) do
    lines[#lines + 1] = ""
    vim.list_extend(lines, vim.split((file.diff or ""):gsub("\n$", ""), "\n"))
  end
  return lines
end

local function show(lines)
  vim.cmd.vsplit()
  local scratch = vim.api.nvim_create_buf(false, true)
  vim.api.nvim_buf_set_lines(scratch, 0, -1, false, lines)
  vim.bo[scratch].filetype = "diff"
  vim.bo[scratch].modifiable = false
  vim.api.nvim_win_set_buf(0, scratch)
  vim.keymap.set("n", "q", vim.cmd.close, { buffer = scratch, nowait = true })
end

--- :DbtSyncYaml [model…] [%] [prune] [types] [reorder] [folder]
---@param opts {fargs?: string[], args?: string}?
function M.run(opts)
  local bufnr = vim.api.nvim_get_current_buf()
  local client = actions.client_for(bufnr)
  if not client then
    return
  end
  local words = (opts and opts.fargs) or vim.split((opts and opts.args) or "", "%s+", { trimempty = true })
  local arg = M.arguments(words, vim.uri_from_bufnr(bufnr))
  client:exec_cmd({ command = "dbt.syncYaml", arguments = { arg } }, { bufnr = bufnr }, function(err, result)
    if err or type(result) ~= "table" then
      vim.notify("dbt.syncYaml failed" .. (err and (": " .. err.message) or ""), vim.log.levels.ERROR)
      return
    end
    if (result.changes or 0) == 0 then
      vim.notify("schema YAML is in sync — nothing to change", vim.log.levels.INFO)
      return
    end
    show(M.lines(result))
    local prompt = ("Apply %d change(s) to %d file(s)?"):format(result.changes, #(result.files or {}))
    require("cogitator.picker").select({ "Apply", "Cancel" }, { prompt = prompt }, function(choice)
      if choice ~= "Apply" then
        return
      end
      local applied = vim.tbl_extend("force", arg, { apply = true })
      client:exec_cmd({ command = "dbt.syncYaml", arguments = { applied } }, { bufnr = bufnr }, function(e)
        if e then
          vim.notify("dbt.syncYaml failed: " .. e.message, vim.log.levels.ERROR)
        end
      end)
    end)
  end)
end

return M
