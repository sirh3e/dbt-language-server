-- Column completion against the live server on the fixture COPY: the
-- relations in scope's columns come before everything global, an
-- ambiguous name qualified first, by sortText — the order Neovim's
-- built-in completion (vim.lsp.completion) and the cmp comparator keep.
-- The probe buffer is never written.

return function(ctx)
  local H = ctx.H
  local probe = vim.fs.joinpath(ctx.fixture, "models", "completion_probe.sql")
  vim.cmd.edit(probe)
  local bufnr = vim.api.nvim_get_current_buf()
  vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, {
    "select  from {{ ref('orders') }} o join {{ ref('customers') }} c on o.customer_id = c.customer_id",
  })
  H.wait("completion: the probe attaches", 10000, function()
    return #vim.lsp.get_clients({ bufnr = bufnr, name = "cogitator" }) == 1
  end)
  local client = vim.lsp.get_clients({ bufnr = bufnr, name = "cogitator" })[1]
  if not client then
    return
  end

  local result = H.request(client, bufnr, "textDocument/completion", {
    textDocument = { uri = vim.uri_from_bufnr(bufnr) },
    position = { line = 0, character = 7 },
  }, "completion: select list")
  local items = result and (result.items or result) or {}
  table.sort(items, function(a, b)
    return (a.sortText or a.label) < (b.sortText or b.label)
  end)
  local labels = vim.tbl_map(function(i)
    return i.label
  end, vim.list_slice(items, 1, 3))
  H.check(
    vim.deep_equal(labels, { "c.customer_id", "o.customer_id", "amount" }),
    "completion: alias.column of an ambiguous name first",
    vim.inspect(labels)
  )
  local first_global
  for i, item in ipairs(items) do
    if item.kind ~= vim.lsp.protocol.CompletionItemKind.Field then
      first_global = i
      break
    end
  end
  local qualified_last = 0
  for i, item in ipairs(items) do
    if item.label:find("^[oc]%.") then
      qualified_last = i
    end
  end
  H.check(
    first_global ~= nil and qualified_last > 0 and qualified_last < first_global,
    "completion: every alias.column before the first global",
    vim.inspect({ first_global = first_global, qualified_last = qualified_last })
  )
  H.check(items[1] and items[1].preselect == true, "completion: the best column is preselected")

  -- Neovim's own conversion: a typed `cus` keeps the qualified forms
  -- (they filter as their column) and sortText still decides
  local convert = vim.lsp.completion and vim.lsp.completion._lsp_to_complete_items
  if convert then
    local words = vim.tbl_map(function(i)
      return i.word
    end, convert({ items = items }, "cus", client.id))
    H.check(
      words[1] == "c.customer_id" and words[2] == "o.customer_id",
      "completion: vim.lsp.completion keeps the order for a typed prefix",
      vim.inspect(vim.list_slice(words, 1, 4))
    )
  end

  -- the cmp comparator orders cogitator's items by sortText too
  local dbt = require("cogitator.completion")
  local entry = function(item)
    return { completion_item = item, source = { source = { client = { name = "cogitator" } } } }
  end
  H.check(
    dbt.compare(entry(items[1]), entry(items[#items])) == true,
    "completion: the cmp comparator ranks the column first"
  )

  -- blink.cmp: the columns in scope get a score lift, globals do not
  local blinked = dbt.blink_transform()({}, {
    { client_name = "cogitator", sortText = items[1].sortText },
    { client_name = "cogitator", sortText = "zselect" },
    { client_name = "other", sortText = "0x" },
  })
  H.check(
    blinked[1].score_offset == dbt.BLINK_BOOST
      and blinked[2].score_offset == nil
      and blinked[3].score_offset == nil,
    "completion: blink_transform lifts the in-scope columns only",
    vim.inspect(blinked)
  )

  vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, {
    "select o. from {{ ref('orders') }} o join {{ ref('customers') }} c on true",
  })
  local member = H.request(client, bufnr, "textDocument/completion", {
    textDocument = { uri = vim.uri_from_bufnr(bufnr) },
    position = { line = 0, character = 9 },
  }, "completion: after o.")
  member = member and (member.items or member) or {}
  local only_orders = #member > 0
  for _, item in ipairs(member) do
    if item.label == "first_name" or item.kind ~= vim.lsp.protocol.CompletionItemKind.Field then
      only_orders = false
    end
  end
  H.check(only_orders, "completion: after `o.` only the orders columns", tostring(#member))

  vim.bo[bufnr].modified = false
  vim.cmd.bwipeout({ bufnr, bang = true })
  vim.api.nvim_set_current_buf(ctx.bufnr)
end
