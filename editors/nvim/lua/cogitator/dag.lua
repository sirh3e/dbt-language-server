--- :DbtDag — the whole project as a dbt-docs-style DAG (Neovim only).
--- Layers flow left→right (sources/roots first); each node is a box with
--- its tags and, when expanded, its documented columns (docglow-style).
--- Keys: <CR> open model, c/C toggle columns, L column lineage, R refresh,
--- q close.

local M = {}

M._state = nil

local GUTTER = 7
local MAX_COLUMNS = 8

-- -------------------------------------------------------------------------
-- canvas: a grid of unicode cells (cursor math is done in char cells)
-- -------------------------------------------------------------------------

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

--- Edge drawing never overwrites box content; crossing edges merge to ┼.
local function put_edge(grid, x, y, char)
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

-- -------------------------------------------------------------------------
-- layout
-- -------------------------------------------------------------------------

local function node_lines(node, expanded)
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
      if i > MAX_COLUMNS then
        table.insert(lines, ("… +%d more"):format(#node.columns - MAX_COLUMNS))
        break
      end
      table.insert(lines, "• " .. column.name)
    end
  end
  return lines
end

local function layout(data, expanded_set, all_expanded)
  local layers = {}
  for _, node in ipairs(data.nodes) do
    local layer = node.layer + 1
    layers[layer] = layers[layer] or {}
    table.insert(layers[layer], node)
  end

  local boxes = {} -- id → {node, lines, w, h, x, y, cx, cy}
  local x = 2
  local max_bottom = 0
  local centers = {}
  for li = 1, #layers do
    local column = layers[li] or {}
    -- barycenter-ish: order by the mean y of already-placed dependencies
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
      local lines = node_lines(node, expanded)
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
    x = x + width + GUTTER
  end
  return boxes, x, max_bottom
end

local function draw(boxes, edges, width, height)
  local grid = canvas_new(width, height)
  for _, box in pairs(boxes) do
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
  end

  for _, edge in ipairs(edges) do
    local from, to = boxes[edge.from], boxes[edge.to]
    if from and to then
      local sx, sy = from.x + from.w, from.cy
      local tx, ty = to.x - 1, to.cy
      if sy == ty then
        for x = sx, tx - 1 do
          put_edge(grid, x, sy, "─")
        end
      else
        local mid = math.max(sx + 1, tx - math.floor(GUTTER / 2))
        for x = sx, mid - 1 do
          put_edge(grid, x, sy, "─")
        end
        put_edge(grid, mid, sy, sy < ty and "╮" or "╯")
        local step = sy < ty and 1 or -1
        for y = sy + step, ty - step, step do
          put_edge(grid, mid, y, "│")
        end
        put_edge(grid, mid, ty, sy < ty and "╰" or "╭")
        for x = mid + 1, tx - 1 do
          put_edge(grid, x, ty, "─")
        end
      end
      put_edge(grid, tx, ty, "▸")
    end
  end

  local lines = {}
  for y = 1, height do
    lines[y] = table.concat(grid[y]):gsub("%s+$", "")
  end
  return lines
end

-- -------------------------------------------------------------------------
-- buffer + interaction
-- -------------------------------------------------------------------------

local function node_at_cursor()
  local s = M._state
  local row, byte_col = unpack(vim.api.nvim_win_get_cursor(0))
  local line = vim.api.nvim_get_current_line()
  local col = vim.fn.charidx(line, math.min(byte_col, #line)) + 1
  for _, box in pairs(s.boxes) do
    if row >= box.y and row <= box.y + box.h - 1 and col >= box.x and col <= box.x + box.w - 1 then
      return box.node
    end
  end
  return nil
end

local function render()
  local s = M._state
  local boxes, width, height = layout(s.data, s.expanded, s.all_expanded)
  s.boxes = boxes
  local lines = draw(boxes, s.data.edges, width + 2, height + 1)
  vim.bo[s.buf].modifiable = true
  vim.api.nvim_buf_set_lines(s.buf, 0, -1, false, lines)
  vim.bo[s.buf].modifiable = false
end

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

local function toggle_node()
  local node = node_at_cursor()
  if node then
    M._state.expanded[node.id] = not M._state.expanded[node.id]
    render()
  end
end

local function lineage_for_node()
  local s = M._state
  local node = node_at_cursor()
  if not (node and node.kind == "model" and node.uri ~= vim.NIL and node.uri) then
    return
  end
  s.client:exec_cmd({
    command = "dbt.showLineage",
    arguments = { { uri = node.uri } },
  }, { bufnr = s.src_buf })
end

function M.close()
  if M._state then
    pcall(vim.cmd.tabclose)
    M._state = nil
  end
end

function M.open()
  local src_buf = vim.api.nvim_get_current_buf()
  local client = vim.lsp.get_clients({ bufnr = src_buf, name = "cogitator" })[1]
  if not client then
    vim.notify("cogitator is not attached to this buffer", vim.log.levels.ERROR)
    return
  end
  client:exec_cmd({
    command = "dbt.projectGraph",
    arguments = { { uri = vim.uri_from_bufnr(src_buf) } },
  }, { bufnr = src_buf }, function(err, data)
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
    vim.bo[buf].buftype = "nofile"
    vim.bo[buf].bufhidden = "wipe"
    vim.bo[buf].swapfile = false
    vim.api.nvim_buf_set_name(buf, "cogitator://dag")
    vim.wo.wrap = false

    M._state = {
      data = data,
      expanded = {},
      all_expanded = false,
      boxes = {},
      buf = buf,
      src_buf = src_buf,
      client = client,
    }
    render()

    local function map(lhs, fn, desc)
      vim.keymap.set("n", lhs, fn, { buffer = buf, nowait = true, desc = desc })
    end
    map("q", M.close, "close the DAG")
    map("<CR>", open_node, "open the model under the cursor")
    map("c", toggle_node, "toggle columns of this node")
    map("C", function()
      M._state.all_expanded = not M._state.all_expanded
      render()
    end, "toggle columns everywhere")
    map("L", lineage_for_node, "column lineage for this model")
    map("R", function()
      M.close()
      vim.schedule(M.open)
    end, "refresh the DAG")
  end)
end

return M
