--- Sources from the project graph (dbt.projectGraph, cached): models,
--- sources, every node, columns, a model's lineage and tags.

local common = require("cogitator.pick.sources.common")
local graph_lib = require("cogitator.pick.graph")
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
              text = node.name
                .. "."
                .. column.name
                .. ((desc and desc ~= vim.NIL and desc ~= "") and ("  — " .. desc) or ""),
              name = node.name,
              uri = node.uri ~= vim.NIL and node.uri or nil,
              value = { node = node, column = column.name },
            }
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
              items[#items + 1] = common.node_item(node, ("%s%d %s"):format(arrow, step.depth, node.name))
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
          items[#items + 1] = {
            text = ("#%s  (%d node%s)"):format(tag, #nodes, #nodes == 1 and "" or "s"),
            name = "tag:" .. tag,
            value = nodes,
          }
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
