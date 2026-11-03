--- Project-wide model browser (:DbtBrowse, Neovim only): a yazi-style float
--- over the whole dbt graph. Center lists every model (`/` filters as you
--- type), the side panes show the selection's upstream and downstream
--- neighbors (dbt.modelInfo), and the bottom preview switches between the
--- raw SQL, the compiled SQL and the versions/tests summary.

local preview = require("cogitator.browse_preview")

local M = {}

M._state = nil

local DEBOUNCE_MS = 100

local function close()
  local s = M._state
  if not s then
    return
  end
  M._state = nil
  if s.timer then
    s.timer:stop()
    s.timer:close()
  end
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

--- All float geometry in one place: three columns on top, one full-width
--- preview below.
local function layout()
  local total = math.min(vim.o.columns - 4, 140)
  local side = math.floor((total - 4) / 4)
  local center = total - 2 * (side + 2)
  local usable = math.max(14, vim.o.lines - 6)
  local list_h = math.max(8, math.floor(usable * 0.55))
  local prev_h = math.max(5, usable - list_h - 2)
  local row = math.max(1, math.floor((vim.o.lines - (list_h + prev_h + 2)) / 2) - 1)
  local col = math.max(0, math.floor((vim.o.columns - total) / 2))
  return {
    row = row,
    col = col,
    side = side,
    center = center,
    list_h = list_h,
    prev_h = prev_h,
    prev_row = row + list_h + 2,
    prev_w = 2 * side + center + 4,
  }
end

local function entry_label(e)
  local label = e.name
  if e.kind == "source" then
    label = label .. "  [source]"
  end
  if not e.uri or e.uri == vim.NIL then
    label = label .. "  (missing)"
  end
  return label
end

local function pane_lines(entries)
  if not entries or #entries == 0 then
    return { "· none ·" }
  end
  return vim.tbl_map(entry_label, entries)
end

local function set_lines(buf, lines)
  vim.bo[buf].modifiable = true
  vim.api.nvim_buf_set_lines(buf, 0, -1, false, lines)
  vim.bo[buf].modifiable = false
end

--- Smart-case plain-substring match: a lowercase filter ignores case.
local function matches(filter, name)
  if filter == "" then
    return true
  end
  if filter:lower() == filter then
    return name:lower():find(filter, 1, true) ~= nil
  end
  return name:find(filter, 1, true) ~= nil
end

local function render_sides(s)
  local info = s.info
  set_lines(s.bufs.left, pane_lines(info and info.upstream))
  set_lines(s.bufs.right, pane_lines(info and info.downstream))
  for _, pane in ipairs({ "left", "right" }) do
    if vim.api.nvim_win_is_valid(s.wins[pane]) then
      vim.api.nvim_win_set_cursor(s.wins[pane], { 1, 0 })
    end
  end
end

--- dbt.modelInfo for the selected model; stale responses (the cursor moved
--- on) are discarded via the generation counter.
local function fetch_info(name)
  local s = M._state
  if not s or not s.client or s.client:is_stopped() then
    return
  end
  s.info_gen = s.info_gen + 1
  local gen = s.info_gen
  s.client:exec_cmd({
    command = "dbt.modelInfo",
    arguments = { { uri = "", model = name } },
  }, { bufnr = s.src_buf }, function(err, result)
    s = M._state
    if not s or gen ~= s.info_gen then
      return
    end
    if err then
      vim.notify("dbt.modelInfo failed: " .. err.message, vim.log.levels.ERROR)
      return
    end
    s.info = (result ~= vim.NIL) and result or nil
    render_sides(s)
    preview.render(s)
  end)
end

--- dbt.compiledSql for the selected model (fetched lazily: on entering the
--- compiled tab, and on selection changes while it is open). `false` marks
--- "answered, nothing compiled" so the pane stops saying loading.
local function fetch_compiled(name)
  local s = M._state
  if not s or not s.client or s.client:is_stopped() then
    return
  end
  local gen = s.info_gen
  s.client:exec_cmd({
    command = "dbt.compiledSql",
    arguments = { { uri = "", model = name } },
  }, { bufnr = s.src_buf }, function(err, result)
    s = M._state
    if not s or gen ~= s.info_gen or s.selected ~= name then
      return
    end
    if err then
      vim.notify("dbt.compiledSql failed: " .. err.message, vim.log.levels.ERROR)
      return
    end
    s.compiled = (result ~= vim.NIL) and result or false
    preview.render(s)
  end)
