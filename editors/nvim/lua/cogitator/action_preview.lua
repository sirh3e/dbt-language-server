--- Previews of the server's code actions for the code-action menu: what
--- a command runs, and the diff of what an edit changes. Pure but for
--- reading the files an edit touches.

local M = {}

--- What a server command does, for the preview.
M.COMMAND_TEXT = {
  ["dbt.run"] = "runs `dbt run` for this model as a job",
  ["dbt.test"] = "runs `dbt test` for this model as a job",
  ["dbt.compile"] = "runs `dbt compile` and opens the compiled SQL",
  ["dbt.openSchema"] = "opens the model's entry in its properties YAML",
  ["dbt.showLineage"] = "shows the model's column lineage",
  ["dbt.goToTests"] = "jumps to a test of this model",
  ["dbt.extractModelYaml"] = "writes a properties-YAML entry for this model",
  ["dbt.deps"] = "runs `dbt deps` for the project",
  ["dbt.setTarget"] = "switches the active profile/target",
  ["dbt.openModel"] = "opens the model",
}

--- The command a code action (or a bare Command) runs, if any.
---@param action table
---@return table?
function M.command_of(action)
  if type(action.command) == "string" then
    return action
  end
  return type(action.command) == "table" and action.command or nil
end

--- Read a file's lines: the loaded buffer's when there is one.
---@param fname string
---@return string[]
local function lines_of(fname)
  local buf = vim.fn.bufnr(fname)
  if buf ~= -1 and vim.api.nvim_buf_is_loaded(buf) then
    return vim.api.nvim_buf_get_lines(buf, 0, -1, false)
  end
  if vim.uv.fs_stat(fname) then
    return vim.fn.readfile(fname)
  end
  return {}
end

local relative = require("cogitator.paths").relative_uri

--- A unified diff of what text edits do to one file.
---@param uri string
---@param edits table[] TextEdit[]
---@param encoding string
---@return string[]
local function file_diff(uri, edits, encoding)
  local fname = vim.uri_to_fname(uri)
  local old = lines_of(fname)
  local scratch = vim.api.nvim_create_buf(false, true)
  vim.api.nvim_buf_set_lines(scratch, 0, -1, false, old)
  local ok, err = pcall(vim.lsp.util.apply_text_edits, edits, scratch, encoding)
  local new = vim.api.nvim_buf_get_lines(scratch, 0, -1, false)
  vim.api.nvim_buf_delete(scratch, { force = true })
  if not ok then
    return { "  (cannot preview " .. relative(uri) .. ": " .. tostring(err) .. ")" }
  end
  local diff = (vim.text and vim.text.diff) or vim.diff
  local function text(t)
    return #t == 0 and "" or (table.concat(t, "\n") .. "\n")
  end
  local body = diff(text(old), text(new), { result_type = "unified", ctxlen = 2 })
  local out = { "--- " .. relative(uri), "+++ " .. relative(uri) }
  vim.list_extend(out, vim.split(vim.trim(body or ""), "\n", { trimempty = true }))
  return out
end

--- The preview of a WorkspaceEdit: file operations, then a diff per file.
---@param edit table
---@param encoding string
---@return string[]
function M.edit_lines(edit, encoding)
  local out = {}
  local function add(lines)
    if #out > 0 then
      out[#out + 1] = ""
    end
    vim.list_extend(out, lines)
  end
  for _, change in ipairs(edit.documentChanges or {}) do
    if change.kind == "create" then
      add({ "new file  " .. relative(change.uri) })
    elseif change.kind == "rename" then
      add({ "rename    " .. relative(change.oldUri) .. " → " .. relative(change.newUri) })
    elseif change.kind == "delete" then
      add({ "delete    " .. relative(change.uri) })
    elseif change.textDocument then
      add(file_diff(change.textDocument.uri, change.edits or {}, encoding))
    end
  end
  for uri, edits in pairs(edit.changes or {}) do
    add(file_diff(uri, edits, encoding))
  end
  return out
end

--- The preview of one server action.
---@param action table
---@param encoding string
---@return {lines: string[], syntax: string?}
function M.action_preview(action, encoding)
  local lines = { action.title }
  local meta = {}
  if action.kind then
    meta[#meta + 1] = action.kind
  end
  if action.isPreferred then
    meta[#meta + 1] = "preferred"
  end
  if #meta > 0 then
    lines[#lines + 1] = table.concat(meta, " · ")
  end
  for _, d in ipairs(action.diagnostics or {}) do
    lines[#lines + 1] = "fixes: " .. (d.message or ""):gsub("\n.*", "")
  end
  if action.disabled then
    lines[#lines + 1] = "unavailable: " .. (action.disabled.reason or "")
  end
  lines[#lines + 1] = ""
  if action.edit then
    vim.list_extend(lines, M.edit_lines(action.edit, encoding))
  end
  local cmd = M.command_of(action)
  if cmd then
    if action.edit then
      lines[#lines + 1] = ""
    end
    lines[#lines + 1] = "runs " .. cmd.command
    if M.COMMAND_TEXT[cmd.command] then
      lines[#lines + 1] = "  " .. M.COMMAND_TEXT[cmd.command]
    end
    for _, arg in ipairs(cmd.arguments or {}) do
      vim.list_extend(lines, vim.split(vim.inspect(arg), "\n"))
    end
  end
  if not action.edit and not cmd then
    lines[#lines + 1] = "(the server fills this action in when it is chosen)"
  end
  return { lines = lines, syntax = action.edit and "diff" or nil }
end

return M
