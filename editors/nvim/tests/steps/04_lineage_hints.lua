-- inline column lineage: with the plugin's default inlay_hints = true the
-- buffer's inlay hints are on, and after the server's
-- workspace/inlayHint/refresh the column lineage hints are what Neovim
-- shows: `← stg_customers.customer_id` after `customers.customer_id,`,
-- each upstream column a label part with a location.

return function(ctx)
  local H, bufnr = ctx.H, ctx.bufnr
  vim.api.nvim_set_current_buf(bufnr)
  H.check(
    vim.lsp.inlay_hint.is_enabled({ bufnr = bufnr }),
    "lineage hints: inlay hints are on for a dbt buffer by default"
  )
  local function lineage_hint(needle)
    for _, item in ipairs(vim.lsp.inlay_hint.get({ bufnr = bufnr })) do
      local label = item.inlay_hint.label
      if type(label) == "table" then
        local text = table.concat(vim.tbl_map(function(p)
          return p.value
        end, label))
        if text:sub(1, #"←") == "←" and text:find(needle, 1, true) then
          return item.inlay_hint, text
        end
      end
    end
  end
  local hint, text
  H.wait("lineage hints: Neovim shows them after the refresh", 15000, function()
    -- a redraw asks again for what is in view, as typing would
    vim.cmd("redraw")
    hint, text = lineage_hint("stg_customers.customer_id")
    return hint ~= nil
  end)
  if not hint then
    return
  end
  local line = vim.api.nvim_buf_get_lines(bufnr, hint.position.line, hint.position.line + 1, false)[1]
  H.check(
    line:find("customers.customer_id,", 1, true) ~= nil,
    "lineage hints: the hint sits on the column's line",
    line .. " | " .. text
  )
  local part = hint.label[2]
  H.check(
    part.location and part.location.uri:find("stg_customers.sql", 1, true) ~= nil,
    "lineage hints: the upstream column's part points at its model",
    vim.inspect(part)
  )
end
