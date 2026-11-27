--- The dialog's live reflection: the column under the cursor in its
--- model's dbt SQL (or a source's YAML) and in the compiled SQL, each
--- pane scrolled to the column and the column marked; and the same mark
--- in the user's own buffer when that file is open. Files are read in the
--- background (cogitator.pick.preview) and cached per file; the compiled
--- SQL comes from the node's compiledUri or dbt.compiledSql.

local actions = require("cogitator.actions")
local locate = require("cogitator.column_dialog.locate")
local tshl = require("cogitator.tshl")
local ui = require("cogitator.ui")

local M = {}

M.NS = vim.api.nvim_create_namespace("cogitator.column_dialog.hit")
vim.api.nvim_set_hl(0, "DbtColumnHit", { link = "IncSearch", default = true })
vim.api.nvim_set_hl(0, "DbtColumnHitLine", { link = "CursorLine", default = true })

local function valid_uri(uri)
  return type(uri) == "string" and uri ~= "" and uri
end

--- The lines of `uri`: an open buffer's (unsaved edits show), else the
--- cached read, else start the read and answer nil.
---@param s table the dialog state
---@param uri string
---@param again fun() redraw once the read lands
---@return string[]?
local function file_lines(s, uri, again)
  local fname = vim.uri_to_fname(uri)
  local bufnr = vim.fn.bufnr(fname)
  if bufnr ~= -1 and vim.api.nvim_buf_is_loaded(bufnr) then
    return vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)
  end
  local cached = s.files[uri]
  if cached ~= nil then
    return cached or nil
  end
  s.files[uri] = false
  require("cogitator.pick.preview").read(fname, function(lines)
    s.files[uri] = lines or { "· cannot read " .. fname .. " ·" }
    again()
  end)
  return nil
end

--- The compiled SQL of a node: its compiledUri's file, else
--- dbt.compiledSql's answer for its model (its content, or the file it
--- names when too large to send); nil while loading, false for none.
local function compiled_lines(s, node, again)
  local uri = valid_uri(node.compiledUri) or (node.model and s.compiled_uri[node.model])
  if uri then
    return file_lines(s, uri, again)
  end
  local model = node.model
  if not model then
    return false
  end
  if s.compiled[model] ~= nil then
    return s.compiled[model]
  end
  if s.compiled_asked[model] then
    return nil
  end
  s.compiled_asked[model] = true
  actions.exec_with(s.client, s.src_buf, "dbt.compiledSql", { uri = "", model = model }, function(err, result)
    if not err and type(result) == "table" and valid_uri(result.uri) then
      s.compiled_file[model] = result.uri
    end
    if err or type(result) ~= "table" then
      s.compiled[model] = false
    elseif result.content and result.content ~= vim.NIL then
      s.compiled[model] = vim.split(result.content, "\n", { plain = true })
    elseif valid_uri(result.uri) then
      s.compiled_uri[model] = result.uri
    else
      s.compiled[model] = false
    end
    again()
  end)
  return nil
end

--- Mark a spot in a buffer: the line's background and the name itself.
local function mark(buf, spot)
  vim.api.nvim_buf_clear_namespace(buf, M.NS, 0, -1)
  if not spot or spot.line > vim.api.nvim_buf_line_count(buf) then
    return
  end
  pcall(vim.api.nvim_buf_set_extmark, buf, M.NS, spot.line - 1, 0, {
    line_hl_group = "DbtColumnHitLine",
    sign_text = "▶",
    sign_hl_group = "DbtColumnHit",
    priority = 200,
  })
  pcall(vim.api.nvim_buf_set_extmark, buf, M.NS, spot.line - 1, spot.col, {
    end_col = spot.stop,
    hl_group = "DbtColumnHit",
    priority = 200,
  })
end

--- Show lines in a preview pane, coloured by the server, the spot marked
--- and centred.
local function show(s, pane, lines, title, code, spot)
  local buf, win = s.bufs[pane], s.wins[pane]
  if not (buf and vim.api.nvim_buf_is_valid(buf) and win and vim.api.nvim_win_is_valid(win)) then
    return
  end
  local text = table.concat(lines, "\n")
  if s.shown[pane] ~= text then
    s.shown[pane] = text
    ui.set_lines(buf, lines)
    tshl.clear(buf)
    vim.bo[buf].filetype = code == "yaml" and "yaml" or (code and "sql" or "")
    if code then
      tshl.paint(buf, { text = text, language = code }, { client = s.client })
    end
  end
  ui.retitle(win, " " .. title .. " ")
  mark(buf, spot)
  if spot then
    pcall(vim.api.nvim_win_set_cursor, win, { math.min(spot.line, #lines), spot.col })
    vim.api.nvim_win_call(win, function()
      vim.cmd("normal! zz")
    end)
  end
  s.spots[pane] = spot
end

--- Mark the column in the user's buffer of that file, if it is open.
local function mark_source_buffer(s, uri, spot)
  for buf in pairs(s.marked_bufs) do
    if vim.api.nvim_buf_is_valid(buf) then
      vim.api.nvim_buf_clear_namespace(buf, M.NS, 0, -1)
    end
  end
  s.marked_bufs = {}
  local bufnr = uri and vim.fn.bufnr(vim.uri_to_fname(uri)) or -1
  if spot and bufnr ~= -1 and vim.api.nvim_buf_is_loaded(bufnr) then
    mark(bufnr, spot)
    s.marked_bufs[bufnr] = true
  end
end

--- Redraw both previews for the node under the cursor.
---@param s table the dialog state
---@param node table?
---@param again fun()
function M.render(s, node, again)
  if not node then
    show(s, "raw", { "· nothing selected ·" }, "dbt SQL")
    show(s, "compiled", { "" }, "compiled SQL")
    return mark_source_buffer(s, nil, nil)
  end
  local column = require("cogitator.column_dialog.model").column_of(node)
  local uri = valid_uri(node.uri)
  if not uri then
    show(s, "raw", { "· no file on disk for " .. (node.name or node.id) .. " ·" }, "dbt SQL")
    mark_source_buffer(s, nil, nil)
  else
    local fname = vim.uri_to_fname(uri)
    local code = tshl.language_of(fname)
    local lines = file_lines(s, uri, again)
    if not lines then
      show(s, "raw", { "· loading ·" }, "dbt SQL")
    else
      local spot = locate.of_range(node.range) or locate.column(lines, column, code == "yaml")
      local label = (code == "yaml" and "YAML · " or "dbt SQL · ") .. vim.fn.fnamemodify(fname, ":t")
      show(s, "raw", lines, label .. (spot and (":" .. spot.line) or "  (column not found)"), code, spot)
      mark_source_buffer(s, uri, spot)
    end
  end
  if node.kind ~= "model" then
    return show(
      s,
      "compiled",
      { "· a " .. (node.kind or "?") .. " column has no compiled SQL ·" },
      "compiled SQL"
    )
  end
  local lines = compiled_lines(s, node, again)
  if lines == nil then
    return show(s, "compiled", { "· loading compiled SQL ·" }, "compiled SQL")
  elseif lines == false then
    return show(s, "compiled", { "not compiled yet — :DbtCompile, then come back" }, "compiled SQL · none")
  end
  local spot = locate.column(lines, column)
  show(
    s,
    "compiled",
    lines,
    "compiled SQL · "
      .. vim.fn.fnamemodify(node.model or "", ":t")
      .. (spot and (":" .. spot.line) or "  (column not found)"),
    "sql",
    spot
  )
end

--- Clear every mark the dialog put in the user's buffers.
---@param s table
function M.clear(s)
  mark_source_buffer(s, nil, nil)
end

return M
