--- How the column-lineage dialog draws its panes: one row per column,
--- " ▦ model.column ······ ⋈ ↑2 ↓1", the name cut to fit and the tags
--- right-aligned, and the panes' titles. Pure over the model's index and
--- navigation (cogitator.column_dialog.model).

local model = require("cogitator.column_dialog.model")

local M = {}

---@class cogitator.column_dialog.Row
---@field id string
---@field text string
---@field marks {col: integer, stop: integer, hl: string}[]

--- One row. `relative` names the column the row is a neighbour of, whose
--- edge kind the row shows; `width` (cells) cuts the name to fit and
--- right-aligns the tags.
---@param ix cogitator.column_dialog.Index
---@param id string
---@param relative string?
---@param width integer?
---@return cogitator.column_dialog.Row
function M.row(ix, id, relative, width)
  local node = ix.nodes[id]
  local kind = model.ICONS[node.kind] and node.kind or "unknown"
  local tail = {}
  local edge = relative and model.edge_between(ix, id, relative)
  if edge then
    tail[#tail + 1] = { model.EDGE_TAGS[edge], model.HL[edge] }
  end
  if model.feeds_join(ix, id) and edge ~= "join" then
    tail[#tail + 1] = { model.EDGE_TAGS.join, model.HL.join }
  end
  local up, down = #(ix.parents[id] or {}), #(ix.children[id] or {})
  if up + down > 0 then
    tail[#tail + 1] = { ("↑%d ↓%d"):format(up, down), model.HL.count }
  end
  local head = " " .. model.ICONS[kind] .. " "
  local name = node.name or id
  local function joined()
    local out = {}
    for _, t in ipairs(tail) do
      out[#out + 1] = t[1]
    end
    return table.concat(out, " ")
  end
  local tail_text = joined()
  -- a narrow pane gives the name the room the counts took
  local function overflows()
    return width and vim.fn.strdisplaywidth(head .. name .. "  " .. tail_text) > width
  end
  if overflows() and #tail > 0 and tail[#tail][2] == model.HL.count then
    tail[#tail] = nil
    tail_text = joined()
  end
  local pad = "  "
  if width then
    local room = width - vim.fn.strdisplaywidth(head) - vim.fn.strdisplaywidth(tail_text) - 2
    if vim.fn.strdisplaywidth(name) > room and room > 1 then
      name = vim.fn.strcharpart(name, 0, room - 1) .. "…"
    end
    pad = string.rep(" ", math.max(2, room - vim.fn.strdisplaywidth(name) + 2))
  end
  local text = head .. name .. (tail_text ~= "" and (pad .. tail_text) or "")
  local marks = {
    { col = 1, stop = 1 + #model.ICONS[kind], hl = model.HL[kind] },
    { col = #head, stop = #head + #name, hl = model.HL[kind] },
  }
  local col = #head + #name + #pad
  for _, t in ipairs(tail) do
    marks[#marks + 1] = { col = col, stop = col + #t[1], hl = t[2] }
    col = col + #t[1] + 1
  end
  return { id = id, text = text, marks = marks }
end

--- The rows of a list of ids.
function M.rows(ix, ids, relative, width)
  return vim.tbl_map(function(id)
    return M.row(ix, id, relative, width)
  end, ids)
end

--- Everything the three panes show for a navigation; `widths` per pane.
---@param ix cogitator.column_dialog.Index
---@param nav cogitator.column_dialog.Nav
---@param widths {left?: integer, center?: integer, right?: integer}?
---@return {left: cogitator.column_dialog.Row[], center: cogitator.column_dialog.Row[], right: cogitator.column_dialog.Row[], current: string?}
function M.panes(ix, nav, widths)
  widths = widths or {}
  local here = model.current(ix, nav)
  return {
    left = M.rows(ix, model.neighbours(ix, here, "left"), here, widths.left),
    center = M.rows(ix, model.visible(ix, nav), nil, widths.center),
    right = M.rows(ix, model.neighbours(ix, here, "right"), here, widths.right),
    current = here,
  }
end

--- The middle pane's title: what it lists.
---@param ix cogitator.column_dialog.Index
---@param nav cogitator.column_dialog.Nav
---@return string
function M.title(ix, nav)
  local l = nav.list
  local what
  if l.parents then
    what = "feeds " .. (ix.nodes[l.parents] and ix.nodes[l.parents].name or l.parents)
  elseif l.children then
    what = "reads " .. (ix.nodes[l.children] and ix.nodes[l.children].name or l.children)
  else
    what = "columns of " .. tostring(l.model)
  end
  local shown, all = #model.visible(ix, nav), #model.list_ids(ix, nav.list)
  local count = shown == all and tostring(all) or ("%d/%d"):format(shown, all)
  local filter = nav.filter ~= "" and ("  /" .. nav.filter) or ""
  return ("%s · %s%s"):format(what, count, filter)
end

return M
