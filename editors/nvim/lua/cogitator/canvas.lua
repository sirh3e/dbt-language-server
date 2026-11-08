--- Character-grid drawing for the DAG: boxes, orthogonal edges, and the
--- byte ranges needed to colour them. Pure — no buffers, no windows.

local M = {}

M.GUTTER = 7

---@alias cogitator.canvas.Range {[1]: integer, [2]: integer, [3]: integer} 0-based row, byte start, byte end

---@class cogitator.canvas.Decor
---@field boxes table<string, cogitator.canvas.Range[]> node id → filled ranges
---@field edges table<integer, cogitator.canvas.Range[]> index into edges → drawn ranges

local function canvas_new(width, height)
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

local function put(grid, x, y, char)
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

--- Set one edge cell: never touches a box; crossing edges merge to ┼ and
--- arrowheads win over lines.
---@return boolean drawn
local function put_edge(grid, boxed, x, y, char)
  local row = grid[y]
  if not row or not row[x] or (boxed[y] and boxed[y][x]) then
    return false
  end
  local current = row[x]
  if current == " " or char == "▸" then
    row[x] = char
  elseif EDGE_CHARS[current] and current ~= char then
    row[x] = "┼"
  end
  return true
end

local function draw_box(grid, boxed, box)
  local x2 = box.x + box.w - 1
  local y2 = box.y + box.h - 1
  put(grid, box.x, box.y, "╭")
  put(grid, x2, box.y, "╮")
  put(grid, box.x, y2, "╰")
  put(grid, x2, y2, "╯")
  for x = box.x + 1, x2 - 1 do
    put(grid, x, box.y, "─")
    put(grid, x, y2, "─")
  end
  for y = box.y + 1, y2 - 1 do
    put(grid, box.x, y, "│")
    put(grid, x2, y, "│")
  end
  for i, line in ipairs(box.lines) do
    local cx = box.x + 2
    for _, char in ipairs(vim.fn.split(line, "\\zs")) do
      put(grid, cx, box.y + i, char)
      cx = cx + 1
    end
  end
  for y = box.y, y2 do
    boxed[y] = boxed[y] or {}
    for x = box.x, x2 do
      boxed[y][x] = true
    end
  end
end

--- Route one edge, returning the cells actually drawn.
---@return {[1]: integer, [2]: integer}[] cells {x, y}
local function draw_edge(grid, boxed, from, to, gutter)
  local cells = {}
  local function cell(x, y, char)
    if put_edge(grid, boxed, x, y, char) then
      cells[#cells + 1] = { x, y }
    end
  end
  local sx, sy = from.x + from.w, from.cy
  local tx, ty = to.x - 1, to.cy
  if sy == ty then
    for x = sx, tx - 1 do
      cell(x, sy, "─")
    end
  else
    local mid = math.max(sx + 1, tx - math.floor(gutter / 2))
    for x = sx, mid - 1 do
      cell(x, sy, "─")
    end
    cell(mid, sy, sy < ty and "╮" or "╯")
    local step = sy < ty and 1 or -1
    for y = sy + step, ty - step, step do
      cell(mid, y, "│")
    end
    cell(mid, ty, sy < ty and "╰" or "╭")
    for x = mid + 1, tx - 1 do
      cell(x, ty, "─")
    end
  end
  cell(tx, ty, "▸")
  return cells
end

--- Byte offset of every cell, per row; grids hold multi-byte glyphs.
local function byte_offsets(grid, width)
  local offsets = {}
  for y, row in ipairs(grid) do
    local at, line = 0, {}
    for x = 1, width do
      line[x] = at
      at = at + #row[x]
    end
    line[width + 1] = at
    offsets[y] = line
  end
  return offsets
end

--- Merge horizontally touching cells into byte ranges.
local function cell_ranges(cells, offsets)
  table.sort(cells, function(a, b)
    return a[2] == b[2] and a[1] < b[1] or a[2] < b[2]
  end)
  local ranges, run_x, last = {}, nil, nil
  local function flush()
    if last then
      local row = offsets[last[2]]
      ranges[#ranges + 1] = { last[2] - 1, row[run_x], row[last[1] + 1] }
    end
  end
  for _, c in ipairs(cells) do
    if not (last and c[2] == last[2] and c[1] <= last[1] + 1) then
      flush()
      run_x = c[1]
    end
    last = c
  end
  flush()
  return ranges
end

--- Draw boxes, then edges, onto a fresh grid.
---@param boxes table<string, cogitator.layout.Box>
---@param edges {from: string, to: string}[]
---@param width integer
---@param height integer
---@return string[] lines right-trimmed buffer lines
---@return cogitator.canvas.Decor decor
function M.draw(boxes, edges, width, height)
  local grid = canvas_new(width, height)
  local boxed = {}
  for _, box in pairs(boxes) do
    draw_box(grid, boxed, box)
  end

  local edge_cells = {}
  for i, edge in ipairs(edges) do
    local from, to = boxes[edge.from], boxes[edge.to]
    if from and to then
      edge_cells[i] = draw_edge(grid, boxed, from, to, M.GUTTER)
    end
  end

  local offsets = byte_offsets(grid, width)
  local decor = { boxes = {}, edges = {} }
  for id, box in pairs(boxes) do
    local ranges = {}
    for y = box.y, box.y + box.h - 1 do
      local row = offsets[y]
      ranges[#ranges + 1] = { y - 1, row[box.x], row[box.x + box.w] }
    end
    decor.boxes[id] = ranges
  end
  for i, cells in pairs(edge_cells) do
    decor.edges[i] = cell_ranges(cells, offsets)
  end

  local lines = {}
  for y = 1, height do
    lines[y] = table.concat(grid[y]):gsub("%s+$", "")
  end
  return lines, decor
end

return M
