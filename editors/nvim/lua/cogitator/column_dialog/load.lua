--- The column-lineage dialog's data: dbt.columnLineageGraph through
--- cogitator.lineage_graph, the server's walk over its column graph.
--- Nothing here blocks: the answer lands in a callback, and one for an
--- older load is dropped.

local model = require("cogitator.column_dialog.model")

local M = {}

--- New data (the whole answer).
local function set_data(dialog, s, data, start, done)
  s.ix = model.index(data)
  if start and not s.nav.cursor then
    s.nav.cursor = start
  end
  s.loading = not done
  dialog.render(s)
end

--- Fetch the graph. The whole model is asked first, so the list holds
--- every column with the cursor on the asked one; a column that answer
--- does not hold (nothing reads it, a declined template) is asked alone.
function M.load(dialog, s)
  -- looked up per load, so a test can stand in for it
  local lineage_graph = require("cogitator.lineage_graph")
  s.gen = s.gen + 1
  local gen, start = s.gen, s.start
  s.loading, s.note = true, nil
  local settings = require("cogitator.dag").settings().lineage
  local depth = math.max(1, s.depth or settings.depth)
  s.depth = depth
  local asked = start.column and ("model:%s::%s"):format(start.model, start.column) or nil
  local function ask(column)
    lineage_graph.fetch(s.src_buf, {
      uri = lineage_graph.model_uri(s.project, start.model),
      column = column,
      direction = "both",
      depth = depth,
    }, function(err, result)
      if dialog._state ~= s or s.gen ~= gen then
        return
      end
      if err or type(result) ~= "table" or type(result.nodes) ~= "table" then
        s.note = "no column lineage"
        return set_data(dialog, s, { nodes = {}, edges = {} }, nil, true)
      end
      local found = not asked
        or vim.iter(result.nodes):any(function(n)
          return n.id == asked
        end)
      if not found and column == nil then
        return ask(start.column)
      end
      s.note = lineage_graph.note(result)
      set_data(dialog, s, { nodes = result.nodes, edges = result.edges }, asked or result.start, true)
    end)
  end
  ask(nil)
end

return M
