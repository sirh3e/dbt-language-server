--- :DbtDag — the whole project as a dbt-docs-style DAG (Neovim only).
--- Layers flow left→right (sources/roots first); each node is a box in
--- its dbt-docs colour with its tags and, when expanded, its documented
--- columns (docglow-style).
--- Keys: h/l parent/child along edges, j/k within the layer, gg/G first/
--- last node, {/} first/last layer, / search with n/N, <C-o>/<C-i>
--- history, <CR> open model, c/C toggle columns, L column lineage graph,
--- m back to the models, f focus on the selection, +/- focus depth,
--- F follow the current buffer, R refresh, q close. The pure engines are
--- cogitator.layout (placement), cogitator.dag_nav (movement) and
--- cogitator.dag_data (relayering, focus); cogitator.dag_modes switches
--- between the model graph and a column-lineage graph.

local actions = require("cogitator.actions")
local filter = require("cogitator.explorer.filter")
local layout_lib = require("cogitator.layout")
local nav = require("cogitator.dag_nav")
local paint = require("cogitator.dag_paint")

local M = {}

---@class cogitator.dag.State
---@field project table the dbt.projectGraph payload
---@field data table the payload on screen (the project, a focus, or columns)
---@field mode "models"|"columns"
---@field focused boolean models mode shows only the selection's neighbourhood
---@field depth integer hops shown each way when focused
---@field follow boolean select the model of the buffer entered elsewhere
---@field start {model: string, column?: string}? the column graph's origin
---@field expanded table<string, boolean> per-node column toggle
---@field all_expanded boolean
---@field boxes table<string, cogitator.layout.Box>
---@field index cogitator.layout.Index
---@field adjacency table parents and children of every node
---@field decor cogitator.canvas.Decor
---@field edges_of table<string, integer[]> node id → touching edge indexes
---@field selected string? the selected node's id
---@field memory cogitator.dag_nav.Memory
---@field history cogitator.dag_nav.History
---@field matches string[]? the current search's matches
---@field note string? a word for the winbar (truncated walks, …)
---@field buf integer the cogitator://dag scratch buffer
---@field win integer
---@field src_buf integer
---@field client vim.lsp.Client
---@type cogitator.dag.State?
M._state = nil

local GROUP = "CogitatorDag"

--- The plugin's dag/lineage settings (defaults before setup()).
---@return {dag: table, lineage: table}
function M.settings()
  local config = require("cogitator").config or require("cogitator.config").build({})
  return { dag = config.dag, lineage = config.lineage }
end

