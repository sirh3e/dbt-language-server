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
  local s = explorer._state
  local e = side_entry(explorer, pane)
  if not e then
    return false
  end
  if e.kind ~= "model" and not s.show_all then
    vim.notify(
      ("%s is a %s — only models are in the center list (. lists every kind)"):format(
        e.name,
        e.kind:gsub("_", " ")
      ),
      vim.log.levels.INFO
    )
    return false
  end
  return explorer.focus(e.name, { record = true, stay = stay })
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
    explorer._set_tab(explorer._state.tab % 5 + 1)
  elseif count <= 5 then
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

--- Close the explorer and open a file in a split, a vsplit or a tab.
---@param explorer table
---@param uri string|vim.NIL|nil
---@param how "split"|"vsplit"|"tabedit"
local function open_in(explorer, uri, how)
  if not uri or uri == vim.NIL then
    vim.notify("no file on disk for this entry", vim.log.levels.WARN)
    return
  end
  explorer.close()
  vim.cmd[how](vim.fn.fnameescape(vim.uri_to_fname(uri)))
end

--- The explorer's actions on an entry (the center selection or a side
--- pane's line): open it elsewhere, compile it, show it in the DAG or
--- its column lineage; and parse the project.
---@param explorer table
---@param entry_of fun(): cogitator.GraphEntry?
---@param map fun(lhs: string, fn: fun())
local function entry_actions(explorer, entry_of, map)
  local function with(fn)
    return function()
      local e = entry_of()
      if e then
        fn(e)
      end
    end
  end
  for lhs, how in pairs({ s = "split", v = "vsplit", t = "tabedit" }) do
    map(
      lhs,
      with(function(e)
        open_in(explorer, e.uri, how)
      end)
    )
  end
  map(
    "c",
    with(function(e)
      if e.kind ~= "model" or not e.uri or e.uri == vim.NIL then
        vim.notify("only a model with a file compiles", vim.log.levels.WARN)
        return
      end
      local s = explorer._state
      local client, src_buf = s.client, s.src_buf
      -- the compiled SQL arrives through showDocument: give it a window
      explorer.close()
      actions.exec_with(client, src_buf, "dbt.compile", { uri = e.uri })
    end)
  )
  map("P", function()
    local s = explorer._state
    actions.exec_with(s.client, s.src_buf, "dbt.parse", { uri = "" })
  end)
  map(
    "g",
    with(function(e)
      explorer.close()
      require("cogitator.dag").open({ select = e.kind .. ":" .. e.name })
    end)
  )
  map(
    "L",
    with(function(e)
      if e.kind ~= "model" then
        vim.notify("column lineage follows models; this is a " .. e.kind, vim.log.levels.INFO)
        return
      end
      explorer.close()
      require("cogitator.dag").open({ columns = { model = e.name } })
    end)
  )
end

local function yank(name)
  vim.fn.setreg(vim.v.register, name)
  vim.notify("yanked " .. name, vim.log.levels.INFO)
end

M.HELP = {
  "navigator — the project's lineage, yazi-style",
  "",
  "  j k / n N      move (n N wrap)          h / l    upstream / downstream pane",
  "  h h… / l l…    walk the lineage          [ - / ]  back / forward",
  "  /              fuzzy filter (fzf syntax) <Esc>    clear the filter / close",
  "  f              find a model (picker)     .        every kind of node",
  "  <Space>        mark                      r        dbt run the marked",
  "  y              yank names                <CR> o   open",
  "  s v t          split / vsplit / tab      c        compile",
  "  g              show in :DbtDag           L        column lineage",
  "  <Tab> 1-5<Tab> preview tab               J K      scroll the preview",
  "  P              dbt parse                 q        close",
  "",
  "preview tabs: 1 raw · 2 compiled · 3 versions/tests · 4 info · 5 columns",
}

--- `?`: the keys, in a float that any key closes.
function M.help()
  local ui = require("cogitator.ui")
  local width = 0
  for _, line in ipairs(M.HELP) do
    width = math.max(width, vim.fn.strdisplaywidth(line))
  end
  local buf, win = ui.float({
    row = math.max(0, math.floor((vim.o.lines - #M.HELP) / 2) - 1),
    col = math.max(0, math.floor((vim.o.columns - width) / 2)),
    width = width,
    height = #M.HELP,
    title = "keys",
  })
  ui.set_lines(buf, M.HELP)
  vim.api.nvim_set_current_win(win)
  for _, lhs in ipairs({ "q", "<Esc>", "?", "<CR>" }) do
    vim.keymap.set("n", lhs, function()
      if vim.api.nvim_win_is_valid(win) then
        vim.api.nvim_win_close(win, true)
      end
    end, { buffer = buf, nowait = true })
  end
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
    explorer._set_tab((explorer._state.tab - 2) % 5 + 1)
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
      local names = vim.tbl_map(function(n)
        return n.name
      end, explorer._chosen())
      if #names > 0 then
        yank(table.concat(names, " "))
      end
    end)
    -- yazi: <Space> marks, r runs the marked (or the hovered) with dbt,
    -- f finds, [ ] - step through the walk's history, . lists every kind
    map("<Space>", explorer._toggle_mark)
    map("r", function()
      local names = vim.tbl_map(function(n)
        return n.name
      end, explorer._chosen())
      if #names > 0 then
        local st = explorer._state
        require("cogitator.commands").run_selection({ args = table.concat(names, " ") })
        st.marked = {}
        require("cogitator.explorer.render").center(st, true)
      end
    end)
    map("f", explorer._find)
    map("[", function()
      explorer._history(-1)
    end)
    map("-", function()
      explorer._history(-1)
    end)
    map("]", function()
      explorer._history(1)
    end)
    map(".", explorer._toggle_all)
    map("?", M.help)
    local open_center = function()
      local node = explorer._selected_node()
      if node then
        open_uri(explorer, node.uri)
      end
    end
    map("<CR>", open_center)
    map("o", open_center)
    entry_actions(explorer, explorer._selected_node, map)
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
    entry_actions(explorer, function()
      return side_entry(explorer, pane)
    end, map)
  end
end

return M
