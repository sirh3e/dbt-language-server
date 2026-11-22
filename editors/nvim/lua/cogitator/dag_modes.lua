--- What :DbtDag shows: the whole model graph, the neighbourhood of the
--- selection (focus, ±depth hops), or a column-lineage graph walked from
--- dbt.columnLineage. Also the view's two autocmds: follow the buffer
--- entered elsewhere, and redraw after a save.

local actions = require("cogitator.actions")
local column_lineage = require("cogitator.column_lineage")
local dag_data = require("cogitator.dag_data")

local M = {}

local MAX_DEPTH = 10

--- The model id behind a node of either mode.
---@param node table?
---@return string?
local function model_id(node)
  if not node then
    return nil
  end
  if node.model then
    return "model:" .. node.model
  end
  return node.id
end

--- The model graph, whole or focused on `around`.
---@param dag table cogitator.dag
---@param around string? the node to centre a focus on
function M.models(dag, around)
  local s = dag._state
  local selected = around or model_id(s.data and s.boxes[s.selected or ""] and s.boxes[s.selected].node)
  s.mode = "models"
  s.start = nil
  s.note = nil
  local data = s.project
  if s.focused and selected then
    data = dag_data.focus(s.project, selected, s.depth)
  end
  dag.show(data, selected)
end

--- The column graph around a model (every column) or one of its columns.
---@param dag table cogitator.dag
---@param start {model: string, column?: string}
function M.columns(dag, start)
  local s = dag._state
  local settings = dag.settings().lineage
  local client, src_buf = s.client, s.src_buf
  s.note = "walking lineage…"
  dag.winbar()
  local function fetch(uri, cb)
    if client:is_stopped() then
      return cb(nil)
    end
    actions.exec_with(client, src_buf, "dbt.columnLineage", { uri = uri }, function(err, result)
      cb((not err and result ~= vim.NIL) and result or nil)
    end)
  end
  column_lineage.walk(s.project, start, fetch, {
    depth = math.max(1, s.lineage_depth or settings.depth),
    max_requests = math.max(1, settings.max_requests),
  }, function(result)
    if dag._state ~= s then
      return
    end
    if #result.data.nodes == 0 then
      s.note = nil
      dag.winbar()
      vim.notify(
        ("no column lineage for %s%s — the model may not parse to a query plan (run :DbtCompile)"):format(
          start.model,
          start.column and ("." .. start.column) or ""
        ),
        vim.log.levels.WARN
      )
      return
    end
    s.mode = "columns"
    s.start = start
    s.note = result.truncated and ("stopped after %d lineages"):format(result.requests) or nil
    dag.show(result.data, result.start)
  end)
end

--- Redraw whatever mode is on screen from a fresh project payload.
---@param dag table
function M.redraw(dag)
  local s = dag._state
  if s.mode == "columns" and s.start then
    M.columns(dag, s.start)
  else
    M.models(dag, s.selected and s.boxes[s.selected] and s.selected or nil)
  end
end

--- The column a cursor sits on inside an expanded box, if any.
---@param dag table
---@return string?
function M.column_at_cursor(dag)
  local s = dag._state
  local box = dag.box_at_cursor()
  if not box then
    return nil
  end
  local row = vim.api.nvim_win_get_cursor(s.win)[1]
  local line = box.lines[row - box.y]
  return line and line:match("^• (.+)$") or nil
end

--- L: in the model graph, the lineage of the model under the cursor (of
--- the one column when the cursor is on an expanded column line); in a
--- column graph, re-centre the walk on the column under the cursor.
---@param dag table
function M.lineage_at_cursor(dag)
  local node = dag.node_at_cursor()
  if not node then
    return
  end
  if node.model and node.column then
    return M.columns(dag, { model = node.model, column = node.column })
  end
  if node.kind ~= "model" then
    vim.notify("column lineage follows models; this is a " .. node.kind, vim.log.levels.INFO)
    return
  end
  M.columns(dag, { model = node.name, column = M.column_at_cursor(dag) })
end

--- f: show only the selection's neighbourhood, or everything again.
---@param dag table
function M.toggle_focus(dag)
  local s = dag._state
  if s.mode ~= "models" then
    return M.models(dag)
  end
  s.focused = not s.focused
  M.models(dag)
end

--- +/-: widen or narrow the focus (turning it on).
---@param dag table
---@param delta integer
function M.change_depth(dag, delta)
  local s = dag._state
  if s.mode == "columns" then
    -- the column graph's depth is the walk's: re-walk at the new depth
    local from = s.lineage_depth or dag.settings().lineage.depth
    s.lineage_depth = math.max(1, math.min(MAX_DEPTH, from + delta))
    vim.notify("column lineage depth " .. s.lineage_depth)
    return M.columns(dag, s.start)
  end
  s.depth = math.max(1, math.min(MAX_DEPTH, s.depth + delta))
  s.focused = true
  M.models(dag)
end

--- F: follow the model of the buffer entered in another window.
---@param dag table
function M.toggle_follow(dag)
  local s = dag._state
  s.follow = not s.follow
  dag.winbar()
end

--- The view's autocmds: follow BufEnter, redraw on BufWritePost.
---@param dag table
---@param group_name string
---@param settings {refresh_on_save: boolean}
function M.watch(dag, group_name, settings)
  local s = dag._state
  local group = vim.api.nvim_create_augroup(group_name, { clear = true })
  vim.api.nvim_create_autocmd("BufEnter", {
    group = group,
    callback = function(args)
      local state = dag._state
      if not (state and state.follow and state.mode == "models") or args.buf == state.buf then
        return
      end
      local id = dag.node_of_buffer(state.project, args.buf)
      if not id then
        return
      end
      if state.focused then
        M.models(dag, id)
      elseif state.boxes[id] then
        dag.select(id, { record = false })
      end
    end,
  })
  if settings.refresh_on_save then
    vim.api.nvim_create_autocmd("BufWritePost", {
      group = group,
      pattern = { "*.sql", "*.yml", "*.yaml" },
      callback = function()
        if dag._state == s then
          dag.refresh()
        end
      end,
    })
  end
  vim.api.nvim_create_autocmd("BufWipeout", {
    group = group,
    buffer = s.buf,
    callback = function()
      if dag._state == s then
        dag._state = nil
        pcall(vim.api.nvim_del_augroup_by_name, group_name)
      end
    end,
  })
end

return M
