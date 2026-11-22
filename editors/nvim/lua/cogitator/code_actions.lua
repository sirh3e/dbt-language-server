--- The code-action menu (`<leader>da`, `gra`, `:Dbt actions`, Neovim
--- only) in the yazi-style picker. It shows everything that can be done
--- at the cursor in one tree:
---
---   Quick fixes · Refactor · Source · dbt   the server's code actions,
---                                           by kind, with a diff of
---                                           what an edit changes
---   Model · Views · Project · Session       the plugin's own commands
---                                           (cogitator.dispatch), which
---                                           the server cannot offer as
---                                           code actions: a stock client
---                                           drops what they answer
---
--- A server command the plugin also has (dbt.run → :Dbt run) is listed
--- once, as the server's.

local actions = require("cogitator.actions")
local dispatch = require("cogitator.dispatch")
local picker = require("cogitator.picker")

local M = {}

--- The server commands a plugin subcommand duplicates.
M.SAME_AS = {
  ["dbt.run"] = "run",
  ["dbt.test"] = "test",
  ["dbt.compile"] = "compile",
  ["dbt.openSchema"] = "schema",
  ["dbt.showLineage"] = "lineage",
  ["dbt.goToTests"] = "tests",
  ["dbt.extractModelYaml"] = "extract",
  ["dbt.deps"] = "deps",
}