end

local function selected_node()
  local s = M._state
  if #s.filtered == 0 then
    return nil
  end
  return s.filtered[vim.api.nvim_win_get_cursor(s.wins.center)[1]]
end

--- Called on every center-cursor move: debounce, then fetch.
local function select_current()
  local s = M._state
  if not s then
    return
  end
  local node = selected_node()
  if not node then
    s.selected, s.info, s.compiled = nil, nil, nil
    render_sides(s)
    preview.render(s)
    return
  end
  if node.name == s.selected then
    return
  end
  s.selected = node.name
  s.info = nil
  s.compiled = nil
  s.timer:stop()
  s.timer:start(
    DEBOUNCE_MS,
    0,
    vim.schedule_wrap(function()
      local st = M._state
      if st and st.selected == node.name then
        fetch_info(node.name)
        if st.tab == 2 then
          fetch_compiled(node.name)
        end
      end
    end)
  )
end

local function center_title(s)
  local title = (" models %d/%d "):format(#s.filtered, #s.nodes)
  if s.filter ~= "" then
    title = (" models /%s "):format(s.filter)
  end
  return title
end

local function apply_filter()
  local s = M._state
  s.filtered = vim.tbl_filter(function(n)
    return matches(s.filter, n.name)
  end, s.nodes)
  local lines = {}
  for _, n in ipairs(s.filtered) do
    lines[#lines + 1] = n.name
  end
  if #lines == 0 then
    lines = { "· no match ·" }
  end
  set_lines(s.bufs.center, lines)
  vim.api.nvim_win_set_cursor(s.wins.center, { 1, 0 })
  vim.api.nvim_win_set_config(s.wins.center, { title = center_title(s), title_pos = "center" })
  s.selected = nil -- force a refetch even when line 1 holds the same model
  select_current()
end

--- Live filter: printable keys narrow the list on every keystroke, <BS>
--- deletes, <CR> accepts, <Esc> clears and leaves.
local function filter_prompt()
  local s = M._state
  local bs = vim.api.nvim_replace_termcodes("<BS>", true, false, true)
  while true do
    vim.cmd.redraw()
    local ok, ch = pcall(vim.fn.getcharstr)
    if not ok or ch == "\27" then
      s.filter = ""
      apply_filter()
      return
    elseif ch == "\r" or ch == "\n" then
      return
    elseif ch == bs then
      s.filter = s.filter:sub(1, -2)
      apply_filter()
    elseif #ch == 1 and ch:match("[%w%p ]") then
      s.filter = s.filter .. ch
      apply_filter()
    else
      return
    end
  end
end

local function open_uri(uri)
  if not uri or uri == vim.NIL then
    vim.notify("no file on disk for this entry", vim.log.levels.WARN)
    return
  end
  close()
  vim.cmd.edit(vim.uri_to_fname(uri))
end

--- <CR> on a side-pane entry: make it the center selection (clearing the
--- filter when it is filtered out), like re-centering in :DbtGraph.
local function reselect(pane)
  local s = M._state
  local entries = s.info and (pane == "left" and s.info.upstream or s.info.downstream)
  local e = entries and entries[vim.api.nvim_win_get_cursor(s.wins[pane])[1]]
  if not e then
    return
  end
  if e.kind ~= "model" then
    vim.notify(e.name .. " is a source — sources are not in the model list", vim.log.levels.INFO)
    return
  end
  local function row_of(name)
    for i, n in ipairs(s.filtered) do
      if n.name == name then
        return i
      end
    end
  end
  local row = row_of(e.name)
  if not row then
    s.filter = ""
    apply_filter()
    row = row_of(e.name)
  end
  if not row then
    vim.notify(e.name .. " is not in the project graph", vim.log.levels.WARN)
    return
  end
  vim.api.nvim_win_set_cursor(s.wins.center, { row, 0 })
  vim.api.nvim_set_current_win(s.wins.center)
  select_current()
end

local function set_tab(tab)
  local s = M._state
  s.tab = tab
  if tab == 2 and s.selected and s.compiled == nil then
    fetch_compiled(s.selected)
  end
  preview.render(s)
end

local function side_entry(pane)
  local s = M._state
  local entries = s.info and (pane == "left" and s.info.upstream or s.info.downstream)
  return entries and entries[vim.api.nvim_win_get_cursor(s.wins[pane])[1]]
end

local function keymaps(pane, buf)
  local s = M._state
  local function map(lhs, fn)
    vim.keymap.set("n", lhs, fn, { buffer = buf, nowait = true })
  end
  map("q", close)
  for i = 1, 3 do
    map(tostring(i), function()
      set_tab(i)
    end)
  end
  map("<Tab>", function()
    set_tab(M._state.tab % 3 + 1)
  end)
  if pane == "preview" then
    map("<Esc>", close)
    map("h", function()
      vim.api.nvim_set_current_win(s.wins.center)
    end)
    return
  end
  map("p", function()
    vim.api.nvim_set_current_win(s.wins.preview)
  end)
  map("h", function()
    vim.api.nvim_set_current_win(s.wins[pane == "right" and "center" or "left"])
  end)
  map("l", function()
    vim.api.nvim_set_current_win(s.wins[pane == "left" and "center" or "right"])
  end)
  if pane == "center" then
    map("<Esc>", function()
      if M._state.filter ~= "" then
        M._state.filter = ""
        apply_filter()
      else
        close()
      end
    end)
    map("/", filter_prompt)
    local open_center = function()
      local node = selected_node()
      if node then
        open_uri(node.uri)
      end
    end
    map("<CR>", open_center)
    map("o", open_center)
  else
    map("<Esc>", close)
    map("<CR>", function()
      reselect(pane)
    end)
    map("o", function()
      local e = side_entry(pane)
      if e then
        open_uri(e.uri)
      end
    end)
  end
end

local function float(pane, row, col, width, height, title)
  local s = M._state
  local buf = vim.api.nvim_create_buf(false, true)
  vim.bo[buf].bufhidden = "wipe"
  local win = vim.api.nvim_open_win(buf, false, {
    relative = "editor",
    row = row,
    col = col,
    width = width,
    height = height,
    style = "minimal",
    border = "rounded",
    title = " " .. title .. " ",
    title_pos = "center",
  })
  vim.wo[win].cursorline = pane ~= "preview"
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
  M._state = {
    client = client,
    src_buf = src_buf,
    wins = {},
    bufs = {},
    nodes = {},
    filtered = {},
    filter = "",
    selected = nil,
    info = nil,
    compiled = nil,
    info_gen = 0,
    tab = 1,
    timer = vim.uv.new_timer(),
  }

  client:exec_cmd({
    command = "dbt.projectGraph",
    arguments = { { uri = vim.uri_from_bufnr(src_buf) } },
  }, { bufnr = src_buf }, function(err, result)
    local s = M._state
    if not s then
      return
    end
    if err or not result or result == vim.NIL or not result.nodes or #result.nodes == 0 then
      close()
      vim.notify("dbt.projectGraph returned nothing — is the project context loaded yet?", vim.log.levels.WARN)
      return
    end
    s.nodes = vim.tbl_filter(function(n)
      return n.kind == "model"
    end, result.nodes)
    s.filtered = s.nodes

    local l = layout()
    float("left", l.row, l.col, l.side, l.list_h, "upstream ⯇")
    float("center", l.row, l.col + l.side + 2, l.center, l.list_h, "models")
    float("right", l.row, l.col + l.side + 2 + l.center + 2, l.side, l.list_h, "⯈ downstream")
    float("preview", l.prev_row, l.col, l.prev_w, l.prev_h, "preview")
    vim.api.nvim_create_autocmd("CursorMoved", {
      buffer = s.bufs.center,
      callback = select_current,
    })
    apply_filter()
    vim.api.nvim_set_current_win(s.wins.center)
  end)
end

--- Test hook: set the filter programmatically (the `/` loop reads typeahead,
--- which is racy in a headless run).
function M._apply_filter(text)
  if M._state then
    M._state.filter = text or ""
    apply_filter()
  end
end

M.close = close

return M
