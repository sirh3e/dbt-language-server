--- The column-lineage dialog's data: dbt.columnLineageGraph through
--- cogitator.lineage_graph when the server answers it, else the client
--- walk over dbt.columnLineage (cogitator.column_lineage), whose progress
--- fills the panes as each lineage arrives. Nothing here blocks: every
--- answer lands in a callback, and one for an older load is dropped.

local actions = require("cogitator.actions")
local model = require("cogitator.column_dialog.model")

local M = {}

--- New data (a progress snapshot, or the whole answer).
local function set_data(dialog, s, data, start, done)
  s.ix = model.index(data)
  if start and not s.nav.cursor then
    s.nav.cursor = start
  end
  s.loading = not done
  dialog.render(s)
end

--- Fetch the graph: dbt.columnLineageGraph when the server answers it,
--- else the client walk.
function M.load(dialog, s)
  s.gen = s.gen + 1
  local gen, start = s.gen, s.start
  s.loading, s.note = true, nil
  local settings = require("cogitator.dag").settings().lineage
  local depth = math.max(1, s.depth or settings.depth)
  s.depth = depth
  local function live()
    return dialog._state == s and s.gen == gen
  end
  -- the walk takes every column of the model, so the list holds them all
  -- with the cursor on the asked one; a column the lineage does not list
  -- (select *, a declined template) is walked on its own after
  local function walk(from)
    local finished = false
    local function fetch(uri, cb)
      if s.client:is_stopped() then
        return cb(nil)
      end
      actions.exec_with(s.client, s.src_buf, "dbt.columnLineage", { uri = uri }, function(err, result)
        cb((not err and result ~= vim.NIL) and result or nil)
      end)
    end
    require("cogitator.column_lineage").walk(s.project, from, fetch, {
      depth = depth,
      max_requests = math.max(1, settings.max_requests),
      progress = function(partial, start_id)
        if live() and not finished then
          set_data(dialog, s, partial, start_id, false)
        end
      end,
    }, function(result)
      finished = true
      if not live() then
        return
      end
      local asked = start.column and ("model:%s::%s"):format(start.model, start.column)
      local found = not asked
        or vim.iter(result.data.nodes):any(function(n)
          return n.id == asked
        end)
      if not found and not from.column then
        return walk(start)
      end
      s.note = result.truncated and ("stopped after %d lineages"):format(result.requests) or nil
      set_data(dialog, s, result.data, result.start, true)
    end)
  end
  local ok, graph = pcall(require, "cogitator.lineage_graph")
  if ok and type(graph) == "table" and graph.supported and graph.supported(s.client) then
    graph.fetch(s.src_buf, { column = start.column, direction = "both", depth = depth }, function(result)
      if not live() then
        return
      end
      if type(result) == "table" and type(result.nodes) == "table" and #result.nodes > 0 then
        s.note = result.stale and "stale" or nil
        local id = start.column and ("model:%s::%s"):format(start.model, start.column) or nil
        set_data(dialog, s, result, id, true)
      else
        walk({ model = start.model })
      end
    end)
  else
    walk({ model = start.model })
  end
end

return M
