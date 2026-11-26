--- Structural search over the models' SQL, through the server
--- (dbt.search): `joins stg_orders on customer_id`, `select-star from
--- source`, `reads source:shop.orders`, `uses column orders.amount`,
--- combined with `and`. The matches have dbt.astGrep's shape, so they
--- reuse its quickfix items.

local actions = require("cogitator.actions")

local M = {}

--- The one-line summary of a dbt.search answer (pure, for the tests).
---@param result table
---@return string
function M.summary(result)
  local n = #(result.matches or {})
  local models = #(result.models or {})
  local skipped = #(result.notSearched or {})
  local text = ("%d match%s in %d model%s (%d searched)"):format(
    n,
    n == 1 and "" or "es",
    models,
    models == 1 and "" or "s",
    result.searched or 0
  )
  if skipped > 0 then
    text = text .. ("; %d not searched"):format(skipped)
  end
  if result.truncated then
    text = text .. " (the first 500)"
  end
  return text
end

--- Why each model was not searched, as quickfix-less lines.
---@param result table
---@return string[]
function M.not_searched_lines(result)
  local lines = {}
  for _, n in ipairs(result.notSearched or {}) do
    lines[#lines + 1] = ("%s: %s"):format(n.model, n.why)
  end
  return lines
end

--- :DbtSearch {query}
---@param opts {args: string}
function M.command(opts)
  local query = vim.trim(opts.args or "")
  if query == "" then
    vim.notify("usage: :DbtSearch joins stg_orders on customer_id   (docs/search.md)", vim.log.levels.INFO)
    return
  end
  local bufnr = vim.api.nvim_get_current_buf()
  local client = actions.client_for(bufnr)
  if not client then
    return
  end
  actions.exec_with(client, bufnr, "dbt.search", { uri = "", query = query }, function(err, result)
    if err or type(result) ~= "table" then
      vim.notify("dbt.search: " .. (err and err.message or "no answer"), vim.log.levels.ERROR)
      return
    end
    local items = vim.tbl_map(require("cogitator.astgrep").item, result.matches or {})
    vim.fn.setqflist({}, " ", { title = "dbt search: " .. query, items = items })
    local skipped = M.not_searched_lines(result)
    local text = "dbt search: " .. M.summary(result)
    if #skipped > 0 then
      text = text .. "\n" .. table.concat(skipped, "\n")
    end
    vim.notify(text, vim.log.levels.INFO)
    if #items > 0 then
      vim.cmd.copen()
    end
  end)
end

return M
