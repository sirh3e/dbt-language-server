--- The DAG layout engine: pure functions from a dbt.projectGraph payload
--- to a character grid — no buffers, no windows, no LSP. The DAG view
--- (cogitator.dag) renders the result; tests can drive this directly.

local canvas = require("cogitator.canvas")

local M = {}

M.GUTTER = canvas.GUTTER
M.MAX_COLUMNS = 8

---@class cogitator.layout.Node
---@field id string
---@field name string
---@field kind "model"|"source"|"exposure"|"metric"|"semantic_model"|"test"|"analysis"
---@field layer integer 0-based layer from the server
---@field tags string[]
---@field columns {name: string, description: string}[]

---@class cogitator.layout.Box
---@field node cogitator.layout.Node
---@field lines string[] rendered content lines (title, tags, columns)
---@field w integer outer width in cells
---@field h integer outer height in rows
---@field x integer left edge (1-based grid column)
---@field y integer top edge (1-based grid row)
---@field cy integer vertical center, where edges attach
---@field layer integer 1-based layer

--- A node's box content: title (+[source]), #tags line, and — when
--- expanded — a rule plus up to MAX_COLUMNS column names.
---@param node cogitator.layout.Node
---@param expanded boolean
---@return string[]
function M.node_lines(node, expanded)
  local badge = ""
  if node.kind ~= "model" then
    badge = "  [" .. node.kind:gsub("_", " ") .. "]"
  end
  local title = node.name .. badge
  local lines = { title }
  if node.tags and #node.tags > 0 then
    local tags = {}
    for _, tag in ipairs(node.tags) do
      table.insert(tags, "#" .. tag)
    end
    table.insert(lines, table.concat(tags, " "))
  end
  if expanded and node.columns and #node.columns > 0 then
    table.insert(lines, string.rep("╌", vim.fn.strchars(title)))
    for i, column in ipairs(node.columns) do
      if i > M.MAX_COLUMNS then
        table.insert(lines, ("… +%d more"):format(#node.columns - M.MAX_COLUMNS))
        break
      end
      table.insert(lines, "• " .. column.name)
    end
  end
  return lines
end

--- Place every node: columns come from the server's layers; within a
--- layer, nodes order by the barycenter (mean center of already-placed
--- dependencies), names breaking ties.
---@param data {nodes: cogitator.layout.Node[], edges: {from: string, to: string}[]}
---@param expanded_set table<string, boolean>
---@param all_expanded boolean
---@return table<string, cogitator.layout.Box> boxes by node id
---@return integer width total grid width
---@return integer height total grid height
function M.layout(data, expanded_set, all_expanded)
  local layers = {}
  for _, node in ipairs(data.nodes) do
    local layer = node.layer + 1
    layers[layer] = layers[layer] or {}
    table.insert(layers[layer], node)
  end

  local parents = M.adjacency(data.edges).parents
  local boxes = {}
  local x = 2
  local max_bottom = 0
  local centers = {}
  for li = 1, #layers do
    local column = layers[li] or {}
    if li > 1 then
      local score = {}
      for _, node in ipairs(column) do
        local total, n = 0, 0
        for _, parent in ipairs(parents[node.id] or {}) do
          if centers[parent] then
            total, n = total + centers[parent], n + 1
          end
        end
        score[node.id] = n > 0 and (total / n) or math.huge
      end
      table.sort(column, function(a, b)
        if score[a.id] == score[b.id] then
          return a.name < b.name
        end
        return score[a.id] < score[b.id]
      end)
    end

    local width = 0
    local y = 2
    for _, node in ipairs(column) do
      local expanded = all_expanded or expanded_set[node.id] == true
      local lines = M.node_lines(node, expanded)
      local inner = 0
      for _, line in ipairs(lines) do
        inner = math.max(inner, vim.fn.strchars(line))
      end
      local box = {
        node = node,
        lines = lines,
        w = inner + 4,
        h = #lines + 2,
        x = x,
        y = y,
        layer = li,
      }
      box.cy = y + math.floor(box.h / 2)
      boxes[node.id] = box
      centers[node.id] = box.cy
      width = math.max(width, box.w)
      y = y + box.h + 1
      max_bottom = math.max(max_bottom, y)
    end
    x = x + width + M.GUTTER
  end
  return boxes, x, max_bottom
end

M.draw = canvas.draw

--- Direct dependencies and dependents of every node.
---@param edges {from: string, to: string}[]
---@return {parents: table<string, string[]>, children: table<string, string[]>}
function M.adjacency(edges)
  local parents, children = {}, {}
  for _, edge in ipairs(edges) do
    parents[edge.to] = parents[edge.to] or {}
    table.insert(parents[edge.to], edge.from)
    children[edge.from] = children[edge.from] or {}
    table.insert(children[edge.from], edge.to)
  end
  return { parents = parents, children = children }
end

---@class cogitator.layout.Index
---@field rows table<integer, cogitator.layout.Box[]> boxes occupying each row
---@field layers cogitator.layout.Box[][] boxes per layer, top to bottom

--- Position lookups over placed boxes.
---@param boxes table<string, cogitator.layout.Box>
---@return cogitator.layout.Index
function M.index(boxes)
  local rows, layers = {}, {}
  for _, box in pairs(boxes) do
    for y = box.y, box.y + box.h - 1 do
      rows[y] = rows[y] or {}
      table.insert(rows[y], box)
    end
    layers[box.layer] = layers[box.layer] or {}
    table.insert(layers[box.layer], box)
  end
  for _, layer in pairs(layers) do
    table.sort(layer, function(a, b)
      return a.y < b.y
    end)
  end
  return { rows = rows, layers = layers }
end

--- The box covering a grid cell, if any.
---@param index cogitator.layout.Index
---@param row integer
---@param col integer
---@return cogitator.layout.Box?
function M.box_at(index, row, col)
  for _, box in ipairs(index.rows[row] or {}) do
    if col >= box.x and col <= box.x + box.w - 1 then
      return box
    end
  end
  return nil
end

return M
