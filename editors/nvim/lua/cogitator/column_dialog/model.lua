--- The column-lineage dialog's pure core: the graph as an index, the three
--- Miller panes (parents | the list the cursor is in | children), walking
--- between them yazi-style, fzf-syntax filtering and how a row is drawn.
--- No windows and no LSP traffic here; every function takes values and
--- returns values.
---
---   upstream ⯇           columns            ⯈ downstream
---   stg_orders.customer  customers.customer_id ⋈   orders.customer_id
---
--- A pane is derived from the cursor: the left one lists what feeds the
--- cursor's column, the right one what reads it. `h` makes the left pane
--- the list, `l` the right one; walking back the way one came lands on the
--- column one came from (the trail).

local M = {}

--- Row glyphs: a column's owner kind, and how an edge carries it.
M.ICONS = { model = "▦", source = "◆", unknown = "?" }
M.EDGE_TAGS = { join = "⋈", derived = "ƒ", direct = "=" }
M.HL = {
  model = "DbtDagModelFg",
  source = "DbtDagSourceFg",
  unknown = "DbtDagDim",
  join = "DbtColumnJoin",
  derived = "DbtColumnDerived",
  direct = "DbtColumnDirect",
  count = "DbtPickerDetail",
}

---@class cogitator.column_dialog.Edge
---@field from string
---@field to string
---@field kind "direct"|"derived"|"join"

---@class cogitator.column_dialog.Index
---@field nodes table<string, table> by id
---@field order string[] node ids as the graph listed them
---@field parents table<string, cogitator.column_dialog.Edge[]> edges into an id
---@field children table<string, cogitator.column_dialog.Edge[]> edges out of an id

--- The column a node names: its own field, else what follows `::` in its id.
---@param node table
---@return string
function M.column_of(node)
  if node.column and node.column ~= vim.NIL then
    return node.column
  end
  return (node.id or ""):match("::(.*)$") or node.name or ""
end

--- An edge's kind: the server's when it says, else a guess from the two
--- columns (the same name is passed through, any other is derived). The
--- one-hop answer cannot tell a join apart, so a guess is never a join.
---@param edge {from: string, to: string, kind?: string}
---@param nodes table<string, table>
---@return "direct"|"derived"|"join"
function M.edge_kind(edge, nodes)
  if edge.kind == "direct" or edge.kind == "derived" or edge.kind == "join" then
    return edge.kind
  end
  local from, to = nodes[edge.from], nodes[edge.to]
  if from and to and M.column_of(from):lower() == M.column_of(to):lower() then
    return "direct"
  end
  return "derived"
end

