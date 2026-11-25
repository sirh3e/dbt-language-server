--- The picker's pure parts: which items a filter keeps (fzf's syntax,
--- cogitator.pick.query) and how one row is drawn.

local M = {}

--- Does an item (or, for a group, any of its leaves) match the filter?
--- The filter is fzf's syntax (cogitator.pick.query): `stg cust` (every
--- term, letters in order), `'exact`, `^prefix`, `suffix$`, `!not`,
--- `a | b` — matched against the label and the detail.
---@param item cogitator.picker.Item
---@param filter string
---@return boolean
function M.matches(item, filter)
  local query = require("cogitator.pick.query")
  local groups = query.parse(filter)
  if #groups == 0 then
    return true
  end
  local function hit(it)
    return query.match(groups, it.label .. " " .. (it.detail or "")) ~= nil
  end
  local function walk(it)
    if hit(it) then
      return true
    end
    for _, child in ipairs(it.children or {}) do
      if walk(child) then
        return true
      end
    end
    return false
  end
  return walk(item)
end

--- The items of a level the filter keeps.
---@param items cogitator.picker.Item[]
---@param filter string
---@return cogitator.picker.Item[]
function M.visible(items, filter)
  return vim.tbl_filter(function(it)
    return M.matches(it, filter)
  end, items)
end

--- One list row: " icon label ··· detail ›", padded to `width` cells.
--- Returns the text and the byte ranges to highlight.
---@param item cogitator.picker.Item
---@param width integer
---@return string line
---@return {col: integer, stop: integer, hl: string}[] marks
function M.format_line(item, width)
  local icon = item.icon or (item.children and "▸" or "•")
  local arrow = item.children and " ›" or ""
  local detail = item.detail and (" " .. item.detail) or ""
  local head = " " .. icon .. " "
  local room = width - vim.fn.strdisplaywidth(head) - vim.fn.strdisplaywidth(detail .. arrow) - 1
  local label = item.label
  if vim.fn.strdisplaywidth(label) > room and room > 1 then
    label = vim.fn.strcharpart(label, 0, room - 1) .. "…"
  end
  local pad = math.max(1, room - vim.fn.strdisplaywidth(label) + 1)
  local line = head .. label .. string.rep(" ", pad) .. detail .. arrow
  local marks = {}
  local function mark(col, stop, hl)
    if hl and stop > col then
      marks[#marks + 1] = { col = col, stop = stop, hl = hl }
    end
  end
  mark(
    1,
    1 + #icon,
    item.disabled and "DbtPickerDisabled" or (item.icon_hl or (item.children and "DbtPickerGroup"))
  )
  local lstart = #head
  mark(
    lstart,
    lstart + #label,
    item.disabled and "DbtPickerDisabled" or (item.hl or (item.children and "DbtPickerGroup"))
  )
  local dstart = lstart + #label + pad
  mark(dstart, dstart + #detail, "DbtPickerDetail")
  mark(dstart + #detail, #line, "DbtPickerGroup")
  return line, marks
end

return M
