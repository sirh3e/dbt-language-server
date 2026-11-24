--- :DbtRename[!] [new_name] — rename a model or a macro (or anything the
--- server renames: a var, a column, a CTE) across the project, through
--- the yazi-style dialogs: the new name is asked for, then the whole
--- WorkspaceEdit is shown — the file rename and a diff per file, walked
--- with j/k and filtered with `/` — and applied only when confirmed.
---
--- What is renamed: what the cursor names (prepareRename), else the
--- file's own model; `!` always the file's model. `summary` and `items`
--- are pure, for the suite.

local actions = require("cogitator.actions")

local M = {}

local relative = require("cogitator.paths").relative_uri

--- What an edit does: how many files it edits, and its file renames.
---@param edit table WorkspaceEdit
---@return {files: integer, edits: integer, renames: {from: string, to: string}[]}
function M.summary(edit)
  local out = { files = 0, edits = 0, renames = {} }
  for _, change in ipairs(edit.documentChanges or {}) do
    if change.kind == "rename" then
      out.renames[#out.renames + 1] = { from = change.oldUri, to = change.newUri }
    elseif change.textDocument then
      out.files = out.files + 1
      out.edits = out.edits + #(change.edits or {})
    end
  end
  for _, edits in pairs(edit.changes or {}) do
    out.files = out.files + 1
    out.edits = out.edits + #edits
  end
  return out
end

--- The confirmation dialog's items: apply, cancel, then a group with a
--- diff per changed file.
---@param edit table WorkspaceEdit
---@param from string
---@param to string
---@param encoding string
---@return cogitator.picker.Item[]
function M.items(edit, from, to, encoding)
  local ca = require("cogitator.code_actions")
  local s = M.summary(edit)
  local what = ("%d %s in %d %s"):format(
    s.edits,
    s.edits == 1 and "edit" or "edits",
    s.files,
    s.files == 1 and "file" or "files"
  )
  if #s.renames > 0 then
    what = what .. (", %d file %s"):format(#s.renames, #s.renames == 1 and "rename" or "renames")
  end
  local files = {}
  local function add(change, label)
    files[#files + 1] = {
      label = label,
      disabled = "this only shows the change — choose “apply” to rename",
      preview = function()
        return { lines = ca.edit_lines({ documentChanges = { change } }, encoding), syntax = "diff" }
      end,
    }
  end
  for _, change in ipairs(edit.documentChanges or {}) do
    if change.kind == "rename" then
      add(change, ("%s → %s"):format(relative(change.oldUri), relative(change.newUri)))
    elseif change.textDocument then
      add(change, ("%s  (%d)"):format(relative(change.textDocument.uri), #(change.edits or {})))
    end
  end
  for uri, edits in pairs(edit.changes or {}) do
    add({ textDocument = { uri = uri }, edits = edits }, ("%s  (%d)"):format(relative(uri), #edits))
  end
  return {
    {
      label = ("apply: %s → %s"):format(from, to),
      icon = "✓",
      icon_hl = "DbtPickerPreferred",
      detail = what,
      value = "apply",
      preview = function()
        return { lines = ca.edit_lines(edit, encoding), syntax = "diff" }
      end,
    },
    { label = "cancel", icon = "✗", value = "cancel" },
    { label = "changes", icon = "±", detail = what, children = files },
  }
end

--- Apply a confirmed edit and say what changed.
local function apply(client, edit, from, to)
  vim.lsp.util.apply_workspace_edit(edit, client.offset_encoding)
  local s = M.summary(edit)
  vim.notify(
    ("renamed %s → %s in %d %s — :wall to save"):format(
      from,
      to,
      s.files,
      s.files == 1 and "file" or "files"
    ),
    vim.log.levels.INFO
  )
end

--- Ask the server for the edit of `subject` → `new_name`, then confirm it.
local function ask_edit(client, bufnr, subject, new_name)
  actions.request(client, bufnr, "textDocument/rename", {
    textDocument = { uri = subject.uri },
    position = subject.position,
    newName = new_name,
  }, { timeout_ms = 30000 }, function(err, result, reason)
    if err or reason then
      vim.notify("rename: " .. (reason or err.message or "failed"), vim.log.levels.ERROR)
      return
    end
    if type(result) ~= "table" or (not result.documentChanges and not result.changes) then
      vim.notify(("rename: the server cannot rename %s here"):format(subject.name), vim.log.levels.WARN)
      return
    end
    require("cogitator.picker").open({
      title = ("rename %s → %s"):format(subject.name, new_name),
      items = M.items(result, subject.name, new_name, client.offset_encoding),
      on_choose = function(item)
        if item.value == "apply" then
          apply(client, result, subject.name, new_name)
        end
      end,
    })
  end)
end

--- Rename `subject`: ask for the new name unless given, then confirm.
---@param client vim.lsp.Client
---@param bufnr integer
---@param subject {uri: string, position: table, name: string}
---@param new_name string?
function M.run(client, bufnr, subject, new_name)
  if new_name and new_name ~= "" then
    return ask_edit(client, bufnr, subject, new_name)
  end
  require("cogitator.picker").input(
    { prompt = "rename " .. subject.name, default = subject.name },
    function(text)
      text = text and vim.trim(text)
      if not text or text == "" or text == subject.name then
        return
      end
      ask_edit(client, bufnr, subject, text)
    end
  )
end

--- The name a prepareRename answer offers.
---@param result table
---@param bufnr integer
---@return string?
function M.placeholder(result, bufnr)
  if result.placeholder then
    return result.placeholder
  end
  local range = result.range or (result.start and result)
  if not range then
    return nil
  end
  local line = vim.api.nvim_buf_get_lines(bufnr, range.start.line, range.start.line + 1, false)[1]
  return line and line:sub(range.start.character + 1, range["end"].character) or nil
end

--- :DbtRename[!] [new_name].
---@param opts {bang?: boolean, args?: string}?
function M.rename(opts)
  opts = opts or {}
  local bufnr = vim.api.nvim_get_current_buf()
  local client = actions.client_for(bufnr)
  if not client then
    return
  end
  local usages = require("cogitator.usages")
  local new_name = opts.args ~= "" and opts.args or nil
  if opts.bang then
    return M.run(client, bufnr, usages.subject(bufnr, true), new_name)
  end
  local here = usages.subject(bufnr, false, client.offset_encoding)
  actions.request(client, bufnr, "textDocument/prepareRename", {
    textDocument = { uri = here.uri },
    position = here.position,
  }, { timeout_ms = 5000 }, function(err, result)
    if err then
      -- the server's refusal says why (an installed package's macro)
      vim.notify("rename: " .. (err.message or "refused"), vim.log.levels.WARN)
      return
    end
    local name = type(result) == "table" and M.placeholder(result, bufnr)
    if name and name ~= "" then
      here.name = name
      return M.run(client, bufnr, here, new_name)
    end
    -- nothing renameable under the cursor: the file's own model
    M.run(client, bufnr, usages.subject(bufnr, true), new_name)
  end)
end

return M
