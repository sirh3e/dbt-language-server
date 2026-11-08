--- :DbtDag — the whole project as a dbt-docs-style DAG (Neovim only).
--- Layers flow left→right (sources/roots first); each node is a box in
--- its dbt-docs colour with its tags and, when expanded, its documented
--- columns (docglow-style).
--- Keys: h/l parent/child along edges, j/k within the layer, gg/G first/
--- last node, {/} first/last layer, / search with n/N, <C-o>/<C-i>
--- history, <CR> open model, c/C toggle columns, L column lineage,
--- R refresh, q close. The pure engines are cogitator.layout (placement)
--- and cogitator.dag_nav (movement).

local actions = require("cogitator.actions")
local filter = require("cogitator.explorer.filter")
local layout_lib = require("cogitator.layout")
local nav = require("cogitator.dag_nav")
local paint = require("cogitator.dag_paint")

local M = {}

---@class cogitator.dag.State
---@field data table the dbt.projectGraph payload
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
---@field buf integer the cogitator://dag scratch buffer
---@field win integer
---@field src_buf integer
---@field client vim.lsp.Client
---@type cogitator.dag.State?
M._state = nil

--- The box under the cursor (char-cell hit test).
---@return cogitator.layout.Box?
local function box_at_cursor()
  local s = M._state
  local row, byte_col = unpack(vim.api.nvim_win_get_cursor(s.win))
  local line = vim.api.nvim_buf_get_lines(s.buf, row - 1, row, false)[1] or ""
  local col = vim.fn.charidx(line, math.min(byte_col, #line)) + 1
  return layout_lib.box_at(s.index, row, col)
end

---@return cogitator.layout.Node?
local function node_at_cursor()
  local box = box_at_cursor()
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

--- Make `id` the selection: paint it, move the cursor, record the visit.
---@param id string?
---@param opts {record?: boolean, move?: boolean}?
local function select_node(id, opts)
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
  select_node(s.selected, { record = false })
end

---@return cogitator.dag_nav.Graph
local function graph()
  local s = M._state
  return { boxes = s.boxes, index = s.index, adjacency = s.adjacency }
end

--- <CR>: close the DAG tab and edit the node's file.
local function open_node()
  local node = node_at_cursor()
  if not node then
    return
  end
  if not node.uri or node.uri == vim.NIL then
    vim.notify("no file for " .. node.name, vim.log.levels.WARN)
    return
  end
  vim.cmd.tabclose()
  M._state = nil
  vim.cmd.edit(vim.uri_to_fname(node.uri))
end

--- c: toggle the columns of the node under the cursor.
local function toggle_node()
  local node = node_at_cursor()
  if node then
    M._state.expanded[node.id] = not M._state.expanded[node.id]
    render()
  end
end

--- L: column lineage (message-rendered) for the model under the cursor.
local function lineage_for_node()
  local s = M._state
  local node = node_at_cursor()
  if not (node and node.kind == "model" and node.uri ~= vim.NIL and node.uri) then
    return
  end
  actions.exec_with(s.client, s.src_buf, "dbt.showLineage", { uri = node.uri })
end

--- Selection follows native motions and the mouse.
local function sync_selection()
  local s = M._state
  if not s then
    return
  end
  local box = box_at_cursor()
  if box and box.node.id ~= s.selected then
    select_node(box.node.id, { move = false })
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
      select_node(origin, { record = false })
      return
    end
    s.matches = nav.matches(graph(), function(name)
      return filter.matches(text, name)
    end)
    select_node(nav.next_match(graph(), s.matches, origin, 1), { record = false })
  end)
  nav.visit(s.history, s.selected)
end

--- n/N: step through the search's matches.
---@param delta 1|-1
local function next_match(delta)
  local s = M._state
  if s.matches then
    select_node(nav.next_match(graph(), s.matches, s.selected, delta))
  end
end

--- The history's next stop, selected without recording a new visit.
---@param delta integer
local function travel(delta)
  local s = M._state
  select_node(nav.travel(s.history, delta), { record = false })
end

function M.close()
  if M._state then
    pcall(vim.cmd.tabclose)
    M._state = nil
  end
end

--- The node for the file the DAG was opened from, else the first node.
---@param data table
---@param src_buf integer
---@return string?
local function initial_selection(data, src_buf)
  local uri = vim.uri_from_bufnr(src_buf)
  for _, node in ipairs(data.nodes) do
    if node.uri == uri then
      return node.id
    end
  end
  return nav.nth(graph(), 1)
end

--- Fetch dbt.projectGraph and open the DAG in a new tab.
function M.open()
  local src_buf = vim.api.nvim_get_current_buf()
  local client = actions.client_for(src_buf)
  if not client then
    return
  end
  actions.exec_with(client, src_buf, "dbt.projectGraph", { uri = vim.uri_from_bufnr(src_buf) }, function(err, data)
    if err or not data or data == vim.NIL or #data.nodes == 0 then
      vim.notify(
        "dbt.projectGraph has nothing to show"
          .. (err and (": " .. err.message) or " (is the project context loaded yet?)"),
        vim.log.levels.WARN
      )
      return
    end
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

    local edges_of = {}
    for i, edge in ipairs(data.edges) do
      for _, id in ipairs({ edge.from, edge.to }) do
        edges_of[id] = edges_of[id] or {}
        table.insert(edges_of[id], i)
      end
    end

    M._state = {
      data = data,
      expanded = {},
      all_expanded = false,
      boxes = {},
      adjacency = layout_lib.adjacency(data.edges),
      edges_of = edges_of,
      memory = nav.new_memory(),
      history = nav.new_history(),
      buf = buf,
      win = win,
      src_buf = src_buf,
      client = client,
    }
    render()
    select_node(initial_selection(data, src_buf))
    require("cogitator.dag_keys").attach(buf, {
      state = function()
        return M._state
      end,
      graph = graph,
      select = select_node,
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
      lineage = lineage_for_node,
      refresh = function()
        M.close()
        vim.schedule(M.open)
      end,
    })
    vim.api.nvim_create_autocmd("CursorMoved", { buffer = buf, callback = sync_selection })
  end)
end

return M
