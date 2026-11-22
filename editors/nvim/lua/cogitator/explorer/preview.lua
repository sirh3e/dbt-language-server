--- The explorer's bottom pane: renders one of five tabs for the
--- selected model — [1] the raw SQL, [2] the compiled SQL served by
--- dbt.compiledSql (a hint when dbt has not written one), [3] versions
--- and tests, [4] the info view (description/tags/relation/columns/
--- tests with jumps), [5] its documented columns. Pure render over the
--- explorer state; the raw file is read in the background.

local render_lib = require("cogitator.render")
local ui = require("cogitator.ui")

local M = {}

local function tab_title(tab)
  local names = { "raw", "compiled", "versions/tests", "info", "columns" }
  local parts = {}
  for i, name in ipairs(names) do
    local label = ("%d %s"):format(i, name)
    parts[i] = (i == tab) and ("[" .. label .. "]") or label
  end
  return " " .. table.concat(parts, "  ") .. " "
end

--- A file's lines: the loaded buffer when there is one (unsaved edits
--- win), else what was read of the file in the background; nil while it
--- is being read (the pane re-renders when it arrives).
local function read_uri_lines(s, uri)
  if not uri or uri == vim.NIL then
    return { "· no file on disk ·" }
  end
  local fname = vim.uri_to_fname(uri)
  local bufnr = vim.fn.bufnr(fname)
  if bufnr ~= -1 and vim.api.nvim_buf_is_loaded(bufnr) then
    return vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)
  end
  if s.raw and s.raw.uri == uri then
    return s.raw.lines
  end
  s.raw = { uri = uri }
  require("cogitator.pick.preview").read(fname, function(lines)
    if s.raw and s.raw.uri == uri then
      s.raw.lines = lines or { "· cannot read " .. fname .. " ·" }
      M.render(s)
    end
  end)
  return nil
end

--- The selection's documented columns, from the project graph.
local function columns_of(s)
  for _, n in ipairs(s.nodes) do
    if n.name == s.selected then
      if not n.columns or #n.columns == 0 then
        return { "· no documented columns (columns: in its YAML) ·" }
      end
      local lines = {}
      for _, c in ipairs(n.columns) do
        local desc = c.description
        lines[#lines + 1] = c.name .. ((desc and desc ~= vim.NIL and desc ~= "") and ("  — " .. desc) or "")
      end
      return lines
    end
  end
  return { "· not in the project graph ·" }
end

local function versions_and_tests(info)
  local lines = {}
  local v = info.versions
  if v and v ~= vim.NIL then
    local latest = (v.latestVersion ~= vim.NIL) and tostring(v.latestVersion) or "?"
    lines[#lines + 1] = "latest_version: " .. latest
    for _, item in ipairs(v.versions) do
      local file = "(no file)"
      if item.uri and item.uri ~= vim.NIL then
        file = vim.fn.fnamemodify(vim.uri_to_fname(item.uri), ":t")
      end
      lines[#lines + 1] = ("  v%d → %s"):format(item.v, file)
    end
  else
    lines[#lines + 1] = "versions: · none ·"
  end
  lines[#lines + 1] = ""
  lines[#lines + 1] = "tests:"
  if not info.tests or #info.tests == 0 then
    lines[#lines + 1] = "  · none ·"
  else
    for _, t in ipairs(info.tests) do
      lines[#lines + 1] = "  " .. render_lib.test_label(t)
    end
  end
  return lines
end

--- Render the preview pane from the browser state (selection, info, tab).
function M.render(s)
  local buf, win = s.bufs.preview, s.wins.preview
  if not (buf and vim.api.nvim_buf_is_valid(buf) and vim.api.nvim_win_is_valid(win)) then
    return
  end
  local lines, filetype = { "· loading ·" }, ""
  if not s.selected then
    lines = { "· no model selected ·" }
  elseif s.info then
    if s.tab == 1 then
      lines = read_uri_lines(s, s.info.uri) or { "· loading ·" }
      filetype = "sql"
    elseif s.tab == 2 then
      filetype = "sql"
      local c = s.compiled
      if c == nil then
        lines = { "· loading compiled SQL ·" }
      elseif type(c) == "table" and c.content and c.content ~= vim.NIL then
        lines = vim.split(c.content, "\n")
      elseif type(c) == "table" and c.uri and c.uri ~= vim.NIL then
        lines = { "compiled file too large for the preview — open it directly:", vim.uri_to_fname(c.uri) }
      else
        lines = { "not compiled — run :DbtCompile (dbt writes target/compiled/…)" }
      end
    elseif s.tab == 3 then
      lines = versions_and_tests(s.info)
    elseif s.tab == 5 then
      lines = columns_of(s)
    else
      local view = require("cogitator.explorer.info").view(s)
      lines = view.lines
      s.info_jumps = view.jumps
    end
  end
  ui.set_lines(buf, lines)
  if vim.bo[buf].filetype ~= filetype then
    vim.bo[buf].filetype = filetype
  end
  vim.api.nvim_win_set_config(win, { title = tab_title(s.tab), title_pos = "center" })
  vim.api.nvim_win_set_cursor(win, { 1, 0 })
end

return M
