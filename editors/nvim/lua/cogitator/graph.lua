--- Model-graph browser (dbt.modelGraph): a yazi-style three-pane float —
--- upstream models on the left, the current model in the center, downstream
--- models on the right. h/l move focus, <CR> re-centers on a neighbor,
--- o opens its file, q/<Esc> closes.

local M = {}

M._state = nil

local function close()
  local s = M._state
  if not s then
    return
  end
  M._state = nil
  for _, win in pairs(s.wins) do
    if vim.api.nvim_win_is_valid(win) then
      vim.api.nvim_win_close(win, true)
    end
  end
  for _, buf in pairs(s.bufs) do
    if vim.api.nvim_buf_is_valid(buf) then
      vim.api.nvim_buf_delete(buf, { force = true })
    end
  end
end

local function entry_label(e)
  local label = e.name
  if e.kind == "source" then
    label = label .. "  [source]"
  end
  if not e.uri then
    label = label .. "  (missing)"
  end
  return label
end

local function pane_lines(entries)
  if #entries == 0 then
    return { "· none ·" }
  end
  return vim.tbl_map(entry_label, entries)
end

local function set_lines(buf, lines)
  vim.bo[buf].modifiable = true
  vim.api.nvim_buf_set_lines(buf, 0, -1, false, lines)
  vim.bo[buf].modifiable = false
end

local function render(data)
  local s = M._state
  s.data = data
  local path = data.uri and vim.fn.fnamemodify(vim.uri_to_fname(data.uri), ":~:.") or ""
  set_lines(s.bufs.left, pane_lines(data.upstream))
  set_lines(s.bufs.center, {
    data.model,
    "",
    path,
    "",
    ("%d upstream · %d downstream"):format(#data.upstream, #data.downstream),
  })
  set_lines(s.bufs.right, pane_lines(data.downstream))
  vim.api.nvim_win_set_config(s.wins.center, { title = " " .. data.model .. " ", title_pos = "center" })
  for _, pane in ipairs({ "left", "right" }) do
    if vim.api.nvim_win_is_valid(s.wins[pane]) then
      vim.api.nvim_win_set_cursor(s.wins[pane], { 1, 0 })
    end
  end
end

--- dbt.modelGraph through the originating client; `by` is {uri=...} for the
--- initial view or {model=...} when re-centering on a neighbor.
local function fetch(by, on_result)
  local s = M._state
  if not s or not s.client or s.client:is_stopped() then
    vim.notify("cogitator client is gone; close and reopen the graph", vim.log.levels.ERROR)
    return
  end
  s.client:exec_cmd({
    command = "dbt.modelGraph",
    arguments = { { uri = by.uri or "", model = by.model } },
  }, { bufnr = s.src_buf }, function(err, result)
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

local function selected_entry(pane)
  local s = M._state
  local entries = pane == "left" and s.data.upstream or s.data.downstream
  if #entries == 0 then
    return nil
  end
  return entries[vim.api.nvim_win_get_cursor(s.wins[pane])[1]]
end

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

local function open_entry(uri)
  if not uri then
    vim.notify("no file on disk for this entry", vim.log.levels.WARN)
    return
  end
  close()
  vim.cmd.edit(vim.uri_to_fname(uri))
end

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

local function float(pane, col, width, height, title)
  local s = M._state
  local buf = vim.api.nvim_create_buf(false, true)
  vim.bo[buf].bufhidden = "wipe"
  local win = vim.api.nvim_open_win(buf, false, {
    relative = "editor",
    row = math.max(1, math.floor((vim.o.lines - height) / 2) - 1),
    col = col,
    width = width,
    height = height,
    style = "minimal",
    border = "rounded",
    title = " " .. title .. " ",
    title_pos = "center",
  })
  vim.wo[win].cursorline = pane ~= "center"
  s.bufs[pane], s.wins[pane] = buf, win
  keymaps(pane, buf)
end

function M.open()
  local src_buf = vim.api.nvim_get_current_buf()
  local client = vim.lsp.get_clients({ bufnr = src_buf, name = "cogitator" })[1]
  if not client then
    vim.notify("cogitator is not attached to this buffer", vim.log.levels.ERROR)
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
