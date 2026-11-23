--- dbt selections in the navigator's finder (`f`, `/`): typing dbt's
--- syntax lists what `dbt ls --select` would (cogitator.pick.select_source).
--- Choosing a node centres it (every kind of node then shows) or, for one
--- the project graph has no node of (a properties file's test, a seed
--- nothing reads), opens it. <C-a> marks every match, <C-r> runs the
--- selection with dbt, and while the marks are what <C-a> marked, `r`
--- runs the expression rather than the names — what you saw is what runs.

local M = {}

--- The navigator's key a match is marked by: its graph node's name, else
--- what selects it alone.
---@param item cogitator.pick.Item
---@return string
function M.mark_key(item)
  local v = item.value or {}
  -- a source's tables share their source's graph node
  if v.in_graph ~= false and v.kind ~= "source" then
    return item.name
  end
  return item.selector or item.name
end

--- <C-a>: mark every match and remember the selection they came from.
---@param explorer table
---@param items cogitator.pick.Item[]
---@param query string
function M.mark_all(explorer, items, query)
  local s = explorer._state
  if not s or #items == 0 then
    return
  end
  local selection = require("cogitator.pick.selection")
  local select, exclude = selection.split(query)
  s.marked = {}
  local keys = {}
  for _, item in ipairs(items) do
    local key = M.mark_key(item)
    s.marked[key] = true
    keys[key] = true
  end
  -- fuzzy words mark names; only a selection is run as itself
  s.selection = selection.is_selection(query)
      and { select = select, exclude = exclude, keys = keys, count = #items }
    or nil
  for _, item in ipairs(items) do
    if item.value and item.value.in_graph ~= false and s.node_of[item.name] then
      M.choose(explorer, item)
      return
    end
  end
  require("cogitator.explorer.render").sides(s)
  require("cogitator.explorer.render").center(s)
end

--- Are the marks exactly what the last <C-a> marked?
---@param s cogitator.explorer.State
---@return boolean
function M.marks_are_the_selection(s)
  local sel = s.selection
  if not sel or vim.tbl_count(s.marked) ~= vim.tbl_count(sel.keys) then
    return false
  end
  for key in pairs(s.marked) do
    if not sel.keys[key] then
      return false
    end
  end
  return true
end

--- <C-r> in the finder, or `r` on the marks of a selection: dbt build,
--- run or test with the selection itself.
---@param s cogitator.explorer.State
---@param select string
---@param exclude string?
function M.run(s, select, exclude)
  require("cogitator.commands").run_selection({
    args = select,
    exclude = exclude,
    ask_kind = true,
    bufnr = s.src_buf,
  })
end

--- A chosen match: centre it, or open its file.
---@param explorer table
---@param item cogitator.pick.Item
function M.choose(explorer, item)
  local s = explorer._state
  local v = item.value or {}
  -- a fuzzy pick is a graph node; a selection's match may be none
  if v.in_graph ~= false and s.node_of[item.name] then
    if s.node_of[item.name].kind ~= "model" and not s.show_all then
      explorer._toggle_all()
    end
    explorer.focus(item.name, { record = true })
    return
  end
  explorer.close()
  if require("cogitator.actions").open_uri(item.uri) and item.lnum then
    pcall(vim.api.nvim_win_set_cursor, 0, { item.lnum, item.col or 0 })
  end
end

--- The finder's own keys while a selection is typed.
---@param explorer table
---@return {lhs: string, desc: string, fn: fun(items: table[], query: string)}[]
function M.keys(explorer)
  return {
    {
      lhs = "<C-a>",
      desc = "mark every match",
      fn = function(items, query)
        M.mark_all(explorer, items, query)
      end,
    },
    {
      lhs = "<C-r>",
      desc = "run the selection (or the matches) with dbt",
      fn = function(items, query)
        local s = explorer._state
        if not s then
          return
        end
        local selection = require("cogitator.pick.selection")
        if selection.is_selection(query) then
          local select, exclude = selection.split(query)
          M.run(s, select, exclude)
          return
        end
        -- fuzzy words: the matches by name, as the pickers' <C-r> does
        local names = {}
        for _, item in ipairs(items) do
          names[#names + 1] = M.mark_key(item)
        end
        if #names > 0 then
          M.run(s, table.concat(names, " "), nil)
        end
      end,
    },
  }
end

--- The centre title's word on the selection the marks came from.
---@param s cogitator.explorer.State
---@return string?
function M.title(s)
  if not (s.selection and M.marks_are_the_selection(s)) then
    return nil
  end
  local sel = s.selection
  return ("dbt select %s%s (%d)"):format(
    sel.select,
    sel.exclude and (" --exclude " .. sel.exclude) or "",
    sel.count
  )
end

return M
