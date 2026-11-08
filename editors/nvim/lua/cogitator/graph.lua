--- Model-graph browser (dbt.modelGraph): a yazi-style three-pane float —
--- upstream models on the left, the current model in the center, downstream
--- models on the right. h/l enter a side pane, and pressed again there walk
--- further along the lineage. <CR> re-centers on a neighbor, o opens its
--- file, y yanks its name, q/<Esc> closes. Float/label plumbing lives in
--- cogitator.ui / cogitator.render / cogitator.actions.

local actions = require("cogitator.actions")
local motion = require("cogitator.motion")
local render_lib = require("cogitator.render")
local ui = require("cogitator.ui")

local M = {}

---@class cogitator.graph.State
---@field client vim.lsp.Client
---@field src_buf integer
---@field wins table<string, integer>
---@field bufs table<string, integer>
---@field data table? the dbt.modelGraph payload
---@field loading boolean? a re-center is in flight
---@field pending {pane: "left"|"right", count: integer}? walk steps waiting on it
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
  render_lib.fill_pane(s.bufs.left, data.upstream)
  ui.set_lines(s.bufs.center, {
    data.model,
    "",
    path,
    "",
    ("%d upstream · %d downstream"):format(#data.upstream, #data.downstream),
  })
  render_lib.fill_pane(s.bufs.right, data.downstream)
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
---@param on_fail fun()? called when no result arrives
local function fetch(by, on_result, on_fail)
  local s = M._state
  if not s or not s.client or s.client:is_stopped() then
    vim.notify("cogitator client is gone; close and reopen the graph", vim.log.levels.ERROR)
    return
  end
  actions.exec_with(s.client, s.src_buf, "dbt.modelGraph", { uri = by.uri or "", model = by.model }, function(err, result)
    if err then
      vim.notify("dbt.modelGraph failed: " .. err.message, vim.log.levels.ERROR)
      if on_fail then
        on_fail()
      end
      return
    end
    if not result or result == vim.NIL then
      vim.notify(
        (by.model or "this model") .. " is not in the project graph (not a model file?)",
        vim.log.levels.WARN
      )
      if on_fail then
        on_fail()
      end
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

--- Make a side entry the center, `count` times over, walking the lineage.
--- A step waits for the previous one's dbt.modelGraph answer.
---@param pane "left"|"right"
---@param count integer
local function recenter(pane, count)
  local s = M._state
  if s.loading then
    s.pending = { pane = pane, count = ((s.pending and s.pending.pane == pane) and s.pending.count or 0) + count }
    return
  end
  local e = selected_entry(pane)
  if not e then
    return
  end
  if e.kind ~= "model" then
    vim.notify(("%s is a %s — no further lineage behind it"):format(e.name, e.kind:gsub("_", " ")), vim.log.levels.INFO)
    return
  end
  s.loading = true
  fetch({ model = e.name }, function(data)
    local st = M._state
    if not st then
      return
    end
    st.loading = false
    render(data)
    local queued = (st.pending and st.pending.pane == pane) and st.pending.count or 0
    st.pending = nil
    if count - 1 + queued > 0 then
      recenter(pane, count - 1 + queued)
    end
  end, function()
    if M._state then
      M._state.loading, M._state.pending = false, nil
    end
  end)
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
    motion.map(buf, lhs, fn)
  end
  local function focus(name)
    return function()
      vim.api.nvim_set_current_win(s.wins[name])
    end
  end
  local function walker(side)
    return function()
      recenter(side, vim.v.count1)
    end
  end
  map("q", close)
  map("<Esc>", close)
  if pane == "left" then
    map("h", walker("left"))
    map("l", focus("center"))
  elseif pane == "right" then
    map("h", focus("center"))
    map("l", walker("right"))
  else
    map("h", focus("left"))
    map("l", focus("right"))
  end
  if pane == "center" then
    local open_center = function()
      open_entry(M._state.data.uri)
    end
    map("<CR>", open_center)
    map("o", open_center)
  else
    map("<CR>", function()
      recenter(pane, 1)
    end)
    map("y", function()
      local e = selected_entry(pane)
      if e then
        vim.fn.setreg(vim.v.register, e.name)
      end
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
