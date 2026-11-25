--- Sources from the project graph (dbt.projectGraph, cached): models,
--- sources, every node, columns, a model's lineage and tags.

local common = require("cogitator.pick.sources.common")
local graph_lib = require("cogitator.pick.graph")
local paint = require("cogitator.pick.paint")
local preview = require("cogitator.pick.preview")

local M = {}

function M.models(ctx)
  return common.nodes(ctx, "models", { model = true })
end

function M.sources(ctx)
  return common.nodes(ctx, "sources", { source = true })
end

function M.all(ctx)
  return common.nodes(ctx, "nodes")
end

--- Every documented column of the project, as model.column.
function M.columns(ctx)
  return {
    title = "columns",
    empty = "no documented columns (columns: in the models' YAML)",
    items = function(done)
      graph_lib.get(ctx.client, ctx.bufnr, function(graph)
        local items = {}
        for _, node in ipairs(graph and graph.nodes or {}) do
          for _, column in ipairs(node.columns or {}) do
            local desc = column.description
            items[#items + 1] = {
              text = nil,
              name = node.name,
              uri = node.uri ~= vim.NIL and node.uri or nil,
              value = { node = node, column = column.name },
            }
            paint.paint(items[#items], {
              { node.name, paint.kind_group(node.kind, node.uri) },
              { "." },
              { column.name, "DbtPickMatchText" },
              { (desc and desc ~= vim.NIL and desc ~= "") and ("  — " .. desc) or "", "DbtPickDetail" },
            })
          end
        end
        done(items)
      end)
    end,
    preview = function(item, done)
      preview.file(
        item,
        { ("-- column %s of %s"):format(item.value.column, item.value.node.name) },
        function(p)
          -- land on the first line naming the column
          for i, line in ipairs(p.lines) do
            if i > 1 and line:find(item.value.column, 1, true) then
              p.lnum = i
              break
            end
          end
          done(p)
        end
      )
    end,
    choose = function(items, action)
      local column = items[1].value.column
      require("cogitator.pick.actions").open(items, action == "nav" and "default" or action)
      if action == "nav" then
        -- the column-lineage graph works on the current buffer's model
        require("cogitator.dag").open({ columns = true, column = column })
        return
      end
      vim.fn.search("\\V\\<" .. column .. "\\>", "w")
    end,
  }
end

--- The lineage of the buffer's model: "upstream", "downstream" or both.
---@param ctx cogitator.pick.Ctx
---@param direction "upstream"|"downstream"|"lineage"
function M.lineage(ctx, direction)
  return {
    title = direction,
    empty = "nothing " .. direction .. " of this model (or it is not a model)",
    items = function(done)
      graph_lib.get(ctx.client, ctx.bufnr, function(graph)
        local here = graph and graph_lib.node_of_buffer(graph, ctx.bufnr)
        if not here then
          return done({})
        end
        local by_id = {}
        for _, node in ipairs(graph.nodes) do
          by_id[node.id] = node
        end
        local parents, children = graph_lib.adjacency(graph)
        local items = {}
        local function add(edges, arrow)
          for _, step in ipairs(graph_lib.walk(here.id, edges)) do
            local node = by_id[step.id]
            if node then
              items[#items + 1] = common.node_item(node, nil, {
                {
                  ("%s%d "):format(arrow, step.depth),
                  arrow == "↑" and "DbtPickUpstream" or "DbtPickDownstream",
                },
              })
            end
          end
        end
        if direction ~= "downstream" then
          add(parents, "↑")
        end
        if direction ~= "upstream" then
          add(children, "↓")
        end
        done(items)
      end)
    end,
    preview = common.node_preview,
  }
end

--- The ref of one of dbt.columnLineage's `sources` as picker parts and
--- the graph node it lives in.
---@param ref table
---@param by_id table<string, table>
---@return {[1]: string, [2]: string?}[] parts
---@return table? node
local function ref_parts(ref, by_id)
  if ref.kind == "model" then
    local node = by_id["model:" .. ref.name]
    return {
      { ref.name, paint.kind_group("model", node and node.uri) },
      { "." },
      { ref.column, "DbtPickMatchText" },
    },
      node
  elseif ref.kind == "source" then
    local node = by_id["source:" .. ref.source] or by_id["source:" .. ref.source .. "." .. (ref.table or "")]
    return {
      { ref.source .. "." .. (ref.table or ""), paint.KIND_GROUP.source },
      { "." },
      { ref.column, "DbtPickMatchText" },
    },
      node
  end
  return {
    { (ref.name or "?") .. "." .. (ref.column or "?"), "DbtPickDetail" },
    { "  (unresolved)", "DbtPickDetail" },
  },
    nil
end

