-- Feature round-trips on the booted session: commands, code actions,
-- the dangling-ref quickfix and server-side formatting.

return function(ctx)
  local H, client, bufnr = ctx.H, ctx.client, ctx.bufnr

  local graph_result = H.exec_cmd(client, bufnr, "dbt.modelGraph", {
    uri = vim.uri_from_bufnr(bufnr),
  }, "integration: dbt.modelGraph")
  local up_names = {}
  if type(graph_result) == "table" and graph_result.upstream then
    up_names = vim.tbl_map(function(e)
      return e.name
    end, graph_result.upstream)
  end
  H.check(
    type(graph_result) == "table"
      and graph_result.model == "customers"
      and vim.tbl_contains(up_names, "stg_customers"),
    "integration: dbt.modelGraph lists direct upstream",
    vim.inspect(graph_result):sub(1, 200)
  )

  local relation_result = H.exec_cmd(client, bufnr, "dbt.relation", {
    uri = vim.uri_from_bufnr(bufnr),
  }, "integration: dbt.relation")
  H.check(
    type(relation_result) == "table"
      and relation_result.kind == "model"
      and relation_result.relation == "jaffle_shop.main.customers",
    "integration: dbt.relation resolves database.schema.identifier",
    vim.inspect(relation_result):sub(1, 200)
  )

  -- :DbtRelation yanks into the unnamed register
  vim.api.nvim_set_current_buf(bufnr)
  vim.fn.setreg('"', "")
  require("cogitator.commands").relation()
  H.wait("integration: :DbtRelation yanks", 10000, function()
    return vim.fn.getreg('"') ~= ""
  end)
  H.check(
    vim.fn.getreg('"') == "jaffle_shop.main.customers",
    "integration: :DbtRelation yanks database.schema.identifier",
    vim.fn.getreg('"')
  )

  local cache_result = H.exec_cmd(client, bufnr, "dbt.cache", {
    uri = vim.uri_from_bufnr(bufnr),
    action = "stats",
  }, "integration: dbt.cache")
  H.check(
    type(cache_result) == "table"
      and type(cache_result.profiles) == "table"
      and type(cache_result.profiles.entries) == "number"
      and type(cache_result.lineage.hits) == "number",
    "integration: dbt.cache reports the derived-cache surfaces",
    vim.inspect(cache_result):sub(1, 200)
  )

  -- :DbtQuery: no DB configured headless → the bounded SELECT lands in
  -- a scratch sql buffer
  require("cogitator.commands").query()
  local query_buf
  H.wait("integration: :DbtQuery scratch buffer", 10000, function()
    for _, b in ipairs(vim.api.nvim_list_bufs()) do
      if vim.api.nvim_buf_is_loaded(b) and vim.bo[b].filetype == "sql" then
        local text = table.concat(vim.api.nvim_buf_get_lines(b, 0, -1, false), "\n")
        if text:find("select %* from jaffle_shop%.main%.customers limit 100", 1, false) then
          query_buf = b
          return true
        end
      end
    end
    return false
  end)
  H.check(query_buf ~= nil, "integration: :DbtQuery builds the bounded SELECT in a scratch buffer")
  vim.api.nvim_set_current_buf(bufnr)
  if query_buf then
    vim.api.nvim_buf_delete(query_buf, { force = true })
  end

  -- code actions: the executable lens set as command-only actions
  local actions = H.request(client, bufnr, "textDocument/codeAction", {
    textDocument = { uri = vim.uri_from_bufnr(bufnr) },
    range = { start = { line = 0, character = 0 }, ["end"] = { line = 0, character = 0 } },
    context = { diagnostics = {} },
  }, "integration: code actions answer") or {}
  local ca_titles = vim.tbl_map(function(a)
    return a.title
  end, actions)
  H.check(
    vim.tbl_contains(ca_titles, "dbt run") and vim.tbl_contains(ca_titles, "Go to schema"),
    "integration: code actions offer the dbt command set",
    vim.inspect(ca_titles)
  )

  -- did-you-mean quickfix on a probe buffer (fixture COPY — never saved)
  vim.cmd.edit(vim.fs.joinpath(ctx.fixture, "models", "quickfix_probe.sql"))
  local fix_buf = vim.api.nvim_get_current_buf()
  vim.api.nvim_buf_set_lines(fix_buf, 0, -1, false, { "select * from {{ ref('customer') }}" })
  H.wait("quickfix: client attaches to the probe buffer", 15000, function()
    return #vim.lsp.get_clients({ bufnr = fix_buf, name = "cogitator" }) > 0
  end)
  -- Neovim's built-in pull diagnostics CANCELS any concurrent manual
  -- textDocument/diagnostic for a changing buffer (a cancelled
  -- buf_request_sync just times out), so ride the builtin: wait for
  -- the diagnostic to land in vim.diagnostic and take the LSP payload
  -- from user_data.
  local fix_diag
  H.wait("quickfix: dangling-ref diagnostic published", 15000, function()
    for _, d in ipairs(vim.diagnostic.get(fix_buf)) do
      local lsp_d = d.user_data and d.user_data.lsp
      if lsp_d and lsp_d.code == "dangling-ref" then
        fix_diag = lsp_d
        return true
      end
    end
    return false
  end)
  H.check(
    fix_diag ~= nil and fix_diag.data ~= nil and fix_diag.data.suggestions[1] == "customers",
    "quickfix: dangling-ref diagnostic carries typed suggestions",
    vim.inspect(fix_diag)
  )
  local fix_action
  if fix_diag then
    local fixes = H.request(client, fix_buf, "textDocument/codeAction", {
      textDocument = { uri = vim.uri_from_bufnr(fix_buf) },
      range = fix_diag.range,
      context = { diagnostics = { fix_diag }, only = { "quickfix" } },
    }, "quickfix: codeAction answers") or {}
    for _, a in ipairs(fixes) do
      if a.kind == "quickfix" then
        fix_action = a
      end
    end
  end
  H.check(
    fix_action ~= nil
      and fix_action.edit ~= nil
      and fix_action.edit.documentChanges[1].edits[1].newText == "customers",
    "quickfix: codeAction answers an edit-bearing quickfix",
    vim.inspect(fix_action)
  )
  vim.api.nvim_set_current_buf(bufnr)
  vim.api.nvim_buf_delete(fix_buf, { force = true })

  -- formatting: reflow through the server, Jinja untouched
  vim.cmd.edit(vim.fs.joinpath(ctx.fixture, "models", "fmt_probe.sql"))
  local fmt_buf = vim.api.nvim_get_current_buf()
  vim.api.nvim_buf_set_lines(fmt_buf, 0, -1, false, {
    "select a,b,   c from {{ ref('stg_customers') }} where a>1",
  })
  H.wait("format: client attaches to the probe buffer", 15000, function()
    return #vim.lsp.get_clients({ bufnr = fmt_buf, name = "cogitator" }) > 0
  end)
  vim.lsp.buf.format({ bufnr = fmt_buf, timeout_ms = 10000 })
  local fmt_text = table.concat(vim.api.nvim_buf_get_lines(fmt_buf, 0, -1, false), "\n")
  H.check(
    fmt_text:find("{{ ref('stg_customers') }}", 1, true) ~= nil and fmt_text:find("\n") ~= nil,
    "format: server reflows SQL and keeps the Jinja expression",
    fmt_text:sub(1, 120)
  )
  vim.api.nvim_set_current_buf(bufnr)
  vim.api.nvim_buf_delete(fmt_buf, { force = true })
end
