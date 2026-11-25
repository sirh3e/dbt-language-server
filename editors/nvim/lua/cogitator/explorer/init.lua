--- The model explorer (:DbtBrowse / :DbtExplorer, Neovim only): one model
--- at a time, in five floats —
---
---   upstream ⯇   |   the model (its card)   |   ⯈ downstream
---   the dbt SQL               |               the compiled SQL
---
--- The side panes list the selection's direct parents and children
--- (dbt.modelInfo); <CR> on one, or h/l pressed again in its pane, makes it
--- the selection (yazi-style walking, with history). `f` and `/` pick any
--- model with the fuzzy picker. Split by concern: render (geometry, side
--- panes, the card), card (the middle pane's lines), preview (the two SQL
--- panes), keys (maps); this module owns the state and the I/O.

local actions = require("cogitator.actions")
local ekeys = require("cogitator.explorer.keys")
local erender = require("cogitator.explorer.render")
local preview = require("cogitator.explorer.preview")
local ui = require("cogitator.ui")

local M = {}

---@class cogitator.explorer.State
---@field client vim.lsp.Client
---@field src_buf integer
---@field wins table<string, integer> left/center/right/raw/compiled
---@field bufs table<string, integer>
---@field graph table the dbt.projectGraph payload
---@field nodes cogitator.GraphEntry[] the nodes the picker offers (models, or every kind)
---@field node_of table<string, table> every graph node, by name
---@field selected string? the selected model's name
---@field info table? the dbt.modelInfo payload for the selection
---@field compiled table|false|nil dbt.compiledSql (false = none, nil = loading)
---@field relation table|false|nil dbt.relation (false = unresolved, nil = loading)
---@field raw {uri: string, lines: string[]?}? the dbt SQL pane's file, read in the background
---@field card_jumps table<integer, {uri: string, range?: table}>? card lines that jump
---@field info_gen integer staleness guard for in-flight fetches
---@field timer uv.uv_timer_t selection debounce
---@field sides_for string? the model the side panes were last filled for
---@field pending_walk {pane: "left"|"right", count: integer}? walk steps waiting on dbt.modelInfo
---@field marked table<string, boolean> marked models, by name (yazi's selection)
---@field history {back: string[], forward: string[]} models re-centred from, for [ and ]
---@field show_all boolean the picker and walks take every kind of node, not only models
---@field macros table[]? the project's macros, as nodes (workspace/symbol)
---@field compiled_jumps table<integer, string>? compiled pane lines → a caller to re-centre on
---@field selection {select: string, exclude: string?, keys: table<string, boolean>, count: integer}? what <C-a> marked from
---@type cogitator.explorer.State?
M._state = nil

local DEBOUNCE_MS = 60

function M.close()
  local s = M._state
  if not s then
    return
  end
  M._state = nil
  ui.close_state(s)
end

--- Redraw every pane from the state.
local function render_all(s)
  erender.sides(s)
  erender.center(s)
  preview.render(s)
end

--- dbt.relation for the selected model's file (it needs info.uri).
local function fetch_relation(s, gen)
  if not s.info or not s.info.uri or s.info.uri == vim.NIL then
    s.relation = false
    erender.center(s)
    return
  end
  local name = s.selected
  actions.exec_with(s.client, s.src_buf, "dbt.relation", { uri = s.info.uri }, function(err, result)
    if M._state ~= s or gen ~= s.info_gen or s.selected ~= name then
      return
    end
    s.relation = (not err and result ~= vim.NIL and result) or false
    erender.center(s)
  end)
end

--- dbt.nodeInfo's answer as the panes read it: a macro's neighbours are
--- the macros it calls (left) and its callers (right); a test's what it
--- refs; the rest the graph's up- and downstream.
---@param info table
---@return table
function M._normalize(info)
  if info.kind == "macro" then
    info.upstream, info.downstream = info.calls or {}, info.callers or {}
  elseif info.kind == "test" or info.kind == "analysis" then
    info.upstream = vim.list_extend(vim.list_extend({}, info.refs or {}), info.calls or {})
    info.downstream = {}
  elseif info.kind == "generic_test" then
    info.upstream, info.downstream = {}, {}
  else
    info.upstream, info.downstream = info.upstream or {}, info.downstream or {}
  end
  return info
end

--- A node that is not a model: dbt.nodeInfo, and for its compiled pane a
--- test's compiled SQL or a macro's callers.
---@param s cogitator.explorer.State
---@param node table
---@param gen integer
local function fetch_node(s, node, gen)
  local name = node.name
  actions.exec_with(
    s.client,
    s.src_buf,
    "dbt.nodeInfo",
    { node = { kind = node.kind, name = name } },
    function(err, result)
      if M._state ~= s or gen ~= s.info_gen then
        return
      end
      if err or result == nil or result == vim.NIL then
        s.pending_walk = nil
        s.info = { kind = node.kind, name = name, uri = node.uri, upstream = {}, downstream = {} }
        s.compiled = false
        render_all(s)
        return
      end
      s.info = M._normalize(result)
      s.relation = false
      if result.kind == "macro" then
        s.compiled = { callers = result.callers or {} }
      elseif result.compiledUri and result.compiledUri ~= vim.NIL then
        s.compiled = nil
        local uri = result.compiledUri
        require("cogitator.pick.preview").read(vim.uri_to_fname(uri), function(lines)
          if M._state == s and gen == s.info_gen then
            s.compiled = lines and { content = table.concat(lines, "\n"), uri = uri } or false
            preview.render_compiled(s)
          end
        end)
      else
        s.compiled = false
      end
      render_all(s)
      ekeys.resume_walk(M)
    end
  )
end

--- Everything the panes show of `name`: dbt.modelInfo (then the relation)
--- and dbt.compiledSql side by side for a model, dbt.nodeInfo for any
--- other node; answers for an older selection are dropped by the
--- generation counter.
---@param name string
local function fetch(name)
  local s = M._state
  if not s or not s.client or s.client:is_stopped() then
    return
  end
  s.info_gen = s.info_gen + 1
  local gen = s.info_gen
  local node = s.node_of[name]
  if node and node.kind ~= "model" then
    return fetch_node(s, node, gen)
  end
  actions.exec_with(s.client, s.src_buf, "dbt.modelInfo", { uri = "", model = name }, function(err, result)
    if M._state ~= s or gen ~= s.info_gen then
      return
    end
    if err then
      s.pending_walk = nil
      vim.notify("dbt.modelInfo failed: " .. err.message, vim.log.levels.ERROR)
      return
    end
    s.info = (result ~= vim.NIL) and result or nil
    render_all(s)
    fetch_relation(s, gen)
    if s.info then
      ekeys.resume_walk(M)
    else
      s.pending_walk = nil
    end
  end)
  actions.exec_with(s.client, s.src_buf, "dbt.compiledSql", { uri = "", model = name }, function(err, result)
    if M._state ~= s or gen ~= s.info_gen then
      return
    end
    if err then
      s.compiled = false
    else
      s.compiled = (result ~= vim.NIL) and result or false
    end
    preview.render_compiled(s)
    erender.center(s) -- the card names the compiled file
  end)
end

--- The graph node of the selection.
---@return table?
function M._selected_node()
  local s = M._state
  return s and s.selected and s.node_of[s.selected] or nil
end

--- Make `name` the selection. With `record`, the model left behind goes
--- on the back history, as walking the lineage does.
---@param name string
---@param opts {record?: boolean, stay?: boolean}?
---@return boolean found
function M.focus(name, opts)
  local s = M._state
  if not s then
    return false
  end
  opts = opts or {}
  -- a neighbour the graph payload does not list (a macro), learnt from
  -- the side pane that names it
  if not s.node_of[name] and opts.node then
    s.node_of[name] = opts.node
  end
  local node = s.node_of[name]
  if not node then
    vim.notify(name .. " is not in the project graph", vim.log.levels.WARN)
    return false
  end
  if node.kind ~= "model" and not s.show_all then
    vim.notify(
      ("%s is a %s — . lets the explorer show every kind of node"):format(name, node.kind:gsub("_", " ")),
      vim.log.levels.INFO
    )
    return false
  end
  if opts.record and s.selected and s.selected ~= name then
    table.insert(s.history.back, s.selected)
    s.history.forward = {}
  end
  s.selected = name
  s.info, s.compiled, s.relation, s.raw = nil, nil, nil, nil
  render_all(s)
  if not opts.stay then
    vim.api.nvim_set_current_win(s.wins.center)
  end
  s.timer:stop()
  s.timer:start(
    DEBOUNCE_MS,
    0,
    vim.schedule_wrap(function()
      if M._state == s and s.selected == name then
        fetch(name)
      end
    end)
  )
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

--- The marked models, or `fallback` (the selection by default).
---@param fallback string?
---@return string[]
function M._chosen(fallback)
  local s = M._state
  local out = vim.tbl_keys(s.marked)
  table.sort(out)
  if #out == 0 then
    out[1] = fallback or s.selected
  end
  return out
end

--- Mark or unmark `name` (the selection by default), yazi's <Space>.
---@param name string?
function M._toggle_mark(name)
  local s = M._state
  name = name or s.selected
  if not name then
    return
  end
  s.marked[name] = not s.marked[name] or nil
  erender.sides(s)
  erender.center(s)
end

--- The nodes the picker offers: the graph's, and the macros
--- (workspace/symbol) when every kind is shown.
local function listed(s)
  local out = vim.tbl_filter(function(n)
    return s.show_all or n.kind == "model"
  end, s.graph.nodes)
  if s.show_all then
    vim.list_extend(out, s.macros or {})
  end
  return out
end

--- The project's macros, as nodes (they are no node of dbt.projectGraph).
---@param s cogitator.explorer.State
local function load_macros(s)
  actions.request(
    s.client,
    s.src_buf,
    "workspace/symbol",
    { query = "" },
    { timeout_ms = 15000 },
    function(err, result)
      if M._state ~= s or err or type(result) ~= "table" then
        return
      end
      s.macros = {}
      for _, sym in ipairs(result) do
        if sym.kind == 12 and sym.location then
          local node = { kind = "macro", name = sym.name, uri = sym.location.uri }
          s.macros[#s.macros + 1] = node
          s.node_of[sym.name] = s.node_of[sym.name] or node
        end
      end
      s.nodes = listed(s)
    end
  )
end

--- `.`: let the picker and the walks take every kind of node, or only
--- models again.
function M._toggle_all()
  local s = M._state
  s.show_all = not s.show_all
  s.nodes = listed(s)
  s.sides_for = nil
  erender.sides(s)
  vim.notify(s.show_all and "explorer: every kind of node" or "explorer: models only")
end

--- `f` and `/`: the models picker; the choice becomes the selection. A
--- dbt selection typed there lists what dbt would select (explorer.select).
---@param on_cancel fun()?
---@param query string? typed in already (:DbtSelect)
function M._find(on_cancel, query)
  local s = M._state
  local sources = require("cogitator.pick.sources")
  local source = sources.build(s.show_all and "all" or "models", { client = s.client, bufnr = s.src_buf })
  if s.show_all and s.macros and #s.macros > 0 then
    -- the graph's nodes, and the macros beside them
    local graph_items = source.items
    source.items = function(done)
      graph_items(function(items)
        for _, m in ipairs(s.macros) do
          items[#items + 1] = { text = m.name .. "  [macro]", name = m.name, uri = m.uri, value = m }
        end
        done(items)
      end)
    end
  end
  local select = require("cogitator.explorer.select")
  require("cogitator.pick.select_source").with_selection(source, { client = s.client, bufnr = s.src_buf })
  source.keys = select.keys(M)
  source.query = query
  source.choose = function(items)
    if M._state == s and items[1] then
      select.choose(M, items[1])
    end
  end
  source.on_cancel = on_cancel
  require("cogitator.pick").open(source)
end

--- <CR> on the card: a test or version line jumps to it, any other line
--- opens the model.
function M._card_enter()
  local s = M._state
  local line = vim.api.nvim_win_get_cursor(s.wins.center)[1]
  local target = s.card_jumps and s.card_jumps[line]
  local uri = target and target.uri
  if not target then
    local node = M._selected_node()
    uri = node and node.uri
  end
  if not uri or uri == vim.NIL then
    vim.notify("no file on disk for this entry", vim.log.levels.WARN)
    return
  end
  M.close()
  if actions.open_uri(uri) and target and target.range and target.range ~= vim.NIL then
    local start = target.range.start
    pcall(vim.api.nvim_win_set_cursor, 0, { start.line + 1, start.character })
  end
end

--- Open the selection's dbt SQL ("raw") or compiled SQL ("compiled"),
--- at `line` when given, closing the explorer.
---@param which "raw"|"compiled"
---@param line integer?
function M._open_sql(which, line)
  local s = M._state
  if not s then
    return
  end
  local uri
  if which == "compiled" then
    uri = require("cogitator.explorer.card").compiled_uri(s)
    if not uri then
      vim.notify("not compiled yet — c compiles it, then E opens it", vim.log.levels.INFO)
      return
    end
  else
    local node = M._selected_node()
    uri = (s.info and s.info.uri ~= vim.NIL and s.info.uri)
      or (node and node.uri ~= vim.NIL and node.uri)
      or nil
    if not uri then
      vim.notify("no file on disk for this entry", vim.log.levels.WARN)
      return
    end
  end
  M.close()
  if actions.open_uri(uri) and line then
    pcall(vim.api.nvim_win_set_cursor, 0, { line, 0 })
  end
end

--- Open the explorer on `opts.focus` (a name), else on the current
--- buffer's model, else ask with the models picker. `opts.select` opens
--- the finder with that dbt selection typed in (:DbtSelect).
---@param opts {focus?: string, focus_buffer?: boolean, select?: string}?
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
    node_of = {},
    marked = {},
    history = { back = {}, forward = {} },
    show_all = (config.navigator or {}).show_all_kinds == true,
    info_gen = 0,
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
    for _, node in ipairs(result.nodes) do
      s.node_of[node.name] = s.node_of[node.name] or node
    end
    s.nodes = listed(s)
    load_macros(s)

    local g = erender.geometry()
    local titles = {
      left = "upstream ⯇",
      center = "model",
      right = "⯈ downstream",
      raw = "dbt SQL",
      compiled = "compiled SQL",
    }
    for _, name in ipairs({ "left", "center", "right", "raw", "compiled" }) do
      local r = g[name]
      local buf, win = ui.float({
        row = r.row,
        col = r.col,
        width = r.width,
        height = r.height,
        title = titles[name],
        cursorline = name ~= "raw" and name ~= "compiled",
      })
      s.bufs[name], s.wins[name] = buf, win
      ekeys.attach(M, name, buf)
    end
    vim.api.nvim_set_current_win(s.wins.center)

    local focus = opts.focus
    if not focus then
      local here = require("cogitator.pick.graph").node_of_buffer(result, src_buf)
      focus = here and here.name
    end
    if focus and M.focus(focus) and not opts.select then
      return
    end
    render_all(s)
    M._find(function()
      -- nothing was chosen: an explorer without a model has nothing to show
      if M._state == s and not s.selected then
        M.close()
      end
    end, opts.select)
  end)
end

return M
