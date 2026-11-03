--- The explorer's info tab: everything known about the selection on one
--- pane — description, tags, relation name, documented columns, and
--- tests whose lines jump to their YAML definition on <CR>.

local render_lib = require("cogitator.render")

local M = {}

---@class cogitator.explorer.InfoView
---@field lines string[]
---@field jumps table<integer, {uri: string, range?: table}> 1-based line → jump target

--- Build the info tab's lines and jump table from the explorer state.
---@param s cogitator.explorer.State
---@return cogitator.explorer.InfoView
function M.view(s)
  local info = s.info
  if not info then
    return { lines = { "· loading ·" }, jumps = {} }
  end
  local lines, jumps = {}, {}
  local function add(line, jump)
    lines[#lines + 1] = line
    if jump then
      jumps[#lines] = jump
    end
  end

  add(info.model)
  local described = info.description and info.description ~= vim.NIL and info.description ~= ""
  add(described and info.description or "· no description ·")
  local tags = {}
  if info.tags and info.tags ~= vim.NIL then
    for _, tag in ipairs(info.tags) do
      tags[#tags + 1] = "#" .. tag
    end
  end
  add(#tags > 0 and table.concat(tags, " ") or "tags: · none ·")

  local r = s.relation
  if r == nil then
    add("relation: · loading ·")
  elseif r == false or not r.relation then
    add("relation: · unresolved ·")
  else
    add("relation: " .. r.relation)
  end

  add("")
  add("columns:")
  local node
  for _, n in ipairs(s.nodes) do
    if n.name == info.model then
      node = n
      break
    end
  end
  if node and node.columns and node.columns ~= vim.NIL and #node.columns > 0 then
    for _, column in ipairs(node.columns) do
      local line = "  • " .. column.name
      if column.description and column.description ~= vim.NIL and column.description ~= "" then
        line = line .. " — " .. column.description
      end
      add(line)
    end
  else
    add("  · none documented ·")
  end

  add("")
  add("tests (<CR> jumps):")
  if info.tests and info.tests ~= vim.NIL and #info.tests > 0 then
    for _, t in ipairs(info.tests) do
      add("  " .. render_lib.test_label(t), { uri = t.uri, range = t.range })
    end
  else
    add("  · none ·")
  end
  return { lines = lines, jumps = jumps }
end

return M
