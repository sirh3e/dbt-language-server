--- Node-to-node navigation over a laid-out DAG. Pure: selections go in
--- and come out as node ids; nothing here touches a buffer.

local M = {}

---@class cogitator.dag_nav.Graph
---@field boxes table<string, cogitator.layout.Box>
---@field index cogitator.layout.Index
---@field adjacency {parents: table<string, string[]>, children: table<string, string[]>}

--- The non-empty layer numbers, ascending.
---@param g cogitator.dag_nav.Graph
---@return integer[]
local function layer_numbers(g)
  local numbers = vim.tbl_keys(g.index.layers)
  table.sort(numbers)
  return numbers
end

--- Every node in reading order: layer by layer, top to bottom.
---@param g cogitator.dag_nav.Graph
---@return string[] ids
function M.ordered(g)
  local ids = {}
  for _, n in ipairs(layer_numbers(g)) do
    for _, box in ipairs(g.index.layers[n]) do
      ids[#ids + 1] = box.node.id
    end
  end
  return ids
end

--- The id at `n` in reading order, clamped to the ends.
---@param g cogitator.dag_nav.Graph
---@param n integer 1-based
---@return string?
function M.nth(g, n)
  local ids = M.ordered(g)
  return ids[math.max(1, math.min(n, #ids))]
end

--- The last id in reading order.
---@param g cogitator.dag_nav.Graph
---@return string?
function M.last(g)
  local ids = M.ordered(g)
  return ids[#ids]
end

--- The next or previous node in the same layer; stays put at the ends.
---@param g cogitator.dag_nav.Graph
---@param id string
---@param delta integer
---@return string
function M.in_layer(g, id, delta)
  local layer = g.index.layers[g.boxes[id].layer]
  for i, box in ipairs(layer) do
    if box.node.id == id then
      local target = layer[math.max(1, math.min(i + delta, #layer))]
      return target.node.id
    end
  end
  return id
end

--- The candidate closest to `id` by row; names break ties.
local function nearest(g, id, candidates)
  local cy = g.boxes[id].cy
  local best
  for _, candidate in ipairs(candidates) do
    local box = g.boxes[candidate]
    if box then
      if best == nil then
        best = box
      else
        local d, best_d = math.abs(box.cy - cy), math.abs(best.cy - cy)
        if d < best_d or (d == best_d and box.node.name < best.node.name) then
          best = box
        end
      end
    end
  end
  return best and best.node.id
end

--- Where the mirror-image move last came from, so `l` undoes `h`.
---@class cogitator.dag_nav.Memory
---@field parent table<string, string> node → the parent it was reached from
---@field child table<string, string> node → the child it was reached from

---@return cogitator.dag_nav.Memory
function M.new_memory()
  return { parent = {}, child = {} }
end

--- A connected parent or child: the one we came from if any, else the
--- nearest by row. Returns `id` when nothing is connected on that side.
---@param g cogitator.dag_nav.Graph
---@param memory cogitator.dag_nav.Memory
---@param id string
---@param side "parent"|"child"
---@return string
function M.neighbour(g, memory, id, side)
  local list = g.adjacency[side == "parent" and "parents" or "children"][id] or {}
  local remembered = memory[side][id]
  local target = (remembered and vim.tbl_contains(list, remembered)) and remembered or nearest(g, id, list)
  if not target then
    return id
  end
  -- moving to a parent means that parent's child side leads back here
  memory[side == "parent" and "child" or "parent"][target] = id
  return target
end

--- The node in layer `count` from either end closest to `id` by row.
---@param g cogitator.dag_nav.Graph
---@param id string
---@param from_end boolean count from the last layer
---@param count integer
---@return string
function M.layer_end(g, id, from_end, count)
  local numbers = layer_numbers(g)
  local at = math.max(1, math.min(count, #numbers))
  local layer = g.index.layers[numbers[from_end and #numbers + 1 - at or at]]
  local ids = {}
  for _, box in ipairs(layer) do
    ids[#ids + 1] = box.node.id
  end
  return nearest(g, id, ids) or id
end

--- Ids whose name matches, in reading order.
---@param g cogitator.dag_nav.Graph
---@param matcher fun(name: string): boolean
---@return string[]
function M.matches(g, matcher)
  return vim.tbl_filter(function(id)
    return matcher(g.boxes[id].node.name)
  end, M.ordered(g))
end

--- The match after (`delta` 1) or before (-1) `id` in reading order,
--- wrapping. `id` needn't be a match itself.
---@param g cogitator.dag_nav.Graph
---@param matches string[]
---@param id string
---@param delta 1|-1
---@return string?
function M.next_match(g, matches, id, delta)
  if #matches == 0 then
    return nil
  end
  local rank = {}
  for i, other in ipairs(M.ordered(g)) do
    rank[other] = i
  end
  local here = rank[id]
  if delta > 0 then
    for _, match in ipairs(matches) do
      if rank[match] > here then
        return match
      end
    end
    return matches[1]
  end
  for i = #matches, 1, -1 do
    if rank[matches[i]] < here then
      return matches[i]
    end
  end
  return matches[#matches]
end

local HISTORY_LIMIT = 100

---@class cogitator.dag_nav.History
---@field ids string[]
---@field pos integer

---@return cogitator.dag_nav.History
function M.new_history()
  return { ids = {}, pos = 0 }
end

--- Record a visit, dropping anything ahead of the current position.
---@param h cogitator.dag_nav.History
---@param id string
function M.visit(h, id)
  if h.ids[h.pos] == id then
    return
  end
  for i = #h.ids, h.pos + 1, -1 do
    h.ids[i] = nil
  end
  h.ids[#h.ids + 1] = id
  if #h.ids > HISTORY_LIMIT then
    table.remove(h.ids, 1)
  end
  h.pos = #h.ids
end

--- Move through the history; nil at either end.
---@param h cogitator.dag_nav.History
---@param delta integer
---@return string?
function M.travel(h, delta)
  local id = h.ids[h.pos + delta]
  if id then
    h.pos = h.pos + delta
  end
  return id
end

return M
