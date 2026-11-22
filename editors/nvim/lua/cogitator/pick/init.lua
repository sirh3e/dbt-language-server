--- fzf-style pickers over the project (spec §5.6). A source says what to
--- list and how to preview and use it; a backend shows it: the user's own
--- fzf-lua, snacks.picker or telescope when installed, else the built-in
--- picker. Nothing waits: items and previews arrive asynchronously.

local actions = require("cogitator.pick.actions")
local backends = require("cogitator.pick.backends")

local M = {}

---@class cogitator.pick.Item
---@field text string what the list shows and the query matches
---@field name string? the dbt name (yank / run / navigator)
---@field uri string? the file to open
---@field path string? the file to open (when there is no uri)
---@field lnum integer? 1-based line to land on
---@field col integer? 0-based column
---@field value any? whatever the source needs back

---@class cogitator.pick.Source
---@field title string
---@field items fun(done: fun(items: cogitator.pick.Item[]))
---@field preview? fun(item: cogitator.pick.Item, done: fun(p: {lines: string[], filetype: string?, lnum: integer?}))
---@field choose? fun(items: cogitator.pick.Item[], action: string)
---@field on_cancel? fun()
---@field empty? string what to say when there is nothing to pick

--- Show `source` in the configured picker.
---@param source cogitator.pick.Source
---@param opts {backend?: string}?
function M.open(source, opts)
  local backend = backends.resolve((opts and opts.backend) or M.backend_name())
  if backend.name == "builtin" then
    backend.open(source, function(action, items)
      actions.run(source, action, items)
    end)
    return
  end
  source.items(function(items)
    if #items == 0 then
      vim.notify(source.empty or ("nothing to pick: " .. source.title), vim.log.levels.INFO)
      if source.on_cancel then
        source.on_cancel()
      end
      return
    end
    local ok, err = pcall(backend.open, source, items, function(action, chosen)
      actions.run(source, action, chosen)
    end)
    if not ok then
      vim.notify(
        ("cogitator: the %s picker failed (%s); using the built-in one"):format(backend.name, err),
        vim.log.levels.WARN
      )
      backends.get("builtin").open(source, function(action, chosen)
        actions.run(source, action, chosen)
      end, items)
    end
  end)
end

--- :DbtFind [source]: the named source for the current buffer, or a
--- picker of the sources.
---@param name string?
function M.find(name)
  local sources = require("cogitator.pick.sources")
  local bufnr = vim.api.nvim_get_current_buf()
  local client = require("cogitator.actions").client_for(bufnr)
  if not client then
    return
  end
  local ctx = { client = client, bufnr = bufnr }
  if name and name ~= "" then
    if not vim.tbl_contains(sources.ORDER, name) then
      vim.notify(
        ("unknown picker source %q — one of %s"):format(name, table.concat(sources.ORDER, ", ")),
        vim.log.levels.ERROR
      )
      return
    end
    M.open_named(name, ctx)
    return
  end
  M.select(sources.ORDER, {
    prompt = "find",
    format_item = function(n)
      return ("%-12s %s"):format(n, sources.DESCRIPTIONS[n])
    end,
  }, function(choice)
    if choice then
      M.open_named(choice, ctx)
    end
  end)
end

--- Open a named source; ast-grep's first asks for its pattern.
---@param name string
---@param ctx table
function M.open_named(name, ctx)
  local sources = require("cogitator.pick.sources")
  if name == "astgrep" and not ctx.query then
    require("cogitator.astgrep").prompt(function(query)
      ctx.query = query
      M.open(sources.build(name, ctx))
    end)
    return
  end
  M.open(sources.build(name, ctx))
end

--- The picker setup() asked for ("auto" unless set).
---@return string
function M.backend_name()
  local config = require("cogitator").config
  return config and config.picker or "auto"
end

--- vim.ui.select, through the configured picker: every list the plugin
--- asks the user to choose from gets fuzzy matching and the same keys.
---@param list any[]
---@param opts {prompt?: string, format_item?: fun(item: any): string, preview?: fun(item: any, done: fun(p: table))}
---@param on_choice fun(item: any?, idx: integer?)
function M.select(list, opts, on_choice)
  opts = opts or {}
  if #list == 0 then
    on_choice(nil, nil)
    return
  end
  local format = opts.format_item or tostring
  local answered = false
  M.open({
    title = (opts.prompt or "select"):gsub("[:%s]+$", ""),
    items = function(done)
      local items = {}
      for i, value in ipairs(list) do
        items[i] = { text = format(value), value = value, index = i }
      end
      done(items)
    end,
    preview = opts.preview and function(item, done)
      opts.preview(item.value, done)
    end or nil,
    choose = function(items)
      answered = true
      on_choice(items[1].value, items[1].index)
    end,
    on_cancel = function()
      if not answered then
        on_choice(nil, nil)
      end
    end,
  })
end

return M
