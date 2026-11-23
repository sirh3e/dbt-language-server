--- The explorer's middle pane: everything known about the selected node
--- on one card. A model: what it is, its description, tags, relation,
--- versions, documented columns and tests. A macro: its signature, who
--- calls it and what it calls. A test: what it tests, whether it is
--- compiled; a generic test, where YAML applies it (dbt.nodeInfo). Lines with a jump target (a test, a
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

--- A description on one line: its first paragraph with the whitespace
--- folded (a doc block is Markdown over many lines), and `…` when more
--- follows it.
---@param text string
---@return string
function M.summary(text)
  local trimmed = vim.trim((text or ""):gsub("\r", ""))
  local first, rest = trimmed:match("^(.-)\n%s*\n(.*)$")
  first = (first or trimmed):gsub("%s+", " ")
  if rest and vim.trim(rest) ~= "" then
    first = first .. " …"
  end
  return first
end

--- A macro's or generic test's signature: `name(a, b='x')`.
---@param name string
---@param params table[]|string[]
---@return string
function M.signature(name, params)
  local parts = {}
  for _, p in ipairs(params or {}) do
    if type(p) == "string" then
      parts[#parts + 1] = p
    else
      parts[#parts + 1] = p.name .. (present(p.default) and ("=" .. p.default) or "")
    end
  end
  return name .. "(" .. table.concat(parts, ", ") .. ")"
end

--- The card lines of a node that is not a model (dbt.nodeInfo's answer):
--- a macro, a singular or generic test, a source, an exposure…, appended
--- through `add` and `heading`.
---@param info table
---@param add fun(line: string, jump: table?)
---@param heading fun(text: string)
local function node_lines(info, add, heading)
  local function entries(title, list, empty)
    heading(("%s (%d)"):format(title, #(list or {})))
    if not list or #list == 0 then
      add("  · " .. empty .. " ·")
      return
    end
    for _, e in ipairs(list) do
      add(("  %s  [%s]"):format(e.name, e.kind or "?"), present(e.uri) and { uri = e.uri } or nil)
    end
  end
  local kind = info.kind
  if kind == "macro" then
    add("")
    add(M.signature(info.name, info.params))
    if present(info.package) then
      add("package: " .. info.package)
    end
    entries("called by", info.callers, "no model or test calls it")
    entries("calls", info.calls, "no other macro")
  elseif kind == "generic_test" then
    add("")
    add("{% test " .. M.signature(info.name, info.params) .. " %}")
    if present(info.package) then
      add("package: " .. info.package)
    end
    heading(("applied to (%d, <CR> jumps)"):format(#(info.appliedTo or {})))
    if not info.appliedTo or #info.appliedTo == 0 then
      add("  · no YAML applies it ·")
    end
    for _, a in ipairs(info.appliedTo or {}) do
      local where = a.model .. (present(a.column) and ("." .. a.column) or "")
      add("  " .. where, { uri = a.uri, range = a.range })
    end
  else
    entries(kind == "test" and "tests" or "upstream", info.refs or info.upstream, "nothing")
    if present(info.calls) and #info.calls > 0 then
      entries("calls", info.calls, "no macro")
    end
    if kind ~= "test" and kind ~= "analysis" then
      entries("downstream", info.downstream, "nothing depends on it")
    end
    if kind == "test" or kind == "analysis" then
      add("")
      add(present(info.compiledUri) and "compiled: yes — below" or "compiled: not yet (dbt compile)")
    end
  end
end

--- The card of a dbt.nodeInfo answer on its own (:DbtNode's float).
---@param info table
---@param width integer?
---@return cogitator.explorer.Card
function M.node(info, width)
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
  add(info.name or info.model or "?")
  headings[1] = 1
  local file = present(info.uri) and vim.fn.fnamemodify(vim.uri_to_fname(info.uri), ":~:.")
  add((info.kind or "model") .. (file and ("  ·  " .. file) or ""))
  if present(info.description) then
    add("")
    for _, l in ipairs(M.wrap(info.description, width)) do
      add(l)
    end
  end
  if info.kind == "model" then
    for _, t in ipairs({ { "upstream", info.upstream }, { "downstream", info.downstream } }) do
      heading(("%s (%d)"):format(t[1], #(t[2] or {})))
      for _, e in ipairs(t[2] or {}) do
        add("  " .. e.name, present(e.uri) and { uri = e.uri } or nil)
      end
    end
  else
    node_lines(info, add, heading)
  end
  return { lines = lines, jumps = jumps, headings = headings }
end

--- `text` wrapped to `width` cells.
---@param text string
---@param width integer?
---@return string[]
function M.wrap(text, width)
  local out, line = {}, ""
  local wrap = math.max(20, (width or 60) - 1)
  for word in text:gsub("%s+", " "):gmatch("%S+") do
    if line ~= "" and vim.fn.strdisplaywidth(line .. " " .. word) > wrap then
      out[#out + 1] = line
      line = word
    else
      line = line == "" and word or (line .. " " .. word)
    end
  end
  if line ~= "" then
    out[#out + 1] = line
  end
  return out
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
    for _, line in ipairs(M.wrap(info.description, width)) do
      add(line)
    end
  elseif not info then
    add("")
    add("· loading ·")
  end

  -- a macro, a test, a source…: dbt.nodeInfo's card
  if kind ~= "model" then
    if info then
      node_lines(info, add, heading)
    end
    return { lines = lines, jumps = jumps, headings = headings }
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
  elseif r.origin == "defaults" then
    -- a project's own generate_schema_name is not evaluated: say so
    -- rather than show a schema the model may not be built in
    add("relation: " .. r.relation .. "  (dbt's defaults; dbt compile renders the real one)")
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
      add(
        "  • "
          .. column.name
          .. (present(column.description) and (" — " .. M.summary(column.description)) or "")
      )
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
