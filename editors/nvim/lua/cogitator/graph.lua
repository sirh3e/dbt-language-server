--- Model-graph browser (dbt.modelGraph): a yazi-style three-pane float —
--- upstream models on the left, the current model in the center, downstream
--- models on the right. h/l move focus, <CR> re-centers on a neighbor,
--- o opens its file, q/<Esc> closes. Float/label plumbing lives in
--- cogitator.ui / cogitator.render / cogitator.actions.

local actions = require("cogitator.actions")
local render_lib = require("cogitator.render")
local ui = require("cogitator.ui")

local M = {}

---@class cogitator.graph.State
---@field client vim.lsp.Client
---@field src_buf integer
---@field wins table<string, integer>
---@field bufs table<string, integer>
---@field data table? the dbt.modelGraph payload
---@type cogitator.graph.State?
M._state = nil

local function close()
  local s = M._state
  if not s then
    return
  end
  M._state = nil
  ui.close_state(s)
end

--- Fill all three panes from a dbt.modelGraph payload.
---@param data table
local function render(data)
  local s = M._state
  s.data = data
  local path = (data.uri and data.uri ~= vim.NIL) and vim.fn.fnamemodify(vim.uri_to_fname(data.uri), ":~:.")
    or ""
  ui.set_lines(s.bufs.left, render_lib.pane_lines(data.upstream))
  ui.set_lines(s.bufs.center, {
    data.model,
    "",
    path,
    "",
    ("%d upstream · %d downstream"):format(#data.upstream, #data.downstream),
  })
  ui.set_lines(s.bufs.right, render_lib.pane_lines(data.downstream))
  ui.retitle(s.wins.center, " " .. data.model .. " ")
  for _, pane in ipairs({ "left", "right" }) do
    if vim.api.nvim_win_is_valid(s.wins[pane]) then
      vim.api.nvim_win_set_cursor(s.wins[pane], { 1, 0 })
    end
  end
end

--- dbt.modelGraph through the originating client; `by` is {uri=...} for the
--- initial view or {model=...} when re-centering on a neighbor.
---@param by {uri?: string, model?: string}
---@param on_result fun(data: table)
local function fetch(by, on_result)
  local s = M._state
  if not s or not s.client or s.client:is_stopped() then
    vim.notify("cogitator client is gone; close and reopen the graph", vim.log.levels.ERROR)
    return
  end
  actions.exec_with(s.client, s.src_buf, "dbt.modelGraph", { uri = by.uri or "", model = by.model }, function(err, result)
    if err then
      vim.notify("dbt.modelGraph failed: " .. err.message, vim.log.levels.ERROR)
      return
    end
    if not result or result == vim.NIL then
      vim.notify(
        (by.model or "this model") .. " is not in the project graph (not a model file?)",
        vim.log.levels.WARN
      )
      return
    end
    on_result(result)
  end)
end

--- The entry under the cursor in a side pane.
---@param pane "left"|"right"
---@return cogitator.GraphEntry?
local function selected_entry(pane)
  local s = M._state
  local entries = pane == "left" and s.data.upstream or s.data.downstream
  if #entries == 0 then
    return nil
  end
  return entries[vim.api.nvim_win_get_cursor(s.wins[pane])[1]]
end

--- <CR> on a side entry: make that model the center.
---@param pane "left"|"right"
local function recenter(pane)
  local e = selected_entry(pane)
  if not e then
    return
  end
  if e.kind ~= "model" then
    vim.notify(e.name .. " is a source — no further lineage behind it", vim.log.levels.INFO)
    return
  end
  fetch({ model = e.name }, render)
end

--- Close the view and open the entry's file (if it has one).
---@param uri string|vim.NIL|nil
local function open_entry(uri)
  if not uri or uri == vim.NIL then
    vim.notify("no file on disk for this entry", vim.log.levels.WARN)
    return
  end
  close()
  actions.open_uri(uri)
end

---@param pane "left"|"center"|"right"
---@param buf integer
local function keymaps(pane, buf)
  local s = M._state
  local function map(lhs, fn)
    vim.keymap.set("n", lhs, fn, { buffer = buf, nowait = true })
  end
  map("q", close)
  map("<Esc>", close)
  map("h", function()
    vim.api.nvim_set_current_win(s.wins[pane == "right" and "center" or "left"])
  end)
  map("l", function()
    vim.api.nvim_set_current_win(s.wins[pane == "left" and "center" or "right"])
  end)
  if pane == "center" then
    local open_center = function()
      open_entry(M._state.data.uri)
    end
    map("<CR>", open_center)
    map("o", open_center)
  else
    map("<CR>", function()
      recenter(pane)
    end)
    map("o", function()
      local e = selected_entry(pane)
      if e then
        open_entry(e.uri)
      end
    end)
  end
end

---@param pane "left"|"center"|"right"
local function float(pane, col, width, height, title)
  local s = M._state
  local buf, win = ui.float({
    row = math.max(1, math.floor((vim.o.lines - height) / 2) - 1),
    col = col,
    width = width,
    height = height,
    title = title,
    cursorline = pane ~= "center",
  })
  s.bufs[pane], s.wins[pane] = buf, win
  keymaps(pane, buf)
end

--- Open the three-pane graph for the current buffer's model.
function M.open()
  local src_buf = vim.api.nvim_get_current_buf()
  local client = actions.client_for(src_buf)
  if not client then
    return
  end
  close()
  M._state = { client = client, src_buf = src_buf, wins = {}, bufs = {}, data = nil }

  fetch({ uri = vim.uri_from_bufnr(src_buf) }, function(data)
    if not M._state then
      return
    end
    local total = math.min(vim.o.columns - 4, 110)
    local width = math.floor((total - 4) / 3)
    local height = math.max(7, math.min(vim.o.lines - 6, math.max(#data.upstream, #data.downstream) + 2))
    local col = math.max(0, math.floor((vim.o.columns - total) / 2))
    float("left", col, width, height, "upstream ⯇")
    float("center", col + width + 2, width, height, data.model)
    float("right", col + 2 * (width + 2), width, height, "⯈ downstream")
    render(data)
    vim.api.nvim_set_current_win(M._state.wins.center)
  end)
end

M.close = close

return M
