--- What the server worked out of a model: its breaking changes against
--- the last build, and which table models could be incremental.

local actions = require("cogitator.actions")

local client_for = actions.client_for

local M = {}

--- The quickfix items of a dbt.breakingChanges answer (exposed for the
--- test suite): one per removed or retyped column, then one per
--- downstream column that reads it.
---@param result table
---@return table[]
function M.breaking_items(result)
  local items = {}
  local renamed = {}
  for _, r in ipairs((result.diff or {}).removed or {}) do
    renamed[r.name] = r.possiblyRenamedTo
  end
  local kind = { error = "E", warning = "W", information = "I" }
  for _, impact in ipairs(result.impacts or {}) do
    local text = ("%s %s"):format(impact.column, impact.change)
    local to = renamed[impact.column]
    if to and to ~= vim.NIL then
      text = text .. (" (possibly renamed to %s)"):format(to)
    end
    if impact.tests and #impact.tests > 0 then
      text = text .. " — tested: " .. table.concat(impact.tests, ", ")
    end
    items[#items + 1] = { text = text, type = kind[impact.severity] or "W" }
    for _, use in ipairs(impact.downstream or {}) do
      local tags = {}
      if use.contracted then
        tags[#tags + 1] = "contracted"
      end
      if use.public then
        tags[#tags + 1] = "public"
      end
      items[#items + 1] = {
        text = ("  read by %s.%s%s"):format(
          use.model,
          use.column,
          #tags > 0 and (" (" .. table.concat(tags, ", ") .. ")") or ""
        ),
      }
    end
  end
  return items
end

--- :DbtBreakingChanges — this model's current columns against its last
--- built schema (target/catalog.json or manifest.json): removed, renamed-
--- looking and retyped columns, and what reads them downstream.
function M.breaking_changes()
  local bufnr = vim.api.nvim_get_current_buf()
  local client = client_for(bufnr)
  if not client then
    return
  end
  client:exec_cmd({
    command = "dbt.breakingChanges",
    arguments = { { uri = vim.uri_from_bufnr(bufnr) } },
  }, { bufnr = bufnr }, function(err, result)
    if err or type(result) ~= "table" then
      vim.notify("dbt.breakingChanges failed" .. (err and (": " .. err.message) or ""), vim.log.levels.ERROR)
      return
    end
    vim.notify(result.summary, vim.log.levels.INFO)
    local items = M.breaking_items(result)
    if #items > 0 then
      vim.fn.setqflist({}, " ", { title = "dbt breaking changes: " .. result.summary, items = items })
      vim.cmd.copen()
    end
  end)
end

--- The quickfix items of a dbt.incrementality report (exposed for the
--- test suite): every table model, as the server orders them.
---@param report table
---@return table[]
function M.incrementality_items(report)
  local items = {}
  for _, row in ipairs(report.models or {}) do
    local view = row.view or {}
    local why = view.summary
    if not why and type(view.reasons) == "table" and #view.reasons > 0 then
      why = table.concat(
        vim.tbl_map(function(r)
          return r.reason
        end, view.reasons),
        "; "
      )
    end
    local text = ("%s — %s%s"):format(
      row.model,
      view.verdict or "?",
      view.strategy and (" (" .. view.strategy .. ")") or ""
    )
    if type(row.lastRunSeconds) == "number" then
      text = text .. (" [last run %.1fs]"):format(row.lastRunSeconds)
    end
    if why then
      text = text .. ": " .. why
    end
    items[#items + 1] = {
      filename = row.uri and vim.uri_to_fname(row.uri) or "",
      lnum = 1,
      col = 1,
      text = text,
    }
  end
  return items
end

--- :DbtIncrementality — every table model and whether it could be
--- incremental, slowest last run first, as a quickfix list.
function M.incrementality()
  local bufnr = vim.api.nvim_get_current_buf()
  local client = client_for(bufnr)
  if not client then
    return
  end
  client:exec_cmd({
    command = "dbt.incrementality",
    arguments = { { uri = vim.uri_from_bufnr(bufnr) } },
  }, { bufnr = bufnr }, function(err, report)
    if err or type(report) ~= "table" then
      vim.notify("dbt.incrementality failed" .. (err and (": " .. err.message) or ""), vim.log.levels.ERROR)
      return
    end
    local items = M.incrementality_items(report)
    if #items == 0 then
      vim.notify("no table models in this project", vim.log.levels.INFO)
      return
    end
    vim.fn.setqflist({}, " ", {
      title = ("dbt incrementality: %d of %d could be incremental"):format(report.incremental or 0, #items),
      items = items,
    })
    vim.cmd.copen()
  end)
end

return M
