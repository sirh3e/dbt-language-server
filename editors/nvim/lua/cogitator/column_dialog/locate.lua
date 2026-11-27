--- Where a column is written in a file, for the dialog's previews and for
--- <CR>: pure over the file's lines. The server's `range` wins when it
--- sends one; this is the reading for when it does not.
---
--- In SQL the column's own definition is what a reader looks for: an
--- `as <column>` alias, else a select-list item naming it, else any
--- mention. dbt models end in their final select, so the last alias or item
--- wins (and the first mention). In YAML it is the `- name: <column>` entry.

local M = {}

---@class cogitator.column_dialog.Spot
---@field line integer 1-based
---@field col integer 0-based byte where the column's name starts
---@field stop integer 0-based byte where it ends (exclusive)

--- Every place `column` is a whole word in `line` (case-insensitive),
--- as {start, stop} 1-based inclusive byte pairs.
local function words(line, column)
  local hay, needle = line:lower(), vim.pesc(column:lower())
  local out, from = {}, 1
  while true do
    local s, e = hay:find("%f[%w_]" .. needle .. "%f[^%w_]", from)
    if not s then
      return out
    end
    out[#out + 1] = { s, e }
    from = e + 1
  end
end

local function spot(i, pair)
  return { line = i, col = pair[1] - 1, stop = pair[2] }
end

--- The line of a SQL comment (`--`) or a Jinja comment start is skipped.
local function comment(line)
  return line:match("^%s*%-%-") ~= nil or line:match("^%s*{#") ~= nil
end

--- Where `column` is defined in `lines`.
---@param lines string[]
---@param column string
---@param yaml boolean? the file is YAML (a source's or a model's properties)
---@return cogitator.column_dialog.Spot?
function M.column(lines, column, yaml)
  if not column or column == "" then
    return nil
  end
  local needle = vim.pesc(column:lower())
  if yaml then
    for i, line in ipairs(lines) do
      local lower = line:lower()
      if lower:match("^%s*%-?%s*name:%s*[\"']?" .. needle .. "[\"']?%s*$") then
        local s = lower:find(needle, lower:find("name:", 1, true), true)
        return { line = i, col = s - 1, stop = s - 1 + #column }
      end
    end
  end
  local alias, item, any
  for i, line in ipairs(lines) do
    if not comment(line) then
      for _, pair in ipairs(words(line, column)) do
        local before = line:sub(1, pair[1] - 1):lower()
        local after = line:sub(pair[2] + 1)
        if before:match('%f[%w_]as%s+["`]?$') then
          alias = spot(i, pair)
        elseif
          before:match('^%s*,?%s*[%w_]*%.?["`]?$')
          and (after:match('^["`]?%s*,?%s*$') or after:match('^["`]?%s*,?%s*%-%-'))
        then
          item = spot(i, pair)
        end
        any = any or spot(i, pair)
      end
    end
  end
  return alias or item or any
end

--- The spot of a server range ({start = {line, character}}), on its line.
---@param range table?
---@return cogitator.column_dialog.Spot?
function M.of_range(range)
  if type(range) ~= "table" or type(range.start) ~= "table" then
    return nil
  end
  local stop = range["end"] and range["end"].line == range.start.line and range["end"].character
    or range.start.character
  return { line = range.start.line + 1, col = range.start.character, stop = stop }
end

return M
