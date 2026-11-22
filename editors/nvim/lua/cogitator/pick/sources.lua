--- What the pickers list: every source is a function of the buffer it was
--- opened from, returning a cogitator.pick.Source. Items come from
--- dbt.projectGraph (cached, cogitator.pick.graph), workspace/symbol and
--- the buffer itself; nothing here waits on the server.

local actions = require("cogitator.actions")
local graph_lib = require("cogitator.pick.graph")
local preview = require("cogitator.pick.preview")

local M = {}

---@class cogitator.pick.Ctx
---@field client vim.lsp.Client
---@field bufnr integer

local KIND_TAG = {
  model = "model",
  source = "source",
  exposure = "exposure",
  metric = "metric",
  semantic_model = "semantic",
  test = "test",
  analysis = "analysis",
}

--- A graph node as a picker item.
local function node_item(node, label)
  local tags = (node.tags and #node.tags > 0) and ("  #" .. table.concat(node.tags, " #")) or ""
  return {
    text = (label or node.name) .. "  [" .. (KIND_TAG[node.kind] or node.kind) .. "]" .. tags,
    name = node.name,
    uri = node.uri ~= vim.NIL and node.uri or nil,
    value = node,
  }
end

--- A node's file, under a header saying what it is.
local function node_preview(item, done)
  local node = item.value
  local header = { ("-- %s %s"):format(node.kind, node.name) }
  if node.tags and #node.tags > 0 then
    header[#header + 1] = "-- tags: " .. table.concat(node.tags, ", ")
  end
  if node.columns and #node.columns > 0 then
    local names = {}
    for _, c in ipairs(node.columns) do
      names[#names + 1] = c.name
    end
    header[#header + 1] = "-- columns: " .. table.concat(names, ", ")
  end
  preview.file(item, header, done)
end

--- Nodes of the kinds in `kinds` (all when nil).
---@param ctx cogitator.pick.Ctx
---@param title string
---@param kinds table<string, boolean>?
function M.nodes(ctx, title, kinds)
  return {
    title = title,
    empty = "no " .. title .. " in this project yet — is the project loaded?",
    items = function(done)
      graph_lib.get(ctx.client, ctx.bufnr, function(graph)
        local items = {}
        for _, node in ipairs(graph and graph.nodes or {}) do
          if not kinds or kinds[node.kind] then
            items[#items + 1] = node_item(node)
          end
        end
        table.sort(items, function(a, b)
          return a.name < b.name
        end)
        done(items)
      end)
    end,
    preview = node_preview,
  }
end

function M.models(ctx)
  return M.nodes(ctx, "models", { model = true })
end

function M.sources(ctx)
  return M.nodes(ctx, "sources", { source = true })
end

function M.all(ctx)
  return M.nodes(ctx, "nodes")
end

local SYMBOL_KINDS = { [5] = "model", [11] = "source", [12] = "macro", [20] = "selector", [24] = "exposure" }

--- workspace/symbol, all of it, matched here (the server matches substrings).
---@param ctx cogitator.pick.Ctx
---@param title string
---@param keep fun(symbol: table): boolean
local function symbols(ctx, title, keep, query)
  return {
    title = title,
    empty = "no " .. title .. " in this project",
    items = function(done)
      actions.request(
        ctx.client,
        ctx.bufnr,
        "workspace/symbol",
        { query = query or "" },
        { timeout_ms = 15000 },
        function(err, result)
          local items = {}
          for _, sym in ipairs((not err and type(result) == "table") and result or {}) do
            if keep(sym) then
              local loc = sym.location or {}
              local start = loc.range and loc.range.start
              items[#items + 1] = {
                text = sym.name .. "  [" .. (SYMBOL_KINDS[sym.kind] or "symbol") .. "]",
                name = sym.name,
                uri = loc.uri,
                lnum = start and (start.line + 1) or nil,
                col = start and start.character or nil,
                value = sym,
              }
            end
          end
          done(items)
        end
      )
    end,
    preview = function(item, done)
      preview.file(item, nil, done)
    end,
  }
end

--- Every model, source, macro and selector: jump to its definition.
function M.definitions(ctx)
  return symbols(ctx, "definitions", function()
    return true
  end)
end

function M.macros(ctx)
  return symbols(ctx, "macros", function(sym)
    return sym.kind == 12
  end)
end

function M.selectors(ctx)
  return symbols(ctx, "selectors", function()
    return true
  end, "selector:")
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
              items[#items + 1] = node_item(node, ("%s%d %s"):format(arrow, step.depth, node.name))
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
    preview = node_preview,
  }
end

function M.upstream(ctx)
  return M.lineage(ctx, "upstream")
end

function M.downstream(ctx)
  return M.lineage(ctx, "downstream")
end

--- The ref()/source() calls and the CTEs and macros of the buffer: go to
--- what each one names.
function M.refs(ctx)
  local bufnr = ctx.bufnr
  return {
    title = "references in this file",
    empty = "no ref(), source() or CTE in this file",
    items = function(done)
      local items = {}
      local lines = vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)
      local patterns = {
        { "ref%(%s*['\"]([^'\"]+)['\"]", "ref" },
        { "source%(%s*['\"]([^'\"]+)['\"]%s*,%s*['\"]([^'\"]+)['\"]", "source" },
      }
      for lnum, line in ipairs(lines) do
        for _, p in ipairs(patterns) do
          local init = 1
          while true do
            local s, _, a, b = line:find(p[1], init)
            if not s then
              break
            end
            local name = b and (a .. "." .. b) or a
            items[#items + 1] = {
              text = ("%s(%s)  :%d"):format(p[2], name, lnum),
              name = b or a,
              lnum = lnum,
              col = (line:find(a, s, true) or s) - 1,
              value = { kind = p[2] },
            }
            init = s + 1
          end
        end
      end
      actions.request(ctx.client, bufnr, "textDocument/documentSymbol", {
        textDocument = { uri = vim.uri_from_bufnr(bufnr) },
      }, { timeout_ms = 5000 }, function(err, result)
        for _, sym in ipairs((not err and type(result) == "table") and result or {}) do
          local range = (sym.location or sym).range
          items[#items + 1] = {
            text = ("%s  [%s]  :%d"):format(
              sym.name,
              sym.kind == 12 and "macro" or "cte",
              range.start.line + 1
            ),
            name = sym.name,
            lnum = range.start.line + 1,
            col = range.start.character,
            value = { kind = "local" },
          }
        end
        table.sort(items, function(a, b)
          return a.lnum < b.lnum
        end)
        done(items)
      end)
    end,
    preview = function(item, done)
      local lines = vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)
      done({ lines = lines, filetype = vim.bo[bufnr].filetype, lnum = item.lnum })
    end,
    choose = function(items, action)
      local item = items[1]
      local win = vim.fn.bufwinid(bufnr)
      if win == -1 then
        return
      end
      vim.api.nvim_set_current_win(win)
      pcall(vim.api.nvim_win_set_cursor, win, { item.lnum, item.col or 0 })
      if item.value.kind == "local" then
        return
      end
      if action == "default" then
        vim.lsp.buf.definition()
      elseif action == "split" or action == "vsplit" or action == "tab" then
        vim.cmd(({ split = "split", vsplit = "vsplit", tab = "tab split" })[action])
        vim.lsp.buf.definition()
      elseif action == "nav" then
        require("cogitator.explorer").open({ focus = item.name })
      end
    end,
  }
end

--- The tests of the buffer's model: jump to their YAML.
function M.tests(ctx)
  return {
    title = "tests",
    empty = "no tests defined for this model (:DbtExtract scaffolds the YAML)",
    items = function(done)
      actions.exec_with(
        ctx.client,
        ctx.bufnr,
        "dbt.modelTests",
        { uri = vim.uri_from_bufnr(ctx.bufnr) },
        function(err, result)
          local items = {}
          for _, t in ipairs((not err and type(result) == "table" and result.tests) or {}) do
            local label = t.name
            if t.column and t.column ~= vim.NIL then
              label = label .. " on " .. t.column
            end
            if t.version and t.version ~= vim.NIL then
              label = label .. (" (v%d)"):format(t.version)
            end
            local start = t.range and t.range ~= vim.NIL and t.range.start
            items[#items + 1] = {
              text = label,
              name = t.name,
              uri = t.uri,
              lnum = start and (start.line + 1) or nil,
              col = start and start.character or nil,
            }
          end
          done(items)
        end
      )
    end,
    preview = function(item, done)
      preview.file(item, nil, done)
    end,
  }
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
            out[#out + 1] = node_item(node)
          end
          done(out)
        end,
        preview = node_preview,
      })
    end,
  }
end

--- The sources a :DbtFind with no argument offers, in this order.
M.ORDER = {
  "models",
  "sources",
  "all",
  "definitions",
  "macros",
  "columns",
  "upstream",
  "downstream",
  "lineage",
  "refs",
  "tests",
  "tags",
  "selectors",
}

M.DESCRIPTIONS = {
  models = "every model",
  sources = "every source",
  all = "every node: models, sources, exposures, metrics…",
  definitions = "models, sources, macros and selectors — jump to the definition",
  macros = "every macro",
  columns = "every documented column, as model.column",
  upstream = "everything upstream of this model, nearest first",
  downstream = "everything downstream of this model, nearest first",
  lineage = "upstream and downstream of this model",
  refs = "the ref(), source(), CTEs and macros of this file",
  tests = "the tests of this model",
  tags = "tags, then the nodes carrying one",
  selectors = "selectors.yml entries",
}

--- Build the named source for the current buffer.
---@param name string
---@param ctx cogitator.pick.Ctx
---@return table
function M.build(name, ctx)
  local make = M[name]
  if name == "lineage" then
    return M.lineage(ctx, "lineage")
  end
  return make(ctx)
end

return M
