--- The :DbtBrowse bottom pane: renders one of three tabs for the selected
--- model — [1] the raw SQL, [2] the compiled SQL served by dbt.compiledSql
--- (a hint when dbt has not written one), [3] versions and tests. Pure
--- render over the browser's state; browse.lua owns that state.

local M = {}

local function tab_title(tab)
  local names = { "raw", "compiled", "versions/tests" }
  local parts = {}
  for i, name in ipairs(names) do
    local label = ("%d %s"):format(i, name)
    parts[i] = (i == tab) and ("[" .. label .. "]") or label
  end
  return " " .. table.concat(parts, "  ") .. " "
end

--- A file's lines: the loaded buffer when there is one (unsaved edits win),
--- else the file on disk.
local function read_uri_lines(uri)
  if not uri or uri == vim.NIL then
    return nil
  end
  local fname = vim.uri_to_fname(uri)
  local bufnr = vim.fn.bufnr(fname)
  if bufnr ~= -1 and vim.api.nvim_buf_is_loaded(bufnr) then
    return vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)
  end
  if vim.fn.filereadable(fname) == 1 then
    return vim.fn.readfile(fname)
  end
  return nil
end

local function test_label(t)
  local label = t.name
  if t.column and t.column ~= vim.NIL then
    label = label .. " on " .. t.column
  end
  if t.version and t.version ~= vim.NIL then
    label = label .. (" (v%d)"):format(t.version)
  end
  return "  " .. label
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
      lines[#lines + 1] = test_label(t)
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
      lines = read_uri_lines(s.info.uri) or { "· no file on disk ·" }
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
    else
      lines = versions_and_tests(s.info)
    end
  end
  vim.bo[buf].modifiable = true
  vim.api.nvim_buf_set_lines(buf, 0, -1, false, lines)
  vim.bo[buf].modifiable = false
  if vim.bo[buf].filetype ~= filetype then
    vim.bo[buf].filetype = filetype
  end
  vim.api.nvim_win_set_config(win, { title = tab_title(s.tab), title_pos = "center" })
  vim.api.nvim_win_set_cursor(win, { 1, 0 })
end

return M
