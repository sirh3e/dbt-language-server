--- Optional nvim-cmp helpers: order cogitator's items by the server's
--- sortText (refs before sources before macros, closest names first),
--- and show an item's detail (model path, column type, …) in the menu.
--- Nothing here requires cmp; pass the functions to your cmp setup:
---
---   local dbt = require("cogitator.completion")
---   cmp.setup({
---     sorting = { comparators = dbt.sorted(cmp.config.compare) },
---     formatting = { format = dbt.format() },
---   })

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
