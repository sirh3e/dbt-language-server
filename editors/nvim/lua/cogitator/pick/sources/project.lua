--- Sources from the server's project-wide commands: a model's tests,
--- what is unresolved, a selector's nodes and ast-grep's matches.

local actions = require("cogitator.actions")
local preview = require("cogitator.pick.preview")

local M = {}

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
              end_lnum = start and (start.line + 1) or nil,
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

--- Every unresolved ref, source, doc, macro and var (dbt.unresolved).
function M.unresolved(ctx)
  return {
    title = "unresolved",
    empty = "nothing unresolved — every ref, source, doc, macro and var resolves",
    items = function(done)
      actions.exec_with(
        ctx.client,
        ctx.bufnr,
        "dbt.unresolved",
        { uri = vim.uri_from_bufnr(ctx.bufnr) },
        function(err, result)
          local items = {}
          local rows = (not err and type(result) == "table")
              and require("cogitator.commands").unresolved_items(result)
            or {}
          for _, row in ipairs(rows) do
            items[#items + 1] = {
              text = row.text,
              name = row.user_data.name,
              path = row.filename,
              lnum = row.lnum,
              end_lnum = row.lnum,
              col = row.col - 1,
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

--- dbt's node selection (`:DbtFind select`): every node, and dbt
--- selections, evaluated as typed (cogitator.pick.select_source).
function M.select(ctx)
  return require("cogitator.pick.select_source").source(ctx)
end

--- The matches of an ast-grep pattern (ctx.query, asked for first).
function M.astgrep(ctx)
  return require("cogitator.astgrep").source(ctx)
end

return M
