--- The DAG layout engine: pure functions from a dbt.projectGraph payload
--- to a character grid — no buffers, no windows, no LSP. The DAG view
--- (cogitator.dag) renders the result; tests can drive this directly.

local M = {}

M.GUTTER = 7
M.MAX_COLUMNS = 8

---@class cogitator.layout.Node
---@field id string
---@field name string
---@field kind "model"|"source"
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

--- An empty character grid (list of rows of single-char cells).
---@param width integer
---@param height integer
---@return string[][]
function M.canvas_new(width, height)
  local grid = {}
  for y = 1, height do
    local row = {}
    for x = 1, width do
      row[x] = " "
    end
    grid[y] = row
  end
  return grid
end

--- Set one cell unconditionally (box drawing).
---@param grid string[][]
---@param x integer
---@param y integer
---@param char string
function M.put(grid, x, y, char)
  if grid[y] and grid[y][x] then
    grid[y][x] = char
  end
end

local EDGE_CHARS = {
  ["─"] = true,
  ["│"] = true,
  ["┼"] = true,
  ["▸"] = true,
  ["╮"] = true,
  ["╯"] = true,
  ["╭"] = true,
  ["╰"] = true,
}

--- Set one cell for an edge: never overwrites box content; crossing
--- edges merge to ┼; arrowheads win over lines.
---@param grid string[][]
---@param x integer
---@param y integer
---@param char string
function M.put_edge(grid, x, y, char)
  local row = grid[y]
  if not row or not row[x] then
    return
  end
  local current = row[x]
  if current == " " then
    row[x] = char
  elseif EDGE_CHARS[current] and current ~= char and char ~= "▸" then
    row[x] = "┼"
  elseif char == "▸" then
    row[x] = char
  end
end

--- A node's box content: title (+[source]), #tags line, and — when
--- expanded — a rule plus up to MAX_COLUMNS column names.
---@param node cogitator.layout.Node
---@param expanded boolean
---@return string[]
function M.node_lines(node, expanded)
  local title = node.name .. (node.kind == "source" and "  [source]" or "")
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
        for _, edge in ipairs(data.edges) do
          if edge.to == node.id and centers[edge.from] then
            total, n = total + centers[edge.from], n + 1
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

--- Draw boxes and orthogonally-routed edges onto a fresh grid; returns
--- the buffer lines (right-trimmed).
---@param boxes table<string, cogitator.layout.Box>
---@param edges {from: string, to: string}[]
---@param width integer
---@param height integer
---@return string[]
function M.draw(boxes, edges, width, height)
  local grid = M.canvas_new(width, height)
  for _, box in pairs(boxes) do
    local x2 = box.x + box.w - 1
    local y2 = box.y + box.h - 1
    M.put(grid, box.x, box.y, "╭")
    M.put(grid, x2, box.y, "╮")
    M.put(grid, box.x, y2, "╰")
    M.put(grid, x2, y2, "╯")
    for x = box.x + 1, x2 - 1 do
      M.put(grid, x, box.y, "─")
      M.put(grid, x, y2, "─")
    end
    for y = box.y + 1, y2 - 1 do
      M.put(grid, box.x, y, "│")
      M.put(grid, x2, y, "│")
    end
    for i, line in ipairs(box.lines) do
      local cx = box.x + 2
      for _, char in ipairs(vim.fn.split(line, "\\zs")) do
        M.put(grid, cx, box.y + i, char)
        cx = cx + 1
      end
    end
  end

  for _, edge in ipairs(edges) do
    local from, to = boxes[edge.from], boxes[edge.to]
    if from and to then
      local sx, sy = from.x + from.w, from.cy
      local tx, ty = to.x - 1, to.cy
      if sy == ty then
        for x = sx, tx - 1 do
          M.put_edge(grid, x, sy, "─")
        end
      else
        local mid = math.max(sx + 1, tx - math.floor(M.GUTTER / 2))
        for x = sx, mid - 1 do
          M.put_edge(grid, x, sy, "─")
        end
        M.put_edge(grid, mid, sy, sy < ty and "╮" or "╯")
        local step = sy < ty and 1 or -1
        for y = sy + step, ty - step, step do
          M.put_edge(grid, mid, y, "│")
        end
        M.put_edge(grid, mid, ty, sy < ty and "╰" or "╭")
        for x = mid + 1, tx - 1 do
          M.put_edge(grid, x, ty, "─")
        end
      end
      M.put_edge(grid, tx, ty, "▸")
    end
  end

  local lines = {}
  for y = 1, height do
    lines[y] = table.concat(grid[y]):gsub("%s+$", "")
  end
  return lines
end

return M
