--- :DbtSplit — a table model split at its incrementality frontier.

local actions = require("cogitator.actions")

local client_for = actions.client_for

local M = {}

--- The lines of a dbt.splitPlan answer (exposed for the test suite): the
--- plan as an S-expression, then the new model's file when there is one.
---@param plan table
---@return string[]
function M.split_lines(plan)
  local lines = vim.split(plan.sexp or "", "\n")
  local part = type(plan.part) == "table" and plan.part or nil
  if part then
    table.insert(lines, "")
    table.insert(lines, (";; %s.sql — press a to split, q to close"):format(part.name))
    vim.list_extend(lines, vim.split((part.text or ""):gsub("\n$", ""), "\n"))
  end
  return lines
end

--- Applies the split code action in `bufnr`'s window.
---@param bufnr integer
function M.apply_split(bufnr)
  local win = vim.fn.bufwinid(bufnr)
  if win == -1 then
    vim.notify("the model's buffer is not shown in a window", vim.log.levels.WARN)
    return
  end
  vim.api.nvim_win_call(win, function()
    vim.lsp.buf.code_action({
      apply = true,
      context = { only = { "refactor.extract" }, diagnostics = {} },
      filter = function(action)
        return type(action.title) == "string" and action.title:find("^Split '") ~= nil
      end,
    })
  end)
end

--- :DbtSplit — this model split at its incrementality frontier
--- (dbt.splitPlan), as an S-expression and the new model's file; `a`
--- applies the split.
function M.split()
  local bufnr = vim.api.nvim_get_current_buf()
  local client = client_for(bufnr)
  if not client then
    return
  end
  client:exec_cmd({
    command = "dbt.splitPlan",
    arguments = { { uri = vim.uri_from_bufnr(bufnr) } },
  }, { bufnr = bufnr }, function(err, plan)
    if err then
      vim.notify("dbt.splitPlan failed: " .. err.message, vim.log.levels.ERROR)
      return
    end
    if type(plan) ~= "table" then
      vim.notify(
        "nothing to split: this model is not a table, or nothing in it keeps it one",
        vim.log.levels.INFO
      )
      return
    end
    vim.cmd.vsplit()
    local scratch = vim.api.nvim_create_buf(false, true)
    vim.api.nvim_buf_set_lines(scratch, 0, -1, false, M.split_lines(plan))
    vim.bo[scratch].filetype = "lisp"
    vim.bo[scratch].modifiable = false
    vim.api.nvim_win_set_buf(0, scratch)
    vim.keymap.set("n", "q", vim.cmd.close, { buffer = scratch, nowait = true })
    if type(plan.part) == "table" then
      vim.keymap.set("n", "a", function()
        vim.cmd.close()
        M.apply_split(bufnr)
      end, { buffer = scratch, nowait = true, desc = "apply the split" })
    end
  end)
end

return M
