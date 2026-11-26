--- Optional completion-plugin helpers. The server ranks with sortText:
--- the columns of the relations in scope first (`alias.col` before a bare
--- name two relations share), then CTEs, buffer words, relations,
--- functions, keywords. Neovim's built-in completion sorts by sortText
--- as it is; nvim-cmp and blink.cmp put their fuzzy score first, so a
--- global that fuzzy-matches the typed word as well as a column can come
--- out above it. Nothing here requires either plugin.
---
--- nvim-cmp: order cogitator's items by sortText, show the detail:
---
---   local dbt = require("cogitator.completion")
---   cmp.setup({
---     sorting = { comparators = dbt.sorted(cmp.config.compare) },
---     formatting = { format = dbt.format() },
---   })
---
--- blink.cmp (LazyVim's default; its fuzzy.sorts default is
--- { "score", "sort_text" }): lift the in-scope columns' score so typing
--- keeps them on top, wrapping a transform you already have:
---
---   sources = { providers = { lsp = {
---     transform_items = require("cogitator.completion").blink_transform(),
---   } } }

local M = {}

local DETAIL_WIDTH = 40

--- Is this cmp entry one of cogitator's?
---@param entry table a cmp entry
---@return boolean
function M.is_dbt(entry)
  local source = entry and entry.source
  local client = source and source.source and source.source.client
  return type(client) == "table" and client.name == "cogitator"
end

local function sort_text(entry)
  local item = entry.completion_item or {}
  return item.sortText or item.label
end

--- A cmp comparator: between two cogitator items, the server's sortText
--- decides; anything else is left to the next comparator (nil).
---@param a table
---@param b table
---@return boolean?
function M.compare(a, b)
  if not (M.is_dbt(a) and M.is_dbt(b)) then
    return nil
  end
  local ta, tb = sort_text(a), sort_text(b)
  if ta == tb then
    return nil
  end
  return ta < tb
end

--- The comparators cogitator suggests on their own.
---@return function[]
function M.comparators()
  return { M.compare }
end

--- `comparators` (cmp's list, or cmp.config.compare as a table of named
--- comparators) with cogitator's first.
---@param comparators table
---@return function[]
function M.sorted(comparators)
  local list = { M.compare }
  if vim.islist(comparators) then
    vim.list_extend(list, comparators)
  else
    for _, name in ipairs({ "offset", "exact", "score", "recently_used", "kind", "length", "order" }) do
      if comparators[name] then
        list[#list + 1] = comparators[name]
      end
    end
  end
  return list
end

--- What `blink_transform` adds to the score of a column in scope: enough
--- to beat a global's fuzzy bonus (prefix, exact case) for the same
--- typed word, not enough to lift a column that matches it badly.
M.BLINK_BOOST = 8

--- A blink.cmp `transform_items` for the lsp provider: cogitator's items
--- of sortText tier `0` (the columns in scope, the CTEs after FROM) get
--- `M.BLINK_BOOST` added to their score_offset; everything else passes
--- through `inner` (another transform), or unchanged.
---@param inner? fun(ctx: table, items: table[]): table[]
---@return fun(ctx: table, items: table[]): table[]
function M.blink_transform(inner)
  return function(ctx, items)
    if inner then
      items = inner(ctx, items)
    end
    for _, item in ipairs(items) do
      if item.client_name == "cogitator" and (item.sortText or ""):sub(1, 1) == "0" then
        item.score_offset = (item.score_offset or 0) + M.BLINK_BOOST
      end
    end
    return items
  end
end

--- The menu text of an item: its labelDetails (a column's type and
--- relation, a model's materialization) when the server sent them, else
--- its detail.
---@param item table an LSP CompletionItem
---@return string?
local function menu_text(item)
  local details = item.labelDetails
  if type(details) == "table" then
    local parts = {}
    for _, part in ipairs({ details.detail, details.description }) do
      if type(part) == "string" and vim.trim(part) ~= "" then
        parts[#parts + 1] = vim.trim(part)
      end
    end
    if #parts > 0 then
      return table.concat(parts, " · ")
    end
  end
  return item.detail
end

--- A cmp `formatting.format` that puts a cogitator item's detail in the
--- menu column, wrapping `formatter` (cmp's own or lspkind's) if given.
---@param formatter? fun(entry: table, vim_item: table): table
---@return fun(entry: table, vim_item: table): table
function M.format(formatter)
  return function(entry, vim_item)
    if formatter then
      vim_item = formatter(entry, vim_item)
    end
    local detail = M.is_dbt(entry) and menu_text(entry.completion_item or {})
    if type(detail) == "string" and detail ~= "" then
      detail = detail:gsub("\n.*", "")
      if vim.fn.strchars(detail) > DETAIL_WIDTH then
        detail = vim.fn.strcharpart(detail, 0, DETAIL_WIDTH - 1) .. "…"
      end
      vim_item.menu = detail
    end
    return vim_item
  end
end

return M
