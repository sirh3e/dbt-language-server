--- The column-lineage dialog (:DbtColumnLineage, Neovim only), in the
--- style of the yazi-style browsers — Miller panes over the columns:
---
---   ┌ upstream ⯇ ───┐┌ columns of customers ┐┌ ⯈ downstream ─┐
---   │ ◆ jaffle.raw… ││ ▦ customers.custom… ⋈││ ▦ orders.cust… │
---   └───────────────┘└──────────────────────┘└────────────────┘
---   ┌ dbt SQL · customers.sql ┐┌ compiled SQL ──────────────────┐
---   └─────────────────────────┘└────────────────────────────────┘
---
--- j/k (any motion) moves the cursor and the side panes and previews
--- follow it; h/l walk (the side pane becomes the list); `/` filters in
--- fzf's syntax; <CR> opens the column where it is defined. The data is
--- dbt.columnLineageGraph (cogitator.lineage_graph) when the server has
--- it, else the client walk over dbt.columnLineage (cogitator.column_lineage),
--- whose progress fills the panes as lineages arrive. Pure parts:
--- model (panes, walking, filter, rows), locate (where a column is
--- written); preview and keys are the shell beside this one.

local actions = require("cogitator.actions")
local model = require("cogitator.column_dialog.model")
local preview = require("cogitator.column_dialog.preview")
local rows = require("cogitator.column_dialog.rows")
local ui = require("cogitator.ui")

local M = {}

M.ROW_NS = vim.api.nvim_create_namespace("cogitator.column_dialog.rows")
vim.api.nvim_set_hl(0, "DbtColumnJoin", { link = "DiagnosticWarn", default = true })
vim.api.nvim_set_hl(0, "DbtColumnDerived", { link = "DiagnosticInfo", default = true })
vim.api.nvim_set_hl(0, "DbtColumnDirect", { link = "Comment", default = true })

---@type table?
M._state = nil

local PANES = { "left", "center", "right", "raw", "compiled" }
local FOOTER = " h/l walk · / filter · <CR> open · c compiled · D graph · ? help "

--- Close the dialog: its floats, the marks in the user's buffers, focus
--- back where it was.
function M.close()
  local s = M._state
  if not s then
    return
  end
  M._state = nil
  preview.clear(s)
  if s.prompt and vim.api.nvim_win_is_valid(s.prompt) then
    vim.api.nvim_win_close(s.prompt, true)
  end
  ui.close_state(s)
  if s.src_win and vim.api.nvim_win_is_valid(s.src_win) then
    vim.api.nvim_set_current_win(s.src_win)
  end
  vim.cmd.stopinsert()
end

--- The node under the cursor.
---@return table?
function M.current()
  local s = M._state
  local id = s and s.ix and model.current(s.ix, s.nav)
  return id and s.ix.nodes[id] or nil
end

