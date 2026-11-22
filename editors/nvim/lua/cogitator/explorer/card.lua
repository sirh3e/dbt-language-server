--- The explorer's middle pane: everything known about the selected model
--- on one card — what it is, its description, tags, relation, versions,
--- documented columns and tests. Lines with a jump target (a test, a
--- version) open it on <CR>. Pure: lines and jumps from the state.

local render_lib = require("cogitator.render")

local M = {}

---@class cogitator.explorer.Card
---@field lines string[]
---@field jumps table<integer, {uri: string, range?: table}> 1-based line → jump target
---@field headings integer[] 1-based lines that start a section

local function present(v)
  return v ~= nil and v ~= vim.NIL and v ~= ""
end

--- The card for the state's selection.
---@param s cogitator.explorer.State
---@param width integer? wrap the description to this many cells
---@return cogitator.explorer.Card
function M.view(s, width)
  local lines, jumps, headings = {}, {}, {}
  local function add(line, jump)
    lines[#lines + 1] = line
    if jump then
      jumps[#lines] = jump
    end
  end
  local function heading(text)
    add("")
    add(text)
    headings[#headings + 1] = #lines
  end
  if not s.selected then
    return { lines = { "· no model selected — f or / to pick one ·" }, jumps = {}, headings = {} }
  end
  local node = s.node_of[s.selected]
  local info = s.info

  add(s.selected .. (s.marked[s.selected] and "  ● marked" or ""))
  headings[1] = 1
  local kind = node and node.kind or "model"
  local file = info and present(info.uri) and vim.fn.fnamemodify(vim.uri_to_fname(info.uri), ":~:.")
    or (node and present(node.uri) and vim.fn.fnamemodify(vim.uri_to_fname(node.uri), ":~:."))
  add(kind .. (file and ("  ·  " .. file) or ""))

  if info and present(info.description) then
    add("")
    local text = info.description:gsub("%s+", " ")
    local wrap = math.max(20, (width or 60) - 1)
    local line = ""
    for word in text:gmatch("%S+") do
      if line ~= "" and vim.fn.strdisplaywidth(line .. " " .. word) > wrap then
        add(line)
        line = word
      else
        line = line == "" and word or (line .. " " .. word)
      end
    end
    if line ~= "" then
      add(line)
    end
  elseif not info then
    add("")
    add("· loading ·")
  end

  local tags = {}
  for _, tag in ipairs((info and present(info.tags) and info.tags) or (node and node.tags) or {}) do
    tags[#tags + 1] = "#" .. tag
  end
  if #tags > 0 then
    add("")
    add(table.concat(tags, " "))
  end

  local r = s.relation
  add("")
  if r == nil then
    add("relation: · loading ·")
  elseif r == false or not r.relation then
    add("relation: · unresolved ·")
  else
    add("relation: " .. r.relation)
  end

  local v = info and info.versions
  if present(v) then
    heading(("versions (latest %s)"):format(present(v.latestVersion) and tostring(v.latestVersion) or "?"))
    for _, item in ipairs(v.versions or {}) do
      local file_name = present(item.uri) and vim.fn.fnamemodify(vim.uri_to_fname(item.uri), ":t")
        or "(no file)"
      add(("  v%d → %s"):format(item.v, file_name), present(item.uri) and { uri = item.uri } or nil)
    end
  end

  heading("columns")
  if node and node.columns and #node.columns > 0 then
    for _, column in ipairs(node.columns) do
      add("  • " .. column.name .. (present(column.description) and (" — " .. column.description) or ""))
    end
  else
    add("  · none documented ·")
  end

  heading("tests (<CR> jumps)")
  if info and present(info.tests) and #info.tests > 0 then
    for _, t in ipairs(info.tests) do
      add("  " .. render_lib.test_label(t), { uri = t.uri, range = t.range })
    end
  else
    add(info and "  · none ·" or "  · loading ·")
  end
  return { lines = lines, jumps = jumps, headings = headings }
end

return M
