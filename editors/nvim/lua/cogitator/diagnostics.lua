--- The server's problems in the editor: what is wrong with a model and
--- what would fix it, where the eye already is.
---
---   virtual text  the problem's first sentence and, when the server
---                 carries a fix, its suggestion and the key that applies
---                 it: `materialization 'incrementl' does not exist — did
---                 you mean 'incremental'? ✚ <leader>di`
---   float         the whole message, then each fix by its title
---   :Dbt fix      (<leader>di) the fix of the problem under the cursor:
---                 applied when there is one, chosen in the code-action
---                 menu when there are several
---   :Dbt problems (<leader>dI) this file's problems in the location list
---
--- Only the cogitator client's namespaces are configured; other servers'
--- diagnostics keep the user's own settings.

local M = {}

--- The key `:Dbt fix` is mapped to (nil while keymaps are off).
M.FIX_KEY = "<leader>di"
M.PROBLEMS_KEY = "<leader>dI"

--- The LSP diagnostic behind a vim.Diagnostic, if it came over LSP.
---@param d vim.Diagnostic
---@return table?
local function lsp_of(d)
  return type(d.user_data) == "table" and type(d.user_data.lsp) == "table" and d.user_data.lsp or nil
end

--- What would fix the problem, as the server carries it in `data`: the
--- did-you-mean suggestions, or the titles of a multi-edit's fixes.
---@param d vim.Diagnostic|table a vim.Diagnostic or an LSP diagnostic
---@return string[] fixes, closest first
function M.fixes(d)
  local lsp = d.user_data and lsp_of(d) or d
  local data = type(lsp) == "table" and lsp.data
  if type(data) ~= "table" then
    return {}
  end
  if type(data.fix) == "table" and type(data.fix.multi_edit) == "table" then
    local titles = {}
    for _, fix in ipairs(data.fix.multi_edit.fixes or {}) do
      titles[#titles + 1] = fix.title
    end
    return titles
  end
  local out = {}
  for _, s in ipairs(type(data.suggestions) == "table" and data.suggestions or {}) do
    local label = (data.fix == "insert_text" and "insert '%s'" or "'%s'"):format(s)
    out[#out + 1] = label
  end
  return out
end

--- The problem's first sentence: what is wrong, and a did-you-mean.
---@param message string
---@return string
function M.headline(message)
  local first = message:match("^[^\n]*") or message
  -- "… did you mean 'x'? The reason." keeps the question, drops the reason
  local question = first:find("? ", 1, true)
  if question then
    return first:sub(1, question)
  end
  local stop = first:find(". ", 1, true)
  return stop and first:sub(1, stop - 1) or first
end

--- The inline text of a diagnostic (vim.diagnostic.config format).
---@param d vim.Diagnostic
---@return string
function M.inline(d)
  local text = M.headline(d.message)
  if #M.fixes(d) > 0 and M.FIX_KEY then
    text = text .. " ✚ " .. M.FIX_KEY
  end
  return text
end

--- The float's text: the whole message, then the fixes on offer.
---@param d vim.Diagnostic
---@return string
function M.float(d)
  local fixes = M.fixes(d)
  if #fixes == 0 then
    return d.message
  end
  local lines = { d.message, "" }
  local how = M.FIX_KEY and (" (" .. M.FIX_KEY .. " or :Dbt fix)") or " (:Dbt fix)"
  lines[#lines + 1] = "Fix" .. how .. ":"
  for _, fix in ipairs(fixes) do
    lines[#lines + 1] = "  ✚ " .. fix
  end
  return table.concat(lines, "\n")
end

--- The cogitator client's push and pull diagnostic namespaces.
---@param client vim.lsp.Client
---@return integer[]
local function namespaces(client)
  local out = {}
  for _, pull in ipairs({ false, true }) do
    local ok, ns = pcall(vim.lsp.diagnostic.get_namespace, client.id, pull)
    if ok then
      out[#out + 1] = ns
    end
  end
  return out
end

--- Show the suggestions inline and in the float for this client.
---@param client vim.lsp.Client
---@param opts {keymaps: boolean}
function M.attach(client, opts)
  M.FIX_KEY = opts.keymaps and "<leader>di" or nil
  M.PROBLEMS_KEY = opts.keymaps and "<leader>dI" or nil
  for _, ns in ipairs(namespaces(client)) do
    M._ours[ns] = true
    vim.diagnostic.config({ virtual_text = { format = M.inline, source = false } }, ns)
  end
  M.chain_float()
end

--- The namespaces whose diagnostics this module formats.
---@type table<integer, boolean>
M._ours = {}

--- Neovim resolves the float's options globally, not per namespace: put
--- our format in front of whatever format the user has, once.
function M.chain_float()
  local float = vim.diagnostic.config().float
  if type(float) == "table" and float.format ~= nil and float.format == M._float_format then
    return
  end
  local theirs = type(float) == "table" and float.format or nil
  float = type(float) == "table" and vim.deepcopy(float) or {}
  M._float_format = function(d)
    if M._ours[d.namespace] then
      return M.float(d)
    end
    return theirs and theirs(d) or d.message
  end
  float.format = M._float_format
  vim.diagnostic.config({ float = float })
end

--- The cogitator diagnostics of the buffer, those under the cursor first
--- (by distance of their start from it), then the rest of its line.
---@param bufnr integer
---@param client vim.lsp.Client
---@return vim.Diagnostic[]
function M.at_cursor(bufnr, client)
  local row, col = unpack(vim.api.nvim_win_get_cursor(0))
  row = row - 1
  local mine = {}
  for _, ns in ipairs(namespaces(client)) do
    mine[ns] = true
  end
  local out = {}
  for _, d in ipairs(vim.diagnostic.get(bufnr, { lnum = row })) do
    if mine[d.namespace] and lsp_of(d) then
      out[#out + 1] = d
    end
  end
  local function under(d)
    local stop = d.end_lnum == row and d.end_col or math.huge
    return d.col <= col and col <= stop
  end
  table.sort(out, function(a, b)
    if under(a) ~= under(b) then
      return under(a)
    end
    return math.abs(a.col - col) < math.abs(b.col - col)
  end)
  return out
end

--- The quickfixes that edit this buffer alone (a fix that creates a file
--- is a choice, never applied without asking).
---@param list table[] the server's code actions
---@return table[] in_place, table[] all
function M.split_fixes(list)
  local in_place, all = {}, {}
  for _, a in ipairs(list or {}) do
    if type(a.kind) == "string" and vim.startswith(a.kind, "quickfix") then
      all[#all + 1] = a
      local changes = a.edit and a.edit.documentChanges
      local plain = true
      for _, change in ipairs(changes or {}) do
        if not change.edits then
          plain = false
        end
      end
      if plain and (changes or (a.edit and a.edit.changes)) then
        in_place[#in_place + 1] = a
      end
    end
  end
  return in_place, all
end

--- What `:Dbt fix` does with the server's answer: apply the one fix, the
--- preferred one, or let the user choose.
---@param list table[]
---@return "apply"|"choose"|"none" what
---@return table? action
function M.decide(list)
  local in_place, all = M.split_fixes(list)
  if #all == 0 then
    return "none"
  end
  for _, a in ipairs(in_place) do
    if a.isPreferred then
      return "apply", a
    end
  end
  if #all == 1 and #in_place == 1 then
    return "apply", in_place[1]
  end
  return "choose"
end

--- Fix the problem under the cursor (`:Dbt fix`, <leader>di).
function M.fix()
  local actions = require("cogitator.actions")
  local bufnr = vim.api.nvim_get_current_buf()
  local client = actions.client_for(bufnr)
  if not client then
    return
  end
  local found = M.at_cursor(bufnr, client)
  if #found == 0 then
    vim.notify("cogitator: no problem on this line", vim.log.levels.INFO)
    return
  end
  local target = found[1]
  local lsp = lsp_of(target)
  client:request("textDocument/codeAction", {
    textDocument = { uri = vim.uri_from_bufnr(bufnr) },
    range = lsp.range,
    context = { diagnostics = { lsp }, only = { "quickfix" }, triggerKind = 1 },
  }, function(err, result)
    if err then
      vim.notify("textDocument/codeAction failed: " .. err.message, vim.log.levels.ERROR)
      return
    end
    local what, action = M.decide(result)
    if what == "apply" then
      require("cogitator.code_actions").apply(client, bufnr, action)
      vim.notify("cogitator: " .. action.title, vim.log.levels.INFO)
    elseif what == "choose" then
      vim.api.nvim_win_set_cursor(0, { target.lnum + 1, target.col })
      require("cogitator.code_actions").open({ only = { "quickfix" } })
    else
      vim.notify("cogitator: no fix for this problem — " .. M.headline(target.message), vim.log.levels.WARN)
      vim.diagnostic.open_float({ bufnr = bufnr, scope = "cursor" })
    end
  end, bufnr)
end

--- This buffer's cogitator problems as location-list items, errors first.
---@param bufnr integer
---@param client vim.lsp.Client
---@return table[]
function M.items(bufnr, client)
  local all = {}
  for _, ns in ipairs(namespaces(client)) do
    vim.list_extend(all, vim.diagnostic.get(bufnr, { namespace = ns }))
  end
  table.sort(all, function(a, b)
    if a.severity ~= b.severity then
      return a.severity < b.severity
    end
    return a.lnum < b.lnum or (a.lnum == b.lnum and a.col < b.col)
  end)
  local items = vim.diagnostic.toqflist(all)
  for i, item in ipairs(items) do
    local d = all[i]
    local lsp = lsp_of(d)
    local code = lsp and lsp.code and (" [" .. tostring(lsp.code) .. "]") or ""
    local fixable = #M.fixes(d) > 0 and " ✚" or ""
    item.text = M.headline(d.message) .. code .. fixable
  end
  return items
end

--- This file's problems in the location list (`:Dbt problems`, <leader>dI).
function M.problems()
  local actions = require("cogitator.actions")
  local bufnr = vim.api.nvim_get_current_buf()
  local client = actions.client_for(bufnr)
  if not client then
    return
  end
  local items = M.items(bufnr, client)
  local name = vim.fn.fnamemodify(vim.api.nvim_buf_get_name(bufnr), ":t")
  vim.fn.setloclist(0, {}, " ", { title = "dbt problems · " .. name, items = items })
  if #items == 0 then
    vim.notify("cogitator: no problems in " .. name, vim.log.levels.INFO)
    return
  end
  vim.cmd.lopen()
end

return M