--- Index a {nodes, edges} payload. Edges to unknown nodes are dropped.
---@param data {nodes: table[], edges: table[]}?
---@return cogitator.column_dialog.Index
function M.index(data)
  local ix = { nodes = {}, order = {}, parents = {}, children = {} }
  for _, node in ipairs(data and data.nodes or {}) do
    if node.id and not ix.nodes[node.id] then
      ix.nodes[node.id] = node
      ix.order[#ix.order + 1] = node.id
    end
  end
  local seen = {}
  for _, e in ipairs(data and data.edges or {}) do
    local key = tostring(e.from) .. "\0" .. tostring(e.to)
    if ix.nodes[e.from] and ix.nodes[e.to] and not seen[key] then
      seen[key] = true
      local edge = { from = e.from, to = e.to, kind = M.edge_kind(e, ix.nodes) }
      ix.parents[e.to] = ix.parents[e.to] or {}
      table.insert(ix.parents[e.to], edge)
      ix.children[e.from] = ix.children[e.from] or {}
      table.insert(ix.children[e.from], edge)
    end
  end
  return ix
end

local function by_name(ix)
  return function(a, b)
    local na, nb = ix.nodes[a].name or a, ix.nodes[b].name or b
    if na == nb then
      return a < b
    end
    return na < nb
  end
end

--- Ids one hop away: "left" the feeders, "right" the readers, by name.
---@param ix cogitator.column_dialog.Index
---@param id string?
---@param side "left"|"right"
---@return string[]
function M.neighbours(ix, id, side)
  local out = {}
  local edges = (side == "left" and ix.parents or ix.children)[id or ""] or {}
  for _, e in ipairs(edges) do
    out[#out + 1] = side == "left" and e.from or e.to
  end
  table.sort(out, by_name(ix))
  return out
end

--- The kind of the edge between two ids, either way round.
---@return string?
function M.edge_between(ix, a, b)
  for _, e in ipairs(ix.children[a] or {}) do
    if e.to == b then
      return e.kind
    end
  end
  for _, e in ipairs(ix.children[b] or {}) do
    if e.to == a then
      return e.kind
    end
  end
  return nil
end

--- Does any edge out of `id` feed a join?
function M.feeds_join(ix, id)
  for _, e in ipairs(ix.children[id] or {}) do
    if e.kind == "join" then
      return true
    end
  end
  return false
end

---@class cogitator.column_dialog.List
---@field model string? every column of this model
---@field parents string? the feeders of this id
---@field children string? the readers of this id

---@class cogitator.column_dialog.Nav
---@field list cogitator.column_dialog.List what the middle pane lists
---@field cursor string? the id under the cursor
---@field filter string fzf syntax over the middle pane
---@field trail {side: "left"|"right", list: cogitator.column_dialog.List, cursor: string?}[]

--- The navigation a dialog opens with: the model's columns, the cursor on
--- `start` (an id) when given.
---@param model string
---@param start string?
---@return cogitator.column_dialog.Nav
function M.start(model, start)
  return { list = { model = model }, cursor = start, filter = "", trail = {} }
end

--- What a list holds now (the graph grows while the walk runs).
---@param ix cogitator.column_dialog.Index
---@param list cogitator.column_dialog.List
---@return string[]
function M.list_ids(ix, list)
  if list.parents then
    return M.neighbours(ix, list.parents, "left")
  elseif list.children then
    return M.neighbours(ix, list.children, "right")
  end
  local out = {}
  for _, id in ipairs(ix.order) do
    if ix.nodes[id].model == list.model then
      out[#out + 1] = id
    end
  end
  return out
end

--- The text a filter is matched against: the label.
local function haystack(node)
  return node.name or node.id
end

--- Does `node` pass the fzf-syntax filter?
---@param node table
---@param filter string
---@return boolean
function M.matches(node, filter)
  if not filter or vim.trim(filter) == "" then
    return true
  end
  local query = require("cogitator.pick.query")
  local groups = query.parse(filter)
  return #groups == 0 or query.match(groups, haystack(node)) ~= nil
end

--- The middle pane's ids under the filter.
---@param ix cogitator.column_dialog.Index
---@param nav cogitator.column_dialog.Nav
---@return string[]
function M.visible(ix, nav)
  return vim.tbl_filter(function(id)
    return M.matches(ix.nodes[id], nav.filter)
  end, M.list_ids(ix, nav.list))
end

--- The cursor's id, kept on a visible row (the first when it is not).
---@return string?
function M.current(ix, nav)
  local ids = M.visible(ix, nav)
  if nav.cursor and vim.tbl_contains(ids, nav.cursor) then
    return nav.cursor
  end
  return ids[1]
end

--- A copy of `nav` with `fields` replaced.
local function with(nav, fields)
  return vim.tbl_extend("force", nav, fields)
end

--- Move the cursor to an id of the middle pane.
function M.select(nav, id)
  return with(nav, { cursor = id })
end

--- Move the cursor by `delta` visible rows, clamped.
function M.move(ix, nav, delta)
  local ids = M.visible(ix, nav)
  if #ids == 0 then
    return nav
  end
  local at = 1
  for i, id in ipairs(ids) do
    if id == M.current(ix, nav) then
      at = i
    end
  end
  return M.select(nav, ids[math.max(1, math.min(#ids, at + delta))])
end

--- Set the filter (the cursor stays on its column when it still shows).
function M.filtered(nav, filter)
  return with(nav, { filter = filter or "" })
end

local OPPOSITE = { left = "right", right = "left" }

--- Walk to the side pane: it becomes the middle one. Going back the way
--- one came restores the list and the column one came from. Returns nil
--- and why when there is nothing that way.
---@param ix cogitator.column_dialog.Index
---@param nav cogitator.column_dialog.Nav
---@param side "left"|"right"
---@return cogitator.column_dialog.Nav? nav
---@return string? reason
function M.walk(ix, nav, side)
  local here = M.current(ix, nav)
  if not here then
    return nil, "nothing selected"
  end
  local ids = M.neighbours(ix, here, side)
  if #ids == 0 then
    return nil, side == "left" and "nothing feeds this column" or "nothing reads this column"
  end
  local top = nav.trail[#nav.trail]
  local trail = vim.list_slice(nav.trail)
  if top and top.side == OPPOSITE[side] and top.cursor and vim.tbl_contains(ids, top.cursor) then
    trail[#trail] = nil
    return { list = top.list, cursor = top.cursor, filter = "", trail = trail }
  end
  trail[#trail + 1] = { side = side, list = nav.list, cursor = here }
  local list = side == "left" and { parents = here } or { children = here }
  return { list = list, cursor = ids[1], filter = "", trail = trail }
end

return M