--- The column lineage of the buffer's model (dbt.columnLineage, which the
--- server computes once per edit and keeps): a line per output column and
--- each column it reads; <CR> jumps to that column, `nav` opens the
--- column graph. `on_none` runs when the server has no lineage for it.
---@param ctx cogitator.pick.Ctx
function M.column_lineage(ctx)
  return {
    title = "column lineage",
    empty = "no column lineage for this model",
    items = function(done)
      graph_lib.get(ctx.client, ctx.bufnr, function(graph)
        local by_id = {}
        for _, node in ipairs(graph and graph.nodes or {}) do
          by_id[node.id] = node
        end
        require("cogitator.actions").exec_with(
          ctx.client,
          ctx.bufnr,
          "dbt.columnLineage",
          { uri = vim.uri_from_bufnr(ctx.bufnr) },
          function(err, lineage)
            local items = {}
            if err or type(lineage) ~= "table" or type(lineage.columns) ~= "table" then
              if ctx.on_none then
                ctx.on_none()
              end
              return done(items)
            end
            local here = graph and graph_lib.node_of_buffer(graph, ctx.bufnr)
            for _, entry in ipairs(lineage.columns) do
              local sources = entry.sources or {}
              if #sources == 0 then
                items[#items + 1] = paint.paint(
                  {
                    name = entry.name,
                    value = { column = entry.name },
                    uri = vim.uri_from_bufnr(ctx.bufnr),
                  },
                  {
                    { entry.name, "DbtPickMatchText" },
                    { "  ←  ", "DbtPickArrow" },
                    { "(computed here)", "DbtPickDetail" },
                  }
                )
              end
              for _, ref in ipairs(sources) do
                local parts, node = ref_parts(ref, by_id)
                table.insert(parts, 1, { "  ←  ", "DbtPickArrow" })
                table.insert(parts, 1, { entry.name, paint.kind_group("model", here and here.uri) })
                items[#items + 1] = paint.paint({
                  name = ref.name or ref.source,
                  uri = node and node.uri ~= vim.NIL and node.uri or nil,
                  value = { column = ref.column, out = entry.name, ref = ref, node = node },
                }, parts)
              end
            end
            done(items)
          end
        )
      end)
    end,
    preview = function(item, done)
      preview.file(
        item,
        { ("-- %s ← %s"):format(item.value.out or item.value.column, item.value.column) },
        function(p)
          for i, line in ipairs(p.lines) do
            if i > 1 and line:find(item.value.column, 1, true) then
              p.lnum = i
              break
            end
          end
          done(p)
        end
      )
    end,
    choose = function(items, action)
      local item = items[1]
      if action == "nav" then
        require("cogitator.dag").open({ columns = true, column = item.value.out or item.value.column })
        return
      end
      if not item.uri then
        vim.notify("no file for " .. item.text, vim.log.levels.INFO)
        return
      end
      require("cogitator.pick.actions").open(items, action)
      vim.fn.search("\\V\\<" .. item.value.column .. "\\>", "w")
    end,
  }
end

function M.upstream(ctx)
  return M.lineage(ctx, "upstream")
end

function M.downstream(ctx)
  return M.lineage(ctx, "downstream")
end

--- Tags, then the nodes carrying the chosen one.
function M.tags(ctx)
  return {
    title = "tags",
    empty = "no tags in this project (tags: in YAML or {{ config(tags=…) }})",
    items = function(done)
      graph_lib.get(ctx.client, ctx.bufnr, function(graph)
        local by_tag = {}
        for _, node in ipairs(graph and graph.nodes or {}) do
          for _, tag in ipairs(node.tags or {}) do
            by_tag[tag] = by_tag[tag] or {}
            table.insert(by_tag[tag], node)
          end
        end
        local items = {}
        for tag, nodes in pairs(by_tag) do
          items[#items + 1] = paint.paint({
            name = "tag:" .. tag,
            value = nodes,
          }, {
            { "#" .. tag, "DbtPickTag" },
            { ("  (%d node%s)"):format(#nodes, #nodes == 1 and "" or "s"), "DbtPickDetail" },
          })
        end
        table.sort(items, function(a, b)
          return a.text < b.text
        end)
        done(items)
      end)
    end,
    preview = function(item, done)
      local lines = {}
      for _, node in ipairs(item.value) do
        lines[#lines + 1] = node.name
      end
      table.sort(lines)
      done({ lines = lines })
    end,
    choose = function(items, action)
      if action == "run" or action == "yank" then
        return require("cogitator.pick.actions").run({}, action, items)
      end
      local nodes = items[1].value
      require("cogitator.pick").open({
        title = items[1].text:match("^(#%S+)"),
        items = function(done)
          local out = {}
          for _, node in ipairs(nodes) do
            out[#out + 1] = common.node_item(node)
          end
          done(out)
        end,
        preview = common.node_preview,
      })
    end,
  }
end

return M
