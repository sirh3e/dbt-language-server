--- The code-action menu (`<leader>dA`, `gra`, `:Dbt actions`, Neovim
--- only) in the yazi-style picker. What is about the cursor comes first,
--- one key away; the rest is grouped below it:
---
---   ✚ fixes, ↻ refactors, ▶ commands     the server's actions at the
---                                         cursor (a diff previews an
---                                         edit), fixes first
---   This model                            the server's actions on the
---                                         whole file and the plugin's
---                                         model commands
---   Views · Project · Session             the plugin's own commands
---                                         (cogitator.dispatch), which a
---                                         stock client could not run:
---                                         it drops what they answer
---
--- The server marks what is about the cursor (see `at_cursor`).
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

local preview = require("cogitator.action_preview")
local command_of = preview.command_of
M.edit_lines = preview.edit_lines
M.action_preview = preview.action_preview

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

--- Is the action about what is under the cursor? The server says so in
--- `data` (`{cogitator = {atCursor = true}}`); a fix always is, as it
--- belongs to the diagnostic there.
---@param action table
---@return boolean
function M.at_cursor(action)
  if type(action.kind) == "string" and vim.startswith(action.kind, "quickfix") then
    return true
  end
  local data = type(action.data) == "table" and action.data.cogitator
  return type(data) == "table" and data.atCursor == true
end

--- Split the server's answer into what is about the cursor and what is
--- about the whole file.
---@param server table[]
---@return table[] at_cursor
---@return table[] model_wide
function M.split(server)
  local at_cursor, model_wide = {}, {}
  for _, action in ipairs(server) do
    if M.at_cursor(action) then
      at_cursor[#at_cursor + 1] = action
    else
      model_wide[#model_wide + 1] = action
    end
  end
  return at_cursor, model_wide
end

--- The server's actions as leaves, fixes first, then refactors, source
--- actions and commands; preferred first within each.
---@param actions table[]
---@param encoding string
---@return cogitator.picker.Item[]
---@return table<string, boolean> skip
local function leaves(actions, encoding)
  local groups, skip = M.server_items(actions, encoding)
  local out = {}
  for _, g in ipairs(groups) do
    vim.list_extend(out, g.children)
  end
  return out, skip
end

--- The whole menu (exposed for the suite): what is about the cursor at
--- the top, chosen with one key; then "This model" (the server's actions
--- on the whole file and the plugin's model commands); then the plugin's
--- views, project and session commands. With nothing at the cursor, the
--- model's actions are the top level themselves.
---@param bufnr integer
---@param server table[] the server's answer
---@param encoding string
---@return cogitator.picker.Item[]
function M.items(bufnr, server, encoding)
  local at_cursor, model_wide = M.split(server)
  local top, skip = leaves(at_cursor, encoding)
  local model, model_skip = leaves(model_wide, encoding)
  for name in pairs(model_skip) do
    skip[name] = true
  end
  local rest = {}
  for _, group in ipairs(dispatch.menu_items(bufnr, skip)) do
    if group.label == "Model" then
      vim.list_extend(model, group.children)
    else
      rest[#rest + 1] = group
    end
  end
  local items = top
  if #top == 0 then
    vim.list_extend(items, model)
  elseif #model > 0 then
    local name = vim.fn.fnamemodify(vim.api.nvim_buf_get_name(bufnr), ":t:r")
    items[#items + 1] = {
      label = name ~= "" and ("This model · " .. name) or "This model",
      icon = "◇",
      detail = tostring(#model),
      children = model,
    }
  end
  vim.list_extend(items, rest)
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
    local name = vim.fn.fnamemodify(vim.api.nvim_buf_get_name(bufnr), ":t")
    picker.open({
      title = name ~= "" and ("actions · " .. name) or "code actions",
      items = M.items(bufnr, server, encoding),
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