local function fill(s, pane, items, empty)
  local buf = s.bufs[pane]
  if not (buf and vim.api.nvim_buf_is_valid(buf)) then
    return
  end
  local lines = vim.tbl_map(function(r)
    return r.text
  end, items)
  ui.set_lines(buf, #lines > 0 and lines or { empty })
  vim.api.nvim_buf_clear_namespace(buf, M.ROW_NS, 0, -1)
  for i, r in ipairs(items) do
    for _, m in ipairs(r.marks) do
      pcall(vim.api.nvim_buf_set_extmark, buf, M.ROW_NS, i - 1, m.col, { end_col = m.stop, hl_group = m.hl })
    end
  end
  if #items == 0 then
    pcall(
      vim.api.nvim_buf_set_extmark,
      buf,
      M.ROW_NS,
      0,
      0,
      { end_col = #empty, hl_group = "DbtPickerDetail" }
    )
  end
end

--- Redraw the previews a moment after the cursor settles.
local function schedule_preview(s)
  s.timer:stop()
  s.timer:start(
    40,
    0,
    vim.schedule_wrap(function()
      if M._state == s then
        preview.render(s, M.current(), function()
          if M._state == s then
            schedule_preview(s)
          end
        end)
      end
    end)
  )
end

--- Redraw the side panes (and the middle one unless `keep_center`).
---@param s table
---@param keep_center boolean?
function M.render(s, keep_center)
  local widths = {}
  for _, pane in ipairs({ "left", "center", "right" }) do
    widths[pane] = vim.api.nvim_win_is_valid(s.wins[pane]) and vim.api.nvim_win_get_width(s.wins[pane]) or nil
  end
  local p = rows.panes(s.ix, s.nav, widths)
  fill(s, "left", p.left, s.loading and "· walking… ·" or "· nothing feeds it ·")
  fill(s, "right", p.right, s.loading and "· walking… ·" or "· nothing reads it ·")
  ui.retitle(s.wins.left, (" upstream ⯇ %d "):format(#p.left))
  ui.retitle(s.wins.right, (" ⯈ downstream %d "):format(#p.right))
  if not keep_center then
    s.rows = p.center
    local empty = s.loading and "· walking lineage… ·"
      or (s.nav.filter ~= "" and "· nothing matches /" .. s.nav.filter .. " ·")
      or "· no column lineage — the model may not parse (:DbtCompile) ·"
    fill(s, "center", p.center, empty)
    local title = rows.title(s.ix, s.nav) .. (s.note and ("  · " .. s.note) or "")
    ui.retitle(s.wins.center, " " .. title .. " ")
    for i, r in ipairs(p.center) do
      if r.id == p.current and vim.api.nvim_win_is_valid(s.wins.center) then
        s.syncing = true
        pcall(vim.api.nvim_win_set_cursor, s.wins.center, { i, 0 })
        s.syncing = false
      end
    end
  end
  schedule_preview(s)
end

--- The cursor moved in the middle pane: follow it.
function M._sync()
  local s = M._state
  if not s or s.syncing or not s.rows then
    return
  end
  local row = s.rows[vim.api.nvim_win_get_cursor(s.wins.center)[1]]
  if row and row.id ~= s.nav.cursor then
    s.nav = model.select(s.nav, row.id)
    M.render(s, true)
  end
end

--- h/l: walk to a side pane.
---@param side "left"|"right"
function M.walk(side)
  local s = M._state
  local nav, why = model.walk(s.ix, s.nav, side)
  if not nav then
    vim.notify(why, vim.log.levels.INFO)
    return
  end
  s.nav = nav
  M.render(s)
end

--- Set the filter and redraw.
function M.filter(text)
  local s = M._state
  s.nav = model.filtered(s.nav, text)
  M.render(s)
end

--- Walk again: `delta` hops deeper (or shallower), or as deep as now.
---@param delta integer?
function M.reload(delta)
  local s = M._state
  s.depth = math.max(1, s.depth + (delta or 0))
  if delta then
    vim.notify("column lineage depth " .. s.depth)
  end
  require("cogitator.column_dialog.load").load(M, s)
end

--- `D`: the same lineage drawn as the DAG graph (cogitator.dag), on the
--- column under the cursor.
function M.graph()
  local s = M._state
  local node = M.current()
  local start = (node and node.model) and { model = node.model, column = model.column_of(node) } or s.start
  M.close()
  if start then
    require("cogitator.dag").open({ columns = start })
  end
end

--- Open the dialog on `opts.model` (default: the current buffer's) and,
--- with `opts.column`, that column.
---@param opts {model?: string, column?: string}?
function M.open(opts)
  opts = opts or {}
  local src_buf, src_win = vim.api.nvim_get_current_buf(), vim.api.nvim_get_current_win()
  local client = actions.client_for(src_buf)
  if not client then
    return
  end
  M.close()
  local s = {
    client = client,
    src_buf = src_buf,
    src_win = src_win,
    wins = {},
    bufs = {},
    ix = model.index(nil),
    nav = model.start(opts.model or "?", nil),
    loading = true,
    files = {},
    compiled = {},
    compiled_uri = {},
    compiled_file = {},
    compiled_asked = {},
    shown = {},
    spots = {},
    marked_bufs = {},
    gen = 0,
    timer = vim.uv.new_timer(),
  }
  M._state = s
  local g = require("cogitator.explorer.render").geometry()
  local titles = {
    left = "upstream ⯇",
    center = "columns",
    right = "⯈ downstream",
    raw = "dbt SQL",
    compiled = "compiled SQL",
  }
  for _, name in ipairs(PANES) do
    local r = g[name]
    s.bufs[name], s.wins[name] = ui.float({
      row = r.row,
      col = r.col,
      width = r.width,
      height = r.height,
      title = titles[name],
      cursorline = name == "center",
    })
    require("cogitator.column_dialog.keys").attach(M, s.bufs[name])
  end
  pcall(vim.api.nvim_win_set_config, s.wins.center, { footer = FOOTER, footer_pos = "center" })
  for _, name in ipairs({ "raw", "compiled" }) do
    vim.wo[s.wins[name]].signcolumn = "yes:1"
    vim.wo[s.wins[name]].number = true
  end
  vim.api.nvim_set_current_win(s.wins.center)
  M.render(s)
  require("cogitator.column_dialog.keys").watch(M, s)

  require("cogitator.pick.graph").get(client, src_buf, function(project)
    if M._state ~= s then
      return
    end
    if not project or #project.nodes == 0 then
      M.close()
      return vim.notify(
        "dbt.projectGraph returned nothing — is the project loaded yet?",
        vim.log.levels.WARN
      )
    end
    local name = opts.model
    if not name then
      local here = require("cogitator.pick.graph").node_of_buffer(project, src_buf)
      if not (here and here.kind == "model") then
        M.close()
        return vim.notify("this buffer is not a model the project graph knows", vim.log.levels.WARN)
      end
      name = here.name
    end
    s.project = project
    s.start = { model = name, column = opts.column }
    s.nav = model.start(name, opts.column and ("model:%s::%s"):format(name, opts.column) or nil)
    require("cogitator.column_dialog.load").load(M, s)
  end)
end

return M
