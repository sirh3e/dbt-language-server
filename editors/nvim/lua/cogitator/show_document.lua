--- window/showDocument from the server. Most of what it shows (a schema
--- entry, a renamed model, a preview) was asked for and opens at once;
--- the compiled SQL a `dbt compile` job leaves arrives when the job ends,
--- maybe long after, so it is asked about first (config.compiled_sql:
--- "ask", the default, "open" or "never").

local M = {}

--- Is `uri` a file dbt compiled (target/compiled or target/run)?
---@param uri string?
---@return boolean
function M.is_compiled(uri)
  if type(uri) ~= "string" then
    return false
  end
  local path = (uri:gsub("\\", "/"))
  return path:find("/target/compiled/", 1, true) ~= nil or path:find("/target/run/", 1, true) ~= nil
end

--- What to do with a showDocument: "open" it as asked, "ask" first, or
--- "skip" it (pure; exposed for the suite).
---@param params {uri: string, external?: boolean}
---@param setting "ask"|"open"|"never"|nil
---@return "open"|"ask"|"skip"
function M.decide(params, setting)
  if params.external or not M.is_compiled(params.uri) then
    return "open"
  end
  if setting == "never" then
    return "skip"
  end
  return setting == "open" and "open" or "ask"
end

--- The choices of the question (exposed for the suite).
---@param uri string
---@return table[]
function M.choices(uri)
  local file = vim.fn.fnamemodify(vim.uri_to_fname(uri), ":~:.")
  local function preview()
    local fname = vim.uri_to_fname(uri)
    local ok, lines = pcall(vim.fn.readfile, fname, "", 200)
    return { lines = ok and lines or { "  " .. file }, syntax = "sql" }
  end
  return {
    {
      label = "open it here",
      icon = "▤",
      icon_hl = "DbtPickerPreferred",
      detail = file,
      value = "edit",
      preview = preview,
    },
    { label = "in a split", icon = "▤", icon_hl = "DbtPickerCommand", value = "split", preview = preview },
    {
      label = "in a vsplit",
      icon = "▤",
      icon_hl = "DbtPickerCommand",
      value = "vsplit",
      preview = preview,
    },
    { label = "in a tab", icon = "▤", icon_hl = "DbtPickerCommand", value = "tabedit", preview = preview },
    { label = "not now", icon = "✕", icon_hl = "DbtPickerDisabled", value = "skip" },
  }
end

--- Open `uri` with `how` (an Ex command), at `selection` when given.
local function open(uri, how, selection)
  vim.cmd[how](vim.fn.fnameescape(vim.uri_to_fname(uri)))
  if selection and selection.start then
    pcall(vim.api.nvim_win_set_cursor, 0, { selection.start.line + 1, selection.start.character })
  end
end

--- The handler: what Neovim's own does, but for the compiled SQL.
---@param err any
---@param params table
---@param ctx table
function M.handler(err, params, ctx)
  local config = package.loaded["cogitator"] and require("cogitator").config or {}
  local decision = M.decide(params or {}, config.compiled_sql)
  if decision == "open" then
    return vim.lsp.handlers["window/showDocument"](err, params, ctx)
  end
  if decision == "skip" then
    return { success = true }
  end
  -- answer at once (the server is not kept waiting on a person), ask after
  vim.schedule(function()
    require("cogitator.picker").open({
      title = "dbt compiled it · open the compiled SQL?",
      items = M.choices(params.uri),
      on_choose = function(item)
        if item.value ~= "skip" then
          open(params.uri, item.value, params.selection)
        end
      end,
    })
  end)
  return { success = true }
end

return M