--- What a server command does, for the preview.
local COMMAND_TEXT = {
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

--- The server action groups, by the kind's first part.
M.KINDS = {
  { prefix = "quickfix", name = "Quick fixes", icon = "✚", hl = "DbtPickerQuickfix" },
  { prefix = "refactor", name = "Refactor", icon = "↻", hl = "DbtPickerRefactor" },
  { prefix = "source", name = "Source", icon = "◉", hl = "DbtPickerSource" },
  { prefix = "", name = "dbt", icon = "▶", hl = "DbtPickerCommand" },
}

--- The group a code action's kind falls in.
---@param kind string?
---@return table
function M.kind_group(kind)
  for _, g in ipairs(M.KINDS) do
    if g.prefix ~= "" and kind and (kind == g.prefix or vim.startswith(kind, g.prefix .. ".")) then
      return g
    end
  end
  return M.KINDS[#M.KINDS]
end

--- The command a code action (or a bare Command) runs, if any.
---@param action table
---@return table?
local function command_of(action)
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

local function relative(uri)
  return vim.fn.fnamemodify(vim.uri_to_fname(uri), ":~:.")
end

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
  local cmd = command_of(action)
  if cmd then
    if action.edit then
      lines[#lines + 1] = ""
    end
    lines[#lines + 1] = "runs " .. cmd.command
    if COMMAND_TEXT[cmd.command] then
      lines[#lines + 1] = "  " .. COMMAND_TEXT[cmd.command]
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

--- Picker items of the server's actions, grouped by kind (groups in
--- KINDS order; actions keep the server's order, preferred first).
---@param server table[] code actions and commands as answered
---@param encoding string
---@return cogitator.picker.Item[]
---@return table<string, boolean> skip the plugin subcommands they cover
function M.server_items(server, encoding)
  local by_group, skip = {}, {}
  for _, action in ipairs(server) do
    local g = M.kind_group(type(action.command) == "string" and nil or action.kind)
    local cmd = command_of(action)
    if cmd and M.SAME_AS[cmd.command] then
      skip[M.SAME_AS[cmd.command]] = true
    end
    by_group[g] = by_group[g] or {}
    table.insert(by_group[g], {
      label = action.title,
      icon = action.isPreferred and "★" or g.icon,
      icon_hl = action.isPreferred and "DbtPickerPreferred" or g.hl,
      detail = action.kind and action.kind:gsub("^" .. vim.pesc(g.prefix) .. "%.?", "") or nil,
      disabled = action.disabled and (action.disabled.reason or "disabled") or nil,
      value = { action = action },
      preferred = action.isPreferred,
      preview = function()
        return M.action_preview(action, encoding)
      end,
    })
  end
  local items = {}
  for _, g in ipairs(M.KINDS) do
    local children = by_group[g]
    if children then
      table.sort(children, function(a, b)
        return (a.preferred and 1 or 0) > (b.preferred and 1 or 0)
      end)
      for i, c in ipairs(children) do
        c.detail = c.detail ~= "" and c.detail or nil
        children[i] = c
      end
      items[#items + 1] = {
        label = g.name,
        icon = g.icon,
        icon_hl = g.hl,
        detail = tostring(#children),
        children = children,
      }
    end
  end
  return items, skip
end

--- Run a server action: resolve it when it comes without an edit or a
--- command, apply its edit, then run its command.
---@param client vim.lsp.Client
---@param bufnr integer
---@param action table
function M.apply(client, bufnr, action)
  if type(action.command) == "string" then
    client:exec_cmd(action, { bufnr = bufnr })
    return
  end
  local function run(a)
    if a.edit then
      vim.lsp.util.apply_workspace_edit(a.edit, client.offset_encoding)
    end
    if type(a.command) == "table" then
      client:exec_cmd(a.command, { bufnr = bufnr })
    end
  end
  if not action.edit and not action.command and client:supports_method("codeAction/resolve") then
    client:request("codeAction/resolve", action, function(err, resolved)
      if err then
        vim.notify("codeAction/resolve failed: " .. err.message, vim.log.levels.ERROR)
        return
      end
      run(resolved or action)
    end, bufnr)
    return
  end
  run(action)
end

--- The LSP diagnostics of `client` on lines first..last (0-based).
local function diagnostics_on(bufnr, client, first, last)
  local namespaces = {}
  for _, pull in ipairs({ false, true }) do
    local ok, ns = pcall(vim.lsp.diagnostic.get_namespace, client.id, pull)
    if ok then
      namespaces[ns] = true
    end
  end
  local out = {}
  for _, d in ipairs(vim.diagnostic.get(bufnr)) do
    if namespaces[d.namespace] and d.lnum >= first and d.lnum <= last and d.user_data and d.user_data.lsp then
      out[#out + 1] = d.user_data.lsp
    end
  end
  return out
end

--- The request's range: the visual selection, else the cursor.
local function range_params(bufnr, encoding)
  local mode = vim.fn.mode()
  if mode:match("^[vV\22]") then
    local a, b = vim.fn.getpos("v"), vim.fn.getpos(".")
    if a[2] > b[2] or (a[2] == b[2] and a[3] > b[3]) then
      a, b = b, a
    end
    vim.cmd("normal! \27")
    local stop = b[3]
    if mode == "V" then
      a[3] = 1
      stop = #(vim.api.nvim_buf_get_lines(bufnr, b[2] - 1, b[2], false)[1] or "")
    end
    return vim.lsp.util.make_given_range_params(
      { a[2], a[3] - 1 },
      { b[2], math.max(stop - 1, 0) },
      bufnr,
      encoding
    )
  end
  return vim.lsp.util.make_range_params(0, encoding)
end

--- The whole menu's items for an answer (exposed for the suite).
---@param bufnr integer
---@param server table[]
---@param encoding string
---@return cogitator.picker.Item[]
function M.items(bufnr, server, encoding)
  local items, skip = M.server_items(server, encoding)
  vim.list_extend(items, dispatch.menu_items(bufnr, skip))
  return items
end

--- Open the code-action menu at the cursor (or for the selection).
---@param opts? {only?: string[]}
function M.open(opts)
  opts = type(opts) == "table" and opts or {}
  local bufnr = vim.api.nvim_get_current_buf()
  local client = actions.client_for(bufnr)
  if not client then
    return
  end
  local encoding = client.offset_encoding or "utf-16"
  local params = range_params(bufnr, encoding)
  local first, last = params.range.start.line, params.range["end"].line
  params.context = {
    diagnostics = diagnostics_on(bufnr, client, first, last),
    only = opts.only,
    triggerKind = 1,
  }
  client:request("textDocument/codeAction", params, function(err, result)
    if err then
      vim.notify("textDocument/codeAction failed: " .. err.message, vim.log.levels.ERROR)
    end
    local server = (type(result) == "table") and result or {}
    local items = M.items(bufnr, server, encoding)
    -- the groups come in KINDS order, so the cursor starts on the
    -- quick fixes whenever there are any
    picker.open({
      title = "code actions",
      items = items,
      on_choose = function(item)
        if item.value.action then
          M.apply(client, bufnr, item.value.action)
        else
          dispatch.SUBCOMMANDS[item.value.subcommand].run("")
        end
      end,
    })
  end, bufnr)
end

return M