--- The box under the cursor (char-cell hit test).
---@return cogitator.layout.Box?
function M.box_at_cursor()
  local s = M._state
  local row, byte_col = unpack(vim.api.nvim_win_get_cursor(s.win))
  local line = vim.api.nvim_buf_get_lines(s.buf, row - 1, row, false)[1] or ""
  local col = vim.fn.charidx(line, math.min(byte_col, #line)) + 1
  return layout_lib.box_at(s.index, row, col)
end

---@return cogitator.layout.Node?
function M.node_at_cursor()
  local box = M.box_at_cursor()
  return box and box.node
end

--- Put the cursor on a box's title.
---@param box cogitator.layout.Box
local function cursor_to(box)
  local s = M._state
  local row = box.y + 1
  local line = vim.api.nvim_buf_get_lines(s.buf, row - 1, row, false)[1] or ""
  vim.api.nvim_win_set_cursor(s.win, { row, vim.fn.byteidx(line, box.x + 1) })
end

--- What the view shows, in the window's winbar.
local function winbar()
  local s = M._state
  if not (s and vim.api.nvim_win_is_valid(s.win)) then
    return
  end
  local parts = { "DbtDag" }
  if s.mode == "columns" and s.start then
    parts[#parts + 1] = "columns of " .. s.start.model .. (s.start.column and ("." .. s.start.column) or "")
  elseif s.focused then
    parts[#parts + 1] = ("focus ±%d"):format(s.depth)
  else
    parts[#parts + 1] = "models"
  end
  if s.follow then
    parts[#parts + 1] = "follow"
  end
  if s.note then
    parts[#parts + 1] = s.note
  end
  vim.wo[s.win].winbar = " " .. table.concat(parts, " · ")
end

--- Make `id` the selection: paint it, move the cursor, record the visit.
---@param id string?
---@param opts {record?: boolean, move?: boolean}?
function M.select(id, opts)
  local s = M._state
  if not (id and s.boxes[id]) then
    return
  end
  opts = opts or {}
  s.selected = id
  paint.selection(s.buf, id, s.decor, s.edges_of[id] or {})
  if opts.move ~= false then
    cursor_to(s.boxes[id])
  end
  if opts.record ~= false then
    nav.visit(s.history, id)
  end
end

--- Re-run layout + draw into the scratch buffer, keeping the selection.
local function render()
  local s = M._state
  local boxes, width, height = layout_lib.layout(s.data, s.expanded, s.all_expanded)
  s.boxes = boxes
  s.index = layout_lib.index(boxes)
  local lines, decor = layout_lib.draw(boxes, s.data.edges, width + 2, height + 1)
  s.decor = decor
  require("cogitator.ui").set_lines(s.buf, lines)
  paint.static(s.buf, boxes, decor)
  winbar()
  M.select(s.selected, { record = false })
end

--- Put a payload on screen and select `selected` (else keep the old
--- selection when it is still there, else the first node).
---@param data table {nodes, edges}
---@param selected string?
function M.show(data, selected)
  local s = M._state
  local edges_of = {}
  for i, edge in ipairs(data.edges) do
    for _, id in ipairs({ edge.from, edge.to }) do
      edges_of[id] = edges_of[id] or {}
      table.insert(edges_of[id], i)
    end
  end
  s.data = data
  s.adjacency = layout_lib.adjacency(data.edges)
  s.edges_of = edges_of
  s.memory = nav.new_memory()
  s.matches = nil
  local keep = s.selected
  s.selected = nil
  render()
  local pick = selected
  for _, id in ipairs({ selected, keep }) do
    if not pick and s.boxes[id] then
      pick = id
    end
  end
  M.select(pick and s.boxes[pick] and pick or nav.nth(M.graph(), 1))
end

M.render = render
M.winbar = winbar

---@return cogitator.dag_nav.Graph
function M.graph()
  local s = M._state
  return { boxes = s.boxes, index = s.index, adjacency = s.adjacency }
end

--- <CR>: close the DAG tab and edit the node's file.
local function open_node()
  local node = M.node_at_cursor()
  if not node then
    return
  end
  if not node.uri or node.uri == vim.NIL then
    vim.notify("no file for " .. node.name, vim.log.levels.WARN)
    return
  end
  M.close()
  vim.cmd.edit(vim.uri_to_fname(node.uri))
end

--- c: toggle the columns of the node under the cursor.
local function toggle_node()
  local node = M.node_at_cursor()
  if node then
    M._state.expanded[node.id] = not M._state.expanded[node.id]
    render()
  end
end

--- Selection follows native motions and the mouse.
local function sync_selection()
  local s = M._state
  if not s then
    return
  end
  local box = M.box_at_cursor()
  if box and box.node.id ~= s.selected then
    M.select(box.node.id, { move = false })
  end
end

--- /: live node-name search; <Esc> returns to where it started.
local function search()
  local s = M._state
  local origin = s.selected
  local function echo(text)
    vim.api.nvim_echo({ { "/" .. text } }, false, {})
  end
  echo("")
  require("cogitator.prompt").run("", function(text)
    echo(text)
    if text == "" then
      s.matches = nil
      M.select(origin, { record = false })
      return
    end
    s.matches = nav.matches(M.graph(), function(name)
      return filter.matches(text, name)
    end)
    M.select(nav.next_match(M.graph(), s.matches, origin, 1), { record = false })
  end)
  nav.visit(s.history, s.selected)
end

--- n/N: step through the search's matches.
---@param delta 1|-1
local function next_match(delta)
  local s = M._state
  if s.matches then
    M.select(nav.next_match(M.graph(), s.matches, s.selected, delta))
  end
end

--- The history's next stop, selected without recording a new visit.
---@param delta integer
local function travel(delta)
  local s = M._state
  M.select(nav.travel(s.history, delta), { record = false })
end

function M.close()
  local s = M._state
  if not s then
    return
  end
  M._state = nil
  pcall(vim.api.nvim_del_augroup_by_name, GROUP)
  if vim.api.nvim_win_is_valid(s.win) then
    vim.api.nvim_set_current_win(s.win)
    pcall(vim.cmd.tabclose)
  end
end

--- dbt.projectGraph, handed to `cb` (nil and a notification when empty).
---@param client vim.lsp.Client
---@param src_buf integer
---@param cb fun(data: table?)
function M.fetch_project(client, src_buf, cb)
  local uri = vim.api.nvim_buf_is_valid(src_buf) and vim.uri_from_bufnr(src_buf) or ""
  actions.exec_with(client, src_buf, "dbt.projectGraph", { uri = uri }, function(err, data)
    if err or not data or data == vim.NIL or #data.nodes == 0 then
      vim.notify(
        "dbt.projectGraph has nothing to show"
          .. (err and (": " .. err.message) or " (is the project context loaded yet?)"),
        vim.log.levels.WARN
      )
      return cb(nil)
    end
    cb(data)
  end)
end

--- R / after a save: fetch the project again and redraw the current
--- mode in place, keeping the selection.
function M.refresh()
  local s = M._state
  if not s or s.client:is_stopped() then
    return
  end
  M.fetch_project(s.client, s.src_buf, function(data)
    if data and M._state == s then
      s.project = data
      require("cogitator.dag_modes").redraw(M)
    end
  end)
end

--- The node for a buffer's file, if the project has one.
---@param data table
---@param buf integer
---@return string?
function M.node_of_buffer(data, buf)
  local uri = vim.uri_from_bufnr(buf)
  for _, node in ipairs(data.nodes) do
    if node.uri == uri then
      return node.id
    end
  end
end

local function new_tab()
  vim.cmd.tabnew()
  local buf = vim.api.nvim_get_current_buf()
  local win = vim.api.nvim_get_current_win()
  vim.bo[buf].buftype = "nofile"
  vim.bo[buf].bufhidden = "wipe"
  vim.bo[buf].swapfile = false
  vim.api.nvim_buf_set_name(buf, "cogitator://dag")
  vim.wo[win].wrap = false
  -- keeps the selection centred as it moves
  vim.wo[win].scrolloff = 999
  vim.wo[win].sidescrolloff = 999
  return buf, win
end

--- Fetch dbt.projectGraph and open the DAG in a new tab; with `opts.columns`
--- it opens straight on that model's (or column's) lineage graph, `true`
--- meaning the model of the current buffer (and `opts.column`, if any).
---@param opts {columns?: {model: string, column?: string}|true, column?: string}?
function M.open(opts)
  opts = opts or {}
  if M._state then
    M.close()
  end
  local src_buf = vim.api.nvim_get_current_buf()
  local client = actions.client_for(src_buf)
  if not client then
    return
  end
  M.fetch_project(client, src_buf, function(data)
    if not data then
      return
    end
    local settings = M.settings()
    local buf, win = new_tab()
    M._state = {
      project = data,
      data = data,
      mode = "models",
      focused = false,
      depth = settings.dag.focus_depth,
      follow = settings.dag.follow,
      expanded = {},
      all_expanded = false,
      boxes = {},
      edges_of = {},
      memory = nav.new_memory(),
      history = nav.new_history(),
      buf = buf,
      win = win,
      src_buf = src_buf,
      client = client,
    }
    local modes = require("cogitator.dag_modes")
    M.show(data, M.node_of_buffer(data, src_buf))
    require("cogitator.dag_keys").attach(buf, {
      state = function()
        return M._state
      end,
      graph = M.graph,
      select = M.select,
      search = search,
      next_match = next_match,
      travel = travel,
      close = M.close,
      open_node = open_node,
      toggle_node = toggle_node,
      toggle_all = function()
        M._state.all_expanded = not M._state.all_expanded
        render()
      end,
      lineage = function()
        modes.lineage_at_cursor(M)
      end,
      models = function()
        modes.models(M)
      end,
      toggle_focus = function()
        modes.toggle_focus(M)
      end,
      depth = function(delta)
        modes.change_depth(M, delta)
      end,
      toggle_follow = function()
        modes.toggle_follow(M)
      end,
      refresh = M.refresh,
    })
    vim.api.nvim_create_autocmd("CursorMoved", { buffer = buf, callback = sync_selection })
    modes.watch(M, GROUP, settings.dag)
    local start = opts.columns
    if start == true then
      local id = M.node_of_buffer(data, src_buf)
      local node = id and vim.iter(data.nodes):find(function(n)
        return n.id == id
      end)
      if not (node and node.kind == "model") then
        vim.notify("this buffer is not a model the project graph knows", vim.log.levels.WARN)
        return
      end
      start = { model = node.name, column = opts.column }
    end
    if start then
      modes.columns(M, start)
    end
  end)
end

return M
