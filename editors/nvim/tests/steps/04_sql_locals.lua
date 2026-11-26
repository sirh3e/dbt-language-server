-- The SQL names of one model: a table alias, a CTE and a column alias.
-- Go to definition on each lands on its declaration in its own scope,
-- and :DbtReferences on the alias lists its uses in the yazi dialog.

return function(ctx)
  local H = ctx.H
  local picker = require("cogitator.picker")
  local usages = require("cogitator.usages")

  local path = vim.fs.joinpath(ctx.fixture, "models", "nav_probe.sql")
  vim.fn.writefile({
    "with paid as (",
    "    select order_id, amount as total from {{ ref('stg_payments') }}",
    ")",
    "select o.order_id, p.total as paid_total",
    "from {{ ref('stg_orders') }} o",
    "join paid p on p.order_id = o.order_id",
    "where exists (select 1 from paid o where o.total > 0)",
    "order by total",
  }, path)
  vim.cmd.edit(path)
  local buf = vim.api.nvim_get_current_buf()
  H.wait("sql locals: the server attaches to the probe", 10000, function()
    return require("cogitator.actions").client_for(buf, true) ~= nil
  end)

  -- the cursor at `{row, col}` (1-based row, 0-based col), then the LSP's
  -- definition; it lands where `want` says
  local function goes(label, from, want)
    vim.api.nvim_set_current_buf(buf)
    vim.api.nvim_win_set_cursor(0, from)
    vim.lsp.buf.definition()
    H.wait(label, 5000, function()
      local at = vim.api.nvim_win_get_cursor(0)
      return at[1] == want[1] and at[2] == want[2]
    end, function()
      return vim.inspect(vim.api.nvim_win_get_cursor(0))
    end)
  end
  goes("sql locals: gd on an alias lands on `from … o`", { 4, 7 }, { 5, 29 })
  goes("sql locals: gd on the subquery's `o` lands on its own alias", { 7, 41 }, { 7, 33 })
  goes("sql locals: gd on a CTE use lands on its name", { 6, 5 }, { 1, 5 })
  goes("sql locals: gd on a CTE column lands on `as total`", { 4, 21 }, { 2, 31 })

  -- :DbtReferences on the outer `o`: its two qualified uses, one file
  vim.api.nvim_win_set_cursor(0, { 4, 7 })
  usages.references({})
  H.wait("sql locals: :DbtReferences opens the dialog", 15000, function()
    return picker._state ~= nil
  end)
  local root = picker._state.stack[1].items
  local probe
  for _, it in ipairs(root) do
    if it.label:find("nav_probe.sql", 1, true) then
      probe = it
    end
  end
  H.check(
    probe ~= nil and #probe.children == 2,
    "sql locals: the dialog lists the alias's two uses",
    vim.inspect(vim.tbl_map(function(i)
      return { i.label, i.children and #i.children }
    end, root))
  )
  picker.close()
  H.wait("sql locals: the dialog closes", 3000, function()
    return picker._state == nil
  end)
  vim.cmd("bwipeout! " .. buf)
  vim.fn.delete(path)
end
