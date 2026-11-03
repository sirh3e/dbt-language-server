--- Explorer keymaps and the navigation they drive. One attach() per
--- pane buffer; everything routes back through the explorer module so
--- state stays in one place.

local actions = require("cogitator.actions")

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

--- <CR> on a side-pane entry: make it the center selection (clearing
--- the filter when it is filtered out), like re-centering in :DbtGraph.
---@param explorer table
---@param pane "left"|"right"
local function reselect(explorer, pane)
  local efilter = require("cogitator.explorer.filter")
  local s = explorer._state
  local e = side_entry(explorer, pane)
  if not e then
    return
  end
  if e.kind ~= "model" then
    vim.notify(e.name .. " is a source — sources are not in the model list", vim.log.levels.INFO)
    return
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
    return
  end
  vim.api.nvim_win_set_cursor(s.wins.center, { row, 0 })
  vim.api.nvim_set_current_win(s.wins.center)
  explorer._select_current()
end

--- Install the pane-local maps (tabs, pane focus, filter, open, close).
---@param explorer table the cogitator.explorer module
---@param pane "left"|"center"|"right"|"preview"
---@param buf integer
function M.attach(explorer, pane, buf)
  local s = explorer._state
  local function map(lhs, fn)
    vim.keymap.set("n", lhs, fn, { buffer = buf, nowait = true })
  end
  map("q", explorer.close)
  for i = 1, 4 do
    map(tostring(i), function()
      explorer._set_tab(i)
    end)
  end
  map("<Tab>", function()
    explorer._set_tab(explorer._state.tab % 4 + 1)
  end)
  if pane == "preview" then
    map("<Esc>", explorer.close)
    map("<CR>", explorer._info_jump)
    map("h", function()
      vim.api.nvim_set_current_win(s.wins.center)
    end)
    return
  end
  map("p", function()
    vim.api.nvim_set_current_win(s.wins.preview)
  end)
  map("h", function()
    vim.api.nvim_set_current_win(s.wins[pane == "right" and "center" or "left"])
  end)
  map("l", function()
    vim.api.nvim_set_current_win(s.wins[pane == "left" and "center" or "right"])
  end)
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
    map("o", function()
      local e = side_entry(explorer, pane)
      if e then
        open_uri(explorer, e.uri)
      end
    end)
  end
end

return M
