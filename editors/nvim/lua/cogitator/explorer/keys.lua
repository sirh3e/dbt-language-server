--- Explorer keymaps and the navigation they drive. One attach() per pane
--- buffer; everything routes back through the explorer module so the
--- state stays in one place.

local actions = require("cogitator.actions")
local motion = require("cogitator.motion")

local M = {}

--- Close the explorer and open a file, in place or in a split/vsplit/tab.
---@param explorer table
---@param uri string|vim.NIL|nil
---@param how ("edit"|"split"|"vsplit"|"tabedit")?
local function open_uri(explorer, uri, how)
  if not uri or uri == vim.NIL then
    vim.notify("no file on disk for this entry", vim.log.levels.WARN)
    return
  end
  explorer.close()
  if how and how ~= "edit" then
    vim.cmd[how](vim.fn.fnameescape(vim.uri_to_fname(uri)))
  else
    actions.open_uri(uri)
  end
end

--- The side-pane entry under the cursor.
---@param explorer table
---@param pane "left"|"right"
---@return cogitator.GraphEntry?
local function side_entry(explorer, pane)
  local s = explorer._state
  if not s.info then
    return nil
  end
  local entries = require("cogitator.explorer.render").side_entries(s, pane)
  return entries[vim.api.nvim_win_get_cursor(s.wins[pane])[1]]
end

--- Make a side-pane entry the selection (recording the history).
---@param explorer table
---@param pane "left"|"right"
---@param stay boolean? keep focus in the side pane
---@return boolean moved
local function reselect(explorer, pane, stay)
  local e = side_entry(explorer, pane)
  if not e then
    return false
  end
  return explorer.focus(e.name, { record = true, stay = stay, node = e })
end

--- Re-centre on the side-pane entry `count` times, staying in the pane so
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

--- Scroll both SQL panes together (they show the same model).
---@param explorer table
---@param lines integer
local function scroll_sql(explorer, lines)
  local s = explorer._state
  for _, pane in ipairs({ "raw", "compiled" }) do
    if s.wins[pane] and vim.api.nvim_win_is_valid(s.wins[pane]) then
      motion.scroll(s.wins[pane], lines)
    end
  end
end

--- The actions on an entry (the selection, or a side pane's line): open
--- it elsewhere, compile it, show it in the DAG or its column lineage;
--- and parse the project.
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
  map(
    "o",
    with(function(e)
      open_uri(explorer, e.uri)
    end)
  )
  for lhs, how in pairs({ s = "split", v = "vsplit", t = "tabedit" }) do
    map(
      lhs,
      with(function(e)
        open_uri(explorer, e.uri, how)
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
  -- the marked models, or this entry: yank their names, dbt-run them
  map(
    "y",
    with(function(e)
      local names = explorer._chosen(e.name)
      vim.fn.setreg(vim.v.register, table.concat(names, " "))
      vim.notify("yanked " .. table.concat(names, " "), vim.log.levels.INFO)
    end)
  )
  map(
    "r",
    with(function(e)
      local s = explorer._state
      local select = require("cogitator.explorer.select")
      if select.marks_are_the_selection(s) then
        -- the marks are what a selection matched: run the selection
        select.run(s, s.selection.select, s.selection.exclude)
      else
        require("cogitator.commands").run_selection({
          args = table.concat(explorer._chosen(e.name), " "),
          bufnr = s.src_buf,
        })
      end
      s.selection = nil
      s.marked = {}
      require("cogitator.explorer.render").sides(s)
      require("cogitator.explorer.render").center(s)
    end)
  )
  map(
    "<Space>",
    with(function(e)
      explorer._toggle_mark(e.name)
    end)
  )
end

M.HELP = {
  "explorer — one model, its lineage and its SQL",
  "",
  "  upstream ⯇  |  the model  |  ⯈ downstream",
  "  dbt SQL                |       compiled SQL",
  "",
  "  h / l          from the model: into upstream / downstream",
  "  h h… / l l…    in a side pane: walk the lineage ([count] too)",
  "  <CR>           side pane: make it the model; card: open the test,",
  "                 version or the model's file",
  "  [ - / ]        back / forward through the models walked",
  "  .              macros, tests, sources… too (a macro's card: its",
  "                 callers and calls; <CR> in its compiled pane re-centres)",
  "  f  /           pick any model (fuzzy, fzf syntax), or type a dbt",
  "                 selection: tag:x  +m  @m  a,b  stg_*  … --exclude x",
  "                 (<Tab> completes, <C-a> marks every match,",
  "                 <C-r> runs it with dbt; r then runs the selection)",
  "  p              to the dbt SQL; h / l between the SQL panes",
  "  e / E          open the dbt SQL / the compiled SQL file",
  "                 (<CR> or o in a SQL pane: that file, at that line)",
  "  J / K          scroll both SQL panes      <Esc>  back to the model",
  "  <Space>        mark                       y / r  yank / dbt run the marked",
  "  o s v t        open / split / vsplit / tab",
  "  c              compile   g  :DbtDag   L  column lineage   P  dbt parse",
  "  ?              these keys                 q      close",
}

--- `?`: the keys, in a float that any of q, <Esc>, ? or <CR> closes.
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

--- Install the pane-local maps.
---@param explorer table the cogitator.explorer module
---@param pane "left"|"center"|"right"|"raw"|"compiled"
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

  -- everywhere
  map("q", explorer.close)
  map("?", M.help)
  map("J", function()
    scroll_sql(explorer, 5 * vim.v.count1)
  end)
  map("K", function()
    scroll_sql(explorer, -5 * vim.v.count1)
  end)
  map("f", function()
    explorer._find()
  end)
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
  -- the model's two files, from any pane
  map("e", function()
    explorer._open_sql("raw")
  end)
  map("E", function()
    explorer._open_sql("compiled")
  end)

  if pane == "raw" or pane == "compiled" then
    map("<Esc>", focus("center"))
    if pane == "compiled" then
      -- a macro's caller: re-centre on it
      map("<CR>", function()
        local line = vim.api.nvim_win_get_cursor(s.wins.compiled)[1]
        local caller = s.compiled_jumps and s.compiled_jumps[line]
        if caller then
          explorer.focus(caller.name, { record = true, node = caller })
        else
          explorer._open_sql("compiled", line)
        end
      end)
      map("o", function()
        explorer._open_sql("compiled", vim.api.nvim_win_get_cursor(s.wins.compiled)[1])
      end)
    else
      -- the dbt SQL at the line the cursor is on
      local function open_here()
        explorer._open_sql("raw", vim.api.nvim_win_get_cursor(s.wins.raw)[1])
      end
      map("<CR>", open_here)
      map("o", open_here)
    end
    if pane == "raw" then
      map("l", focus("compiled"))
    else
      map("h", focus("raw"))
    end
    return
  end

  map("p", focus("raw"))
  if pane == "center" then
    map("h", focus("left"))
    map("l", focus("right"))
    map("/", function()
      explorer._find()
    end)
    map("<Esc>", explorer.close)
    map("<CR>", explorer._card_enter)
    entry_actions(explorer, explorer._selected_node, map)
  else
    map("<Esc>", focus("center"))
    if pane == "left" then
      map("h", walker("left"))
      map("l", focus("center"))
    else
      map("h", focus("center"))
      map("l", walker("right"))
    end
    map("<CR>", function()
      reselect(explorer, pane)
    end)
    entry_actions(explorer, function()
      return side_entry(explorer, pane)
    end, map)
  end
end

return M
