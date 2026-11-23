--- :DbtPreviewCte [limit] — the rows of the CTE under the cursor (or of
--- the final select past every CTE) via dbt.previewCte: the model run up
--- to that CTE through `dbt show --inline`, shown as a table in a split.

local actions = require("cogitator.actions")

local M = {}

--- The split's lines for a dbt.previewCte answer (exposed for the tests):
--- the server's markdown table, which already names the model, the CTE,
--- the row count and the query.
---@param result table
---@return string[]
function M.lines(result)
  local text = type(result.table) == "string" and result.table or ""
  return vim.split(text:gsub("\n$", ""), "\n")
end

---@param opts? {args?: string}
function M.preview_cte(opts)
  local bufnr = vim.api.nvim_get_current_buf()
  local client = actions.client_for(bufnr)
  if not client then
    return
  end
  local pos = vim.api.nvim_win_get_cursor(0)
  local arg = {
    uri = vim.uri_from_bufnr(bufnr),
    position = { line = pos[1] - 1, character = pos[2] },
  }
  local limit = tonumber(opts and opts.args or "")
  if limit then
    arg.limit = limit
  end
  client:exec_cmd(
    { command = "dbt.previewCte", arguments = { arg } },
    { bufnr = bufnr },
    function(err, result)
      if err or type(result) ~= "table" then
        vim.notify("dbt.previewCte failed" .. (err and (": " .. err.message) or ""), vim.log.levels.ERROR)
        return
      end
      vim.cmd.split()
      local scratch = vim.api.nvim_create_buf(false, true)
      vim.api.nvim_buf_set_lines(scratch, 0, -1, false, M.lines(result))
      vim.bo[scratch].filetype = "markdown"
      vim.bo[scratch].modifiable = false
      vim.api.nvim_win_set_buf(0, scratch)
      vim.keymap.set("n", "q", vim.cmd.close, { buffer = scratch, nowait = true })
    end
  )
end

return M
