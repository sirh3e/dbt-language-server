--- Explorer keymaps and the navigation they drive. One attach() per
--- pane buffer; everything routes back through the explorer module so
--- state stays in one place.

local actions = require("cogitator.actions")
local motion = require("cogitator.motion")

local M = {}

--- Close the explorer and open a file.
---@param explorer table
---@param uri string|vim.NIL|nil
local function open_uri(explorer, uri)
  if not uri or uri == vim.NIL then
    vim.notify("no file on disk for this entry", vim.log.levels.WARN)
    return
  end
  explorer.close()
  actions.open_uri(uri)
end

--- The side-pane entry under the cursor.
---@param explorer table
---@param pane "left"|"right"
---@return cogitator.GraphEntry?
local function side_entry(explorer, pane)
  local s = explorer._state
  local entries = s.info and (pane == "left" and s.info.upstream or s.info.downstream)
  return entries and entries[vim.api.nvim_win_get_cursor(s.wins[pane])[1]]
end

--- Make a side-pane entry the center selection (clearing the filter when
--- it is filtered out), like re-centering in :DbtGraph.
---@param explorer table
---@param pane "left"|"right"
---@param stay boolean? keep focus in the side pane
---@return boolean moved
local function reselect(explorer, pane, stay)
  local efilter = require("cogitator.explorer.filter")
  local s = explorer._state
  local e = side_entry(explorer, pane)
  if not e then
    return false
  end
  if e.kind ~= "model" then
    vim.notify(
      ("%s is a %s — only models are in the center list"):format(e.name, e.kind:gsub("_", " ")),
      vim.log.levels.INFO
    )
    return false
  end
  local function row_of(name)
    for i, n in ipairs(s.filtered) do
      if n.name == name then
        return i
      end
    end
  end
  local row = row_of(e.name)
  if not row then
    s.filter = ""
    efilter.apply(explorer)
    row = row_of(e.name)
  end
  if not row then
    vim.notify(e.name .. " is not in the project graph", vim.log.levels.WARN)
    return false
  end
  vim.api.nvim_win_set_cursor(s.wins.center, { row, 0 })
  if not stay then
    vim.api.nvim_set_current_win(s.wins.center)
  end
  explorer._select_current()
  return true
end

--- Re-center on the side-pane entry `count` times, staying in the pane so
--- repeated presses walk the lineage. Each step needs the previous
--- selection's neighbours, so steps wait for dbt.modelInfo to land.
---@param explorer table
---@param pane "left"|"right"
---@param count integer
local function walk(explorer, pane, count)
  local s = explorer._state
  if not s.info then
    local pending = s.pending_walk
    local queued = (pending and pending.pane == pane) and pending.count or 0
    s.pending_walk = { pane = pane, count = queued + count }
    return
  end
  if reselect(explorer, pane, true) and count > 1 then
    s.pending_walk = { pane = pane, count = count - 1 }
  end
end

--- Continue a walk once the selection's neighbours have arrived.
---@param explorer table
function M.resume_walk(explorer)
  local s = explorer._state
  local pending = s.pending_walk
  if pending then
    s.pending_walk = nil
    walk(explorer, pending.pane, pending.count)
  end
end

--- <Tab> cycles the preview; [count]<Tab> jumps to tab [count].
---@param explorer table
local function next_tab(explorer)
  local count = vim.v.count
  if count == 0 then
    explorer._set_tab(explorer._state.tab % 4 + 1)
  elseif count <= 4 then
    explorer._set_tab(count)
  end
end

--- Step the center selection, wrapping at both ends.
---@param explorer table
---@param delta integer
local function step_center(explorer, delta)
  local s = explorer._state
  local total = #s.filtered
  if total == 0 then
    return
  end
  local row = vim.api.nvim_win_get_cursor(s.wins.center)[1]
  vim.api.nvim_win_set_cursor(s.wins.center, { (row - 1 + delta) % total + 1, 0 })
  explorer._select_current()
end

local function yank(name)
  vim.fn.setreg(vim.v.register, name)
  vim.notify("yanked " .. name, vim.log.levels.INFO)
end

--- Install the pane-local maps (tabs, pane focus, filter, open, close).
---@param explorer table the cogitator.explorer module
---@param pane "left"|"center"|"right"|"preview"
---@param buf integer
function M.attach(explorer, pane, buf)
  local s = explorer._state
  local function map(lhs, fn)
    motion.map(buf, lhs, fn)
  end
  local function focus(name)
    return function()
      vim.api.nvim_set_current_win(s.wins[name])
    end
  end
  local function walker(side)
    return function()
      walk(explorer, side, vim.v.count1)
    end
  end

  map("q", explorer.close)
  map("<Tab>", function()
    next_tab(explorer)
  end)
  map("<S-Tab>", function()
    explorer._set_tab((explorer._state.tab - 2) % 4 + 1)
  end)
  map("J", function()
    motion.scroll(s.wins.preview, 5 * vim.v.count1)
  end)
  map("K", function()
    motion.scroll(s.wins.preview, -5 * vim.v.count1)
  end)

  if pane == "preview" then
    map("<Esc>", explorer.close)
    map("<CR>", explorer._info_jump)
    map("h", focus("center"))
    return
  end

  map("p", focus("preview"))
  if pane == "left" then
    map("h", walker("left"))
    map("l", focus("center"))
  elseif pane == "right" then
    map("h", focus("center"))
    map("l", walker("right"))
  else
    map("h", focus("left"))
    map("l", focus("right"))
  end

  if pane == "center" then
    map("<Esc>", function()
      if explorer._state.filter ~= "" then
        explorer._apply_filter("")
      else
        explorer.close()
      end
    end)
    map("/", function()
      require("cogitator.explorer.filter").prompt(explorer)
    end)
    map(
      "n",
      motion.repeated(function()
        step_center(explorer, 1)
      end)
    )
    map(
      "N",
      motion.repeated(function()
        step_center(explorer, -1)
      end)
    )
    map("y", function()
      local node = explorer._selected_node()
      if node then
        yank(node.name)
      end
    end)
    local open_center = function()
      local node = explorer._selected_node()
      if node then
        open_uri(explorer, node.uri)
      end
    end
    map("<CR>", open_center)
    map("o", open_center)
  else
    map("<Esc>", explorer.close)
    map("<CR>", function()
      reselect(explorer, pane)
    end)
    map("y", function()
      local e = side_entry(explorer, pane)
      if e then
        yank(e.name)
      end
    end)
    map("o", function()
      local e = side_entry(explorer, pane)
      if e then
        open_uri(explorer, e.uri)
      end
    end)
  end
end

return M
