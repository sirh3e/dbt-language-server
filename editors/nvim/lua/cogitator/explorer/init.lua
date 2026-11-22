--- Project-wide model explorer (:DbtBrowse / :DbtExplorer, Neovim only):
--- a yazi-style float over the whole dbt graph. Center lists every model
--- (`/` filters as you type), the side panes show the selection's
--- upstream and downstream neighbors (dbt.modelInfo), and the bottom
--- preview switches between raw SQL, compiled SQL and versions/tests.
--- Split by concern: render (geometry + panes), filter (live search),
--- keys (maps), preview (bottom tabs); this module owns state and I/O.

local actions = require("cogitator.actions")
local efilter = require("cogitator.explorer.filter")
local ekeys = require("cogitator.explorer.keys")
local erender = require("cogitator.explorer.render")
local preview = require("cogitator.explorer.preview")
local ui = require("cogitator.ui")

local M = {}

---@class cogitator.explorer.State
---@field client vim.lsp.Client
---@field src_buf integer
---@field wins table<string, integer> left/center/right/preview
---@field bufs table<string, integer>
---@field nodes cogitator.GraphEntry[] every model node in the project
---@field filtered cogitator.GraphEntry[] the center list after filtering
---@field filter string the live filter text
---@field selected string? the selected model's name
---@field info table? the dbt.modelInfo payload for the selection
---@field compiled table|false|nil dbt.compiledSql (false = none, nil = loading)
---@field relation table|false|nil dbt.relation (false = unresolved, nil = loading)
---@field info_jumps table<integer, {uri: string, range?: table}>? info-tab jump targets
---@field info_gen integer staleness guard for in-flight fetches
---@field tab integer active preview tab (1-based)
---@field timer uv.uv_timer_t selection debounce
---@field sides_for string? the model the side panes were last filled for
---@field pending_walk {pane: "left"|"right", count: integer}? walk steps waiting on dbt.modelInfo
---@field graph table the dbt.projectGraph payload
---@field positions table<string, integer[]> the filter's matched characters, by name
---@field marked table<string, boolean> marked nodes, by name (yazi's selection)
---@field history {back: string[], forward: string[]} models re-centred on, for [ and ]
---@field show_all boolean list every kind of node, not only models
---@field raw {uri: string, lines: string[]?}? the raw tab's file, read in the background
---@type cogitator.explorer.State?
M._state = nil

local DEBOUNCE_MS = 100

function M.close()
  local s = M._state
  if not s then
    return
  end
  M._state = nil
  ui.close_state(s)
end

--- dbt.modelInfo for the selected model; stale responses (the cursor
--- moved on) are discarded via the generation counter.
---@param name string
local function fetch_info(name)
  local s = M._state
  if not s or not s.client or s.client:is_stopped() then
    return
  end
  s.info_gen = s.info_gen + 1
  local gen = s.info_gen
  actions.exec_with(s.client, s.src_buf, "dbt.modelInfo", { uri = "", model = name }, function(err, result)
    s = M._state
    if not s or gen ~= s.info_gen then
      return
    end
    if err then
      s.pending_walk = nil
      vim.notify("dbt.modelInfo failed: " .. err.message, vim.log.levels.ERROR)
      return
    end
    s.info = (result ~= vim.NIL) and result or nil
    erender.sides(s)
    preview.render(s)
    if s.tab == 4 and s.relation == nil then
      M._fetch_relation()
    end
    if s.info then
      ekeys.resume_walk(M)
    else
      s.pending_walk = nil
    end
  end)
end

--- dbt.compiledSql for the selected model (fetched lazily: on entering
--- the compiled tab, and on selection changes while it is open).
--- `false` marks "answered, nothing compiled".
---@param name string
function M._fetch_compiled(name)
  local s = M._state
  if not s or not s.client or s.client:is_stopped() then
    return
  end
  local gen = s.info_gen
  actions.exec_with(s.client, s.src_buf, "dbt.compiledSql", { uri = "", model = name }, function(err, result)
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

--- dbt.relation for the selected model's file (needs s.info.uri, so it
--- runs after modelInfo lands). `false` marks "answered, unresolved".
function M._fetch_relation()
  local s = M._state
  if not s or not s.client or s.client:is_stopped() or not s.info then
    return
  end
  local gen = s.info_gen
  local name = s.selected
  actions.exec_with(s.client, s.src_buf, "dbt.relation", { uri = s.info.uri }, function(err, result)
    s = M._state
    if not s or gen ~= s.info_gen or s.selected ~= name then
      return
    end
    if err then
      vim.notify("dbt.relation failed: " .. err.message, vim.log.levels.ERROR)
      return
    end
    s.relation = (result ~= vim.NIL) and result or false
    preview.render(s)
  end)
end

--- <CR> on an info-tab test line: close and jump to its YAML entry.
function M._info_jump()
  local s = M._state
  if not s or s.tab ~= 4 or not s.info_jumps then
    return
  end
  local line = vim.api.nvim_win_get_cursor(s.wins.preview)[1]
  local target = s.info_jumps[line]
  if not target or not target.uri or target.uri == vim.NIL then
    return
  end
  M.close()
  if actions.open_uri(target.uri) and target.range and target.range ~= vim.NIL then
    local start = target.range.start
    pcall(vim.api.nvim_win_set_cursor, 0, { start.line + 1, start.character })
  end
end

--- The model under the center cursor, or nil with an empty filter hit.
---@return cogitator.GraphEntry?
function M._selected_node()
  local s = M._state
  if #s.filtered == 0 then
    return nil
  end
  return s.filtered[vim.api.nvim_win_get_cursor(s.wins.center)[1]]
end

--- Called on every center-cursor move: debounce, then fetch.
function M._select_current()
  local s = M._state
  if not s then
    return
  end
  local node = M._selected_node()
  if not node then
    s.selected, s.info, s.compiled, s.relation = nil, nil, nil, nil
    erender.sides(s)
    preview.render(s)
    return
  end
  if node.name == s.selected then
    return
  end
  s.selected = node.name
  ui.retitle(s.wins.center, erender.center_title(s))
  s.info = nil
  s.compiled = nil
  s.relation = nil
  s.timer:stop()
  s.timer:start(
    DEBOUNCE_MS,
    0,
    vim.schedule_wrap(function()
      local st = M._state
      if st and st.selected == node.name then
        fetch_info(node.name)
        if st.tab == 2 then
          M._fetch_compiled(node.name)
        end
      end
    end)
  )
end

--- Switch the preview tab (1=raw, 2=compiled, 3=versions/tests, 4=info).
---@param tab integer
function M._set_tab(tab)
  local s = M._state
  s.tab = tab
  if tab == 2 and s.selected and s.compiled == nil then
    M._fetch_compiled(s.selected)
  end
  if tab == 4 and s.relation == nil then
    M._fetch_relation()
  end
  preview.render(s)
end

--- The nodes the center lists: the models, or every kind with `.`.
---@param s cogitator.explorer.State
local function listed(s)
  return vim.tbl_filter(function(n)
    return s.show_all or n.kind == "model"
  end, s.graph.nodes)
end

--- Put `name` under the center cursor (clearing a filter that hides it)
--- and select it. With `record`, the model left behind goes on the back
--- history, as walking the lineage does.
---@param name string
---@param opts {record?: boolean, stay?: boolean}?
---@return boolean found
function M.focus(name, opts)
  local s = M._state
  if not s then
    return false
  end
  opts = opts or {}
  local function row_of()
    for i, n in ipairs(s.filtered) do
      if n.name == name then
        return i
      end
    end
  end
  local row = row_of()
  if not row and s.filter ~= "" then
    s.filter = ""
    efilter.apply(M)
    row = row_of()
  end
  if not row then
    vim.notify(name .. " is not in the navigator's list (. lists every kind of node)", vim.log.levels.WARN)
    return false
  end
  if opts.record and s.selected and s.selected ~= name then
    table.insert(s.history.back, s.selected)
    s.history.forward = {}
  end
  vim.api.nvim_win_set_cursor(s.wins.center, { row, 0 })
  if not opts.stay then
    vim.api.nvim_set_current_win(s.wins.center)
  end
  M._select_current()
  return true
end

--- Step through the history: -1 back (`[`, `-`), +1 forward (`]`).
---@param delta integer
function M._history(delta)
  local s = M._state
  local from, to = s.history.back, s.history.forward
  if delta > 0 then
    from, to = to, from
  end
  local name = table.remove(from)
  if not name then
    return
  end
  if s.selected then
    table.insert(to, s.selected)
  end
  M.focus(name)
end

--- The marked nodes, or the one under the cursor.
---@return cogitator.GraphEntry[]
function M._chosen()
  local s = M._state
  local out = {}
  for _, n in ipairs(s.nodes) do
    if s.marked[n.name] then
      out[#out + 1] = n
    end
  end
  if #out == 0 then
    out[1] = M._selected_node()
  end
  return out
end

--- Mark or unmark the node under the cursor, then move down (yazi's <Space>).
function M._toggle_mark()
  local s = M._state
  local node = M._selected_node()
  if not node then
    return
  end
  s.marked[node.name] = not s.marked[node.name] or nil
  erender.center(s, true)
  local row = vim.api.nvim_win_get_cursor(s.wins.center)[1]
  if row < #s.filtered then
    vim.api.nvim_win_set_cursor(s.wins.center, { row + 1, 0 })
  end
  M._select_current()
end

--- `.`: list every kind of node, or only the models again.
function M._toggle_all()
  local s = M._state
  s.show_all = not s.show_all
  local keep = s.selected
  s.nodes = listed(s)
  efilter.apply(M)
  if keep then
    M.focus(keep)
  end
end

--- `f`: the models picker; the choice becomes the selection.
function M._find()
  local s = M._state
  local sources = require("cogitator.pick.sources")
  local source = sources.build(s.show_all and "all" or "models", { client = s.client, bufnr = s.src_buf })
  source.choose = function(items)
    if M._state and items[1] then
      M.focus(items[1].name, { record = true })
    end
  end
  require("cogitator.pick").open(source)
end

--- Open the navigator over the whole project graph; `opts.focus` (a
--- name) or `opts.focus_buffer` puts that model under the cursor.
---@param opts {focus?: string, focus_buffer?: boolean}?
function M.open(opts)
  opts = opts or {}
  local src_buf = vim.api.nvim_get_current_buf()
  local client = actions.client_for(src_buf)
  if not client then
    return
  end
  M.close()
  local config = require("cogitator").config or {}
  M._state = {
    client = client,
    src_buf = src_buf,
    wins = {},
    bufs = {},
    nodes = {},
    filtered = {},
    filter = "",
    positions = {},
    marked = {},
    history = { back = {}, forward = {} },
    show_all = (config.navigator or {}).show_all_kinds == true,
    selected = nil,
    info = nil,
    compiled = nil,
    info_gen = 0,
    tab = 1,
    timer = vim.uv.new_timer(),
  }

  require("cogitator.pick.graph").get(client, src_buf, function(result)
    local s = M._state
    if not s then
      return
    end
    if not result or #result.nodes == 0 then
      M.close()
      vim.notify(
        "dbt.projectGraph returned nothing — is the project context loaded yet?",
        vim.log.levels.WARN
      )
      return
    end
    s.graph = result
    s.nodes = listed(s)
    s.filtered = s.nodes

    local l = erender.geometry()
    local function pane(name, row, col, width, height, title, cursorline)
      local buf, win = ui.float({
        row = row,
        col = col,
        width = width,
        height = height,
        title = title,
        cursorline = cursorline,
      })
      s.bufs[name], s.wins[name] = buf, win
      ekeys.attach(M, name, buf)
    end
    pane("left", l.row, l.col, l.side, l.list_h, "upstream ⯇", true)
    pane("center", l.row, l.col + l.side + 2, l.center, l.list_h, "models", true)
    pane("right", l.row, l.col + l.side + 2 + l.center + 2, l.side, l.list_h, "⯈ downstream", true)
    pane("preview", l.prev_row, l.col, l.prev_w, l.prev_h, "preview", false)
    vim.api.nvim_create_autocmd("CursorMoved", {
      buffer = s.bufs.center,
      callback = M._select_current,
    })
    efilter.apply(M)
    vim.api.nvim_set_current_win(s.wins.center)
    local focus = opts.focus
    if not focus and opts.focus_buffer then
      local here = require("cogitator.pick.graph").node_of_buffer(result, src_buf)
      focus = here and here.name
    end
    if focus then
      M.focus(focus)
    end
  end)
end

--- Test hook: set the filter programmatically (the `/` loop reads
--- typeahead, which is racy in a headless run).
---@param text string?
function M._apply_filter(text)
  if M._state then
    M._state.filter = text or ""
    efilter.apply(M)
  end
end

return M
