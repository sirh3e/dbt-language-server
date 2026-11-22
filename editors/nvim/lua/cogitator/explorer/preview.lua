--- The explorer's two bottom panes, side by side: the model's dbt SQL
--- (its file, Jinja and all; the loaded buffer when there is one, so
--- unsaved edits show) and the SQL dbt compiled it to (dbt.compiledSql).
--- Pure render over the explorer state; the file is read in the
--- background and the pane redrawn when it arrives. Both panes are
--- coloured by tree-sitter-dbt through the server (dbt.highlight).

local tshl = require("cogitator.tshl")
local ui = require("cogitator.ui")

local M = {}

--- Show `lines` in a pane. `code` names the grammar the server colours
--- them with ("dbt" for the model's file, "sql" for the compiled SQL):
--- tree-sitter-dbt's highlights, painted over Vim's SQL syntax once they
--- arrive (cogitator.tshl).
local function show(s, pane, lines, filetype, title, code)
  local buf, win = s.bufs[pane], s.wins[pane]
  if not (buf and vim.api.nvim_buf_is_valid(buf) and win and vim.api.nvim_win_is_valid(win)) then
    return
  end
  local same = s.shown and s.shown[pane] == s.selected
  local top = same and vim.fn.getwininfo(win)[1].topline or 1
  ui.set_lines(buf, lines)
  tshl.clear(buf)
  if vim.bo[buf].filetype ~= filetype then
    vim.bo[buf].filetype = filetype
  elseif vim.bo[buf].syntax == "OFF" then
    vim.bo[buf].syntax = filetype
  end
  if code then
    tshl.paint(buf, { text = table.concat(lines, "\n"), language = code }, { client = s.client })
  end
  ui.retitle(win, " " .. title .. " ")
  -- a refresh of the same model keeps where it was scrolled to
  vim.api.nvim_win_call(win, function()
    vim.fn.winrestview({ topline = math.min(top, #lines), lnum = math.min(top, #lines) })
  end)
  s.shown = s.shown or {}
  s.shown[pane] = s.selected
end

--- The file behind `uri`: the loaded buffer, else what the background read
--- brought; nil while it is being read.
local function file_lines(s, uri)
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
      M.render_raw(s)
    end
  end)
  return nil
end

--- The dbt SQL pane.
---@param s cogitator.explorer.State
function M.render_raw(s)
  if not s.selected then
    return show(s, "raw", {}, "", "dbt SQL")
  end
  local node = s.node_of[s.selected]
  local uri = s.info and s.info.uri ~= vim.NIL and s.info.uri or (node and node.uri ~= vim.NIL and node.uri)
  if not uri or uri == "" then
    return show(s, "raw", { s.info and "· no file on disk ·" or "· loading ·" }, "", "dbt SQL")
  end
  local lines = file_lines(s, uri)
  local fname = vim.uri_to_fname(uri)
  local language = tshl.language_of(fname)
  show(
    s,
    "raw",
    lines or { "· loading ·" },
    language == "yaml" and "yaml" or "sql",
    (language == "yaml" and "YAML · " or "dbt SQL · ") .. vim.fn.fnamemodify(fname, ":t"),
    lines and language or nil
  )
  -- a macro or a generic test in a file of many: start at its definition
  local range = s.info and s.info.kind ~= "model" and s.info.range
  local buf, win = s.bufs.raw, s.wins.raw
  if lines and range and range ~= vim.NIL and buf and vim.api.nvim_buf_is_valid(buf) then
    local from, to = range.start.line + 1, range["end"].line + 1
    tshl.mark_definition(buf, from, to)
    -- once per selection: later redraws keep where the reader scrolled
    if s.def_shown ~= s.selected and win and vim.api.nvim_win_is_valid(win) then
      s.def_shown = s.selected
      pcall(vim.api.nvim_win_set_cursor, win, { math.min(from, #lines), 0 })
      vim.api.nvim_win_call(win, function()
        vim.cmd("normal! zt")
      end)
    end
  elseif buf and vim.api.nvim_buf_is_valid(buf) then
    tshl.mark_definition(buf, nil)
  end
end

--- The compiled SQL pane.
---@param s cogitator.explorer.State
function M.render_compiled(s)
  if not s.selected then
    return show(s, "compiled", {}, "", "compiled SQL")
  end
  local c = s.compiled
  s.compiled_jumps = nil
  if type(c) == "table" and c.callers then
    -- a macro compiles into the models that call it
    local lines, jumps = { "a macro compiles into the models that call it (<CR> opens one):", "" }, {}
    for _, caller in ipairs(c.callers) do
      lines[#lines + 1] = ("  %s  [%s]"):format(caller.name, caller.kind or "?")
      jumps[#lines] = caller
    end
    if #c.callers == 0 then
      lines[#lines + 1] = "  · nothing calls it ·"
    end
    s.compiled_jumps = jumps
    return show(s, "compiled", lines, "", "compiled SQL · " .. #c.callers .. " caller(s)")
  end
  if c == nil then
    return show(s, "compiled", { "· loading compiled SQL ·" }, "", "compiled SQL")
  end
  if type(c) == "table" and c.content and c.content ~= vim.NIL then
    return show(s, "compiled", vim.split(c.content, "\n"), "sql", "compiled SQL", "sql")
  end
  if type(c) == "table" and c.uri and c.uri ~= vim.NIL then
    return show(s, "compiled", {
      "the compiled file is too large to show here — open it directly:",
      vim.uri_to_fname(c.uri),
    }, "", "compiled SQL")
  end
  local node_kind = s.info and s.info.kind
  if node_kind == "test" or node_kind == "analysis" then
    return show(s, "compiled", {
      "not compiled yet — dbt compile writes target/compiled/…/"
        .. (node_kind == "test" and "tests" or "analyses"),
    }, "", "compiled SQL · not compiled")
  end
  if node_kind and node_kind ~= "model" then
    return show(
      s,
      "compiled",
      { "· a " .. node_kind:gsub("_", " ") .. " has no compiled SQL ·" },
      "",
      "compiled SQL"
    )
  end
  show(s, "compiled", {
    "not compiled yet — c in the explorer or :DbtCompile",
    "(dbt writes target/compiled/…; the explorer never runs dbt itself)",
  }, "", "compiled SQL · not compiled")
end

--- Both bottom panes.
---@param s cogitator.explorer.State
function M.render(s)
  M.render_raw(s)
  M.render_compiled(s)
end

return M
