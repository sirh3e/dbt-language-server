--- The explorer's two bottom panes, side by side: the model's dbt SQL
--- (its file, Jinja and all; the loaded buffer when there is one, so
--- unsaved edits show) and the SQL dbt compiled it to (dbt.compiledSql).
--- Pure render over the explorer state; the file is read in the
--- background and the pane redrawn when it arrives.

local ui = require("cogitator.ui")

local M = {}

local function show(s, pane, lines, filetype, title)
  local buf, win = s.bufs[pane], s.wins[pane]
  if not (buf and vim.api.nvim_buf_is_valid(buf) and win and vim.api.nvim_win_is_valid(win)) then
    return
  end
  local same = s.shown and s.shown[pane] == s.selected
  local top = same and vim.fn.getwininfo(win)[1].topline or 1
  ui.set_lines(buf, lines)
  if vim.bo[buf].filetype ~= filetype then
    vim.bo[buf].filetype = filetype
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
  show(
    s,
    "raw",
    lines or { "· loading ·" },
    "sql",
    "dbt SQL · " .. vim.fn.fnamemodify(vim.uri_to_fname(uri), ":t")
  )
end

--- The compiled SQL pane.
---@param s cogitator.explorer.State
function M.render_compiled(s)
  if not s.selected then
    return show(s, "compiled", {}, "", "compiled SQL")
  end
  local c = s.compiled
  if c == nil then
    return show(s, "compiled", { "· loading compiled SQL ·" }, "", "compiled SQL")
  end
  if type(c) == "table" and c.content and c.content ~= vim.NIL then
    return show(s, "compiled", vim.split(c.content, "\n"), "sql", "compiled SQL")
  end
  if type(c) == "table" and c.uri and c.uri ~= vim.NIL then
    return show(s, "compiled", {
      "the compiled file is too large to show here — open it directly:",
      vim.uri_to_fname(c.uri),
    }, "", "compiled SQL")
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
