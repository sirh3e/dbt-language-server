--- Project-wide answers as quickfix lists: downstream impact, dead
--- models and unresolved names (a selector's nodes are the navigator's:
--- cogitator.explorer.select).

local actions = require("cogitator.actions")

local client_for = actions.client_for

local M = {}

--- :DbtImpact [column] — downstream closure of this model, with
--- column-level hits when a column is given; quickfix list + summary.
function M.impact(opts)
  local bufnr = vim.api.nvim_get_current_buf()
  local client = client_for(bufnr)
  if not client then
    return
  end
  local column = opts and opts.args ~= "" and opts.args or nil
  client:exec_cmd({
    command = "dbt.impact",
    arguments = { { uri = vim.uri_from_bufnr(bufnr), column = column } },
  }, { bufnr = bufnr }, function(err, result)
    if err or type(result) ~= "table" then
      vim.notify("dbt.impact failed" .. (err and (": " .. err.message) or ""), vim.log.levels.ERROR)
      return
    end
    vim.notify(result.summary, vim.log.levels.INFO)
    local items = {}
    for _, node in ipairs(result.models) do
      items[#items + 1] = {
        filename = node.uri and node.uri ~= vim.NIL and vim.uri_to_fname(node.uri) or "",
        lnum = 1,
        col = 1,
        text = ("depth %d — %s"):format(node.depth, node.name),
      }
    end
    for _, hit in ipairs(result.columnHits or {}) do
      items[#items + 1] = { text = ("column hit: %s.%s"):format(hit.model, hit.column) }
    end
    if #items > 0 then
      vim.fn.setqflist({}, " ", { title = "dbt impact: " .. result.summary, items = items })
      vim.cmd.copen()
    end
  end)
end

--- :DbtDeadModels — models nothing uses, as a quickfix list.
function M.dead_models()
  local bufnr = vim.api.nvim_get_current_buf()
  local client = client_for(bufnr)
  if not client then
    return
  end
  client:exec_cmd({
    command = "dbt.deadModels",
    arguments = { { uri = vim.uri_from_bufnr(bufnr) } },
  }, { bufnr = bufnr }, function(err, result)
    if err or type(result) ~= "table" then
      vim.notify("dbt.deadModels failed" .. (err and (": " .. err.message) or ""), vim.log.levels.ERROR)
      return
    end
    if #result.models == 0 then
      vim.notify("no dead models — everything is referenced", vim.log.levels.INFO)
      return
    end
    local items = {}
    for _, dead in ipairs(result.models) do
      items[#items + 1] = {
        filename = dead.uri and vim.uri_to_fname(dead.uri) or "",
        lnum = 1,
        col = 1,
        text = dead.name .. " — " .. table.concat(dead.reasons, "; "),
      }
    end
    vim.fn.setqflist({}, " ", { title = "dbt dead models", items = items })
    vim.cmd.copen()
  end)
end

--- The quickfix items of a dbt.unresolved answer (exposed for the test
--- suite and the picker): one per unresolved name, at its place.
---@param result table
---@return table[]
function M.unresolved_items(result)
  local items = {}
  for _, u in ipairs(result.items or {}) do
    local start = type(u.range) == "table" and u.range.start or {}
    items[#items + 1] = {
      filename = u.uri and vim.uri_to_fname(u.uri) or "",
      lnum = (start.line or 0) + 1,
      col = (start.character or 0) + 1,
      text = ("[%s] %s — %s"):format(u.kind, u.name, u.message or ""),
      type = "W",
      user_data = { kind = u.kind, name = u.name },
    }
  end
  return items
end

--- :DbtUnresolved [kind…] — every unresolved ref, source, doc, macro and
--- var of the project, as a quickfix list.
---@param opts {fargs?: string[], args?: string}?
function M.unresolved(opts)
  local bufnr = vim.api.nvim_get_current_buf()
  local client = client_for(bufnr)
  if not client then
    return
  end
  local words = (opts and opts.fargs) or vim.split((opts and opts.args) or "", "%s+", { trimempty = true })
  local kinds = #words > 0 and words or nil
  client:exec_cmd({
    command = "dbt.unresolved",
    arguments = { { uri = vim.uri_from_bufnr(bufnr), kinds = kinds } },
  }, { bufnr = bufnr }, function(err, result)
    if err or type(result) ~= "table" then
      vim.notify("dbt.unresolved failed" .. (err and (": " .. err.message) or ""), vim.log.levels.ERROR)
      return
    end
    if result.count == 0 then
      vim.notify("nothing unresolved — every ref, source, doc, macro and var resolves", vim.log.levels.INFO)
      return
    end
    vim.fn.setqflist({}, " ", {
      title = ("dbt unresolved (%d)"):format(result.count),
      items = M.unresolved_items(result),
    })
    vim.cmd.copen()
  end)
end

--- :DbtSyncYaml [model…] [%] [prune] [types] [reorder] [folder] — the
--- schema-YAML sync (dbt.syncYaml): preview, then apply on confirm.
---@param opts {fargs?: string[], args?: string}?
function M.sync_yaml(opts)
  require("cogitator.sync_yaml").run(opts)
end

return M
