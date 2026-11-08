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

--- Open the explorer over the whole project graph.
function M.open()
  local src_buf = vim.api.nvim_get_current_buf()
  local client = actions.client_for(src_buf)
  if not client then
    return
  end
  M.close()
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

  actions.exec_with(client, src_buf, "dbt.projectGraph", { uri = vim.uri_from_bufnr(src_buf) }, function(err, result)
    local s = M._state
    if not s then
      return
    end
    if err or not result or result == vim.NIL or not result.nodes or #result.nodes == 0 then
      M.close()
      vim.notify("dbt.projectGraph returned nothing — is the project context loaded yet?", vim.log.levels.WARN)
      return
    end
    s.nodes = vim.tbl_filter(function(n)
      return n.kind == "model"
    end, result.nodes)
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
