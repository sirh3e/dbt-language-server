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

  local site_result = H.exec_cmd(client, bufnr, "dbt.lineageSite", {
    uri = vim.uri_from_bufnr(bufnr),
    write = true,
  }, "integration: dbt.lineageSite")
  H.check(
    type(site_result) == "table"
      and type(site_result.written) == "table"
      and vim.uv.fs_stat(site_result.written.path) ~= nil
      and site_result.counts.columnsTraced > 0,
    "integration: dbt.lineageSite writes the page under target/",
    vim.inspect(site_result):sub(1, 200)
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

  -- the dependencies dialog's answer: direct and transitive, each with
  -- how it is built (models/ is a table, staging/ a view)
  local deps = H.exec_cmd(client, bufnr, "dbt.modelDependencies", {
    uri = vim.uri_from_bufnr(bufnr),
  }, "integration: dbt.modelDependencies")
  local by_name = {}
  for _, node in ipairs(type(deps) == "table" and deps.parents or {}) do
    by_name[node.name] = node
  end
  H.check(
    type(deps) == "table"
      and deps.model.name == "customers"
      and deps.model.config.materialized == "table"
      and by_name.stg_customers
      and by_name.stg_customers.config.materialized == "view"
      and by_name.stg_customers.depth == 1
      and #deps.upstream >= #deps.parents,
    "integration: dbt.modelDependencies says what customers uses and how each is built",
    vim.inspect(deps):sub(1, 600)
  )
  -- the columns come with the project's schema job, which follows the refresh
  local typed
  H.wait("integration: dbt.modelDependencies carries column types", 20000, function()
    local answer = H.exec_cmd(client, bufnr, "dbt.modelDependencies", {
      uri = vim.uri_from_bufnr(bufnr),
    }, "integration: dbt.modelDependencies (types)")
    typed = type(answer) == "table" and answer.model.columns
    return type(typed) == "table" and #typed > 0
  end)
  local bases = {}
  for _, c in ipairs(type(typed) == "table" and typed or {}) do
    bases[c.basis] = (bases[c.basis] or 0) + 1
  end
  H.check(
    type(typed) == "table" and #typed > 0 and typed[1].name ~= nil and typed[1].basis ~= nil,
    "integration: customers' columns come with a type and a basis",
    vim.inspect(bases)
  )
  deps = H.exec_cmd(
    client,
    bufnr,
    "dbt.modelDependencies",
    { uri = vim.uri_from_bufnr(bufnr) },
    "integration: deps again"
  )
  local items = require("cogitator.deps").items(deps)
  H.check(
    #items == 6
      and items[2].label == "Columns"
      and items[3].label == "Uses directly"
      and #items[3].children == #deps.parents,
    "integration: the dialog is the model, then direct and transitive both ways",
    vim.inspect(vim.tbl_map(function(i)
      return i.label
    end, items))
  )

  -- definition and hover from the word `ref`, not only from its argument
  local model_lines = vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)
  local ref_line, ref_col
  for i, l in ipairs(model_lines) do
    local s = l:find("ref%(")
    if s then
      ref_line, ref_col = i - 1, s
      break
    end
  end
  local pos = {
    textDocument = { uri = vim.uri_from_bufnr(bufnr) },
    position = { line = ref_line, character = ref_col },
  }
  local def = client:request_sync("textDocument/definition", pos, 10000, bufnr)
  local def_uri = def and def.result and (def.result.uri or (def.result[1] and def.result[1].uri))
  H.check(
    type(def_uri) == "string" and def_uri:find("stg_", 1, true) ~= nil,
    "integration: definition on the word ref goes to the model it names",
    vim.inspect(def and def.result)
  )
  local hover = client:request_sync("textDocument/hover", pos, 10000, bufnr)
  local contents = hover and hover.result and hover.result.contents
  contents = type(contents) == "table" and (contents.value or "") or (contents or "")
  local link = require("cogitator.hover_jump").link_in(vim.split(contents, "\n"))
  H.check(
    contents:find("^%*%*model%*%*") ~= nil and link and link.uri == def_uri,
    "integration: the hover on ref names the model and links its definition",
    contents
  )

  -- in a hover float holding that link, <CR> opens the definition
  local fbuf = vim.api.nvim_create_buf(false, true)
  vim.api.nvim_buf_set_lines(fbuf, 0, -1, false, vim.split(contents, "\n"))
  local fwin =
    vim.api.nvim_open_win(fbuf, true, { relative = "editor", row = 1, col = 1, width = 60, height = 6 })
  H.wait("integration: the hover float gets its keys", 3000, function()
    return vim.b[fbuf].cogitator_hover_jump == true
  end)
  vim.api.nvim_feedkeys(vim.api.nvim_replace_termcodes("<CR>", true, false, true), "x", false)
  H.check(
    not vim.api.nvim_win_is_valid(fwin) and vim.uri_from_bufnr(0) == def_uri,
    "integration: <CR> in the hover closes it and opens the definition",
    vim.uri_from_bufnr(0)
  )
  vim.api.nvim_set_current_buf(bufnr)

  local incr_result = H.exec_cmd(client, bufnr, "dbt.incrementality", {
    uri = vim.uri_from_bufnr(bufnr),
  }, "integration: dbt.incrementality")
  H.check(
    type(incr_result) == "table"
      and type(incr_result.models) == "table"
      and type(incr_result.incremental) == "number",
    "integration: dbt.incrementality lists the table models",
    vim.inspect(incr_result):sub(1, 200)
  )
  local split_result, split_null = H.exec_cmd(client, bufnr, "dbt.splitPlan", {
    uri = vim.uri_from_bufnr(bufnr),
  }, "integration: dbt.splitPlan")
  H.check(
    split_null or (type(split_result) == "table" and type(split_result.sexp) == "string"),
    "integration: dbt.splitPlan answers a plan or null",
    vim.inspect(split_result):sub(1, 200)
  )
  -- customers.sql calls a package macro the preview cannot stub; the
  -- staging model renders, and has no is_incremental() to differ on
  local diff_result = H.exec_cmd(client, bufnr, "dbt.incrementalDiff", {
    uri = vim.uri_from_fname(vim.fs.joinpath(ctx.fixture, "models", "staging", "stg_customers.sql")),
  }, "integration: dbt.incrementalDiff")
  H.check(
    type(diff_result) == "table"
      and type(diff_result.fullRefresh) == "string"
      and type(diff_result.incremental) == "string"
      and diff_result.identical == true,
    "integration: dbt.incrementalDiff renders both ways",
    vim.inspect(diff_result):sub(1, 200)
  )

  local unresolved_result = H.exec_cmd(client, bufnr, "dbt.unresolved", {
    uri = vim.uri_from_bufnr(bufnr),
  }, "integration: dbt.unresolved")
  H.check(
    type(unresolved_result) == "table"
      and type(unresolved_result.items) == "table"
      and type(unresolved_result.count) == "number",
    "integration: dbt.unresolved lists the project's unresolved names",
    vim.inspect(unresolved_result):sub(1, 200)
  )

  local sync_result = H.exec_cmd(client, bufnr, "dbt.syncYaml", {
    models = { "stg_customers" },
  }, "integration: dbt.syncYaml")
  H.check(
    type(sync_result) == "table"
      and sync_result.applied == false
      and type(sync_result.files) == "table"
      and sync_result.models[1].model == "stg_customers"
      and sync_result.models[1].added[1] == "first_name",
    "integration: dbt.syncYaml plans without applying",
    vim.inspect(sync_result):sub(1, 300)
  )

  local select_result = H.exec_cmd(client, bufnr, "dbt.select", {
    uri = vim.uri_from_bufnr(bufnr),
    selector = "tag:nightly",
  }, "integration: dbt.select")
  H.check(
    type(select_result) == "table"
      and type(select_result.nodes) == "table"
      and select_result.count >= 1
      and select_result.nodes[1].name == "customers",
    "integration: dbt.select picks the nightly model",
    vim.inspect(select_result):sub(1, 200)
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
    -- the did-you-mean fix: since #37 the dangling ref also offers
    -- quickfixes that create a file (documentChanges[1] is a CreateFile),
    -- so pick the first one whose first change is a text edit
    for _, a in ipairs(fixes) do
      local first = a.kind == "quickfix" and a.edit and a.edit.documentChanges and a.edit.documentChanges[1]
      if not fix_action and first and first.edits then
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

  -- an invalid model as it is written: the problem, its suggestion
  -- inline, and :Dbt fix applying it (fixture COPY — never saved)
  vim.cmd.edit(vim.fs.joinpath(ctx.fixture, "models", "invalid_probe.sql"))
  local bad_buf = vim.api.nvim_get_current_buf()
  vim.api.nvim_buf_set_lines(bad_buf, 0, -1, false, {
    "{{ config(materialized='incrementl') }}",
    "select * from {{ ref('stg_customers') }}",
  })
  H.wait("invalid model: client attaches to the probe buffer", 15000, function()
    return #vim.lsp.get_clients({ bufnr = bad_buf, name = "cogitator" }) > 0
  end)
  local bad
  H.wait("invalid model: invalid-materialization published", 15000, function()
    for _, d in ipairs(vim.diagnostic.get(bad_buf)) do
      local lsp_d = d.user_data and d.user_data.lsp
      if lsp_d and lsp_d.code == "invalid-materialization" then
        bad = d
        return true
      end
    end
    return false
  end)
  local diagnostics = require("cogitator.diagnostics")
  H.check(
    bad ~= nil and bad.severity == vim.diagnostic.severity.ERROR,
    "invalid model: a misspelt materialization is an error",
    vim.inspect(bad)
  )
  H.check(
    bad ~= nil and diagnostics.inline(bad):find("did you mean 'incremental'?", 1, true) ~= nil,
    "invalid model: the inline text carries the suggestion",
    bad and diagnostics.inline(bad)
  )
  if bad then
    vim.api.nvim_win_set_cursor(0, { bad.lnum + 1, bad.col })
    require("cogitator.dispatch").SUBCOMMANDS.fix.run("")
  end
  H.wait("invalid model: :Dbt fix applies the suggestion", 15000, function()
    return (vim.api.nvim_buf_get_lines(bad_buf, 0, 1, false)[1] or ""):find(
      "materialized='incremental'",
      1,
      true
    ) ~= nil
  end)
  vim.api.nvim_buf_set_lines(bad_buf, 0, -1, false, { "select * from {{ ref('invalid_probe') }}" })
  H.wait("invalid model: a self ref is reported", 15000, function()
    for _, d in ipairs(vim.diagnostic.get(bad_buf)) do
      if d.user_data and d.user_data.lsp and d.user_data.lsp.code == "ref-self" then
        return true
      end
    end
    return false
  end)
  vim.api.nvim_win_set_cursor(0, { 1, 23 })
  require("cogitator.dispatch").SUBCOMMANDS.problems.run("")
  local loc = vim.fn.getloclist(0, { title = 0, items = 0 })
  H.check(
    loc.title == "dbt problems · invalid_probe.sql"
      and #loc.items > 0
      and loc.items[1].text:find("[ref-self]", 1, true) ~= nil,
    ":Dbt problems: the file's problems in the location list",
    vim.inspect(loc)
  )
  vim.cmd.lclose()
  vim.api.nvim_set_current_buf(bufnr)
  vim.api.nvim_buf_delete(bad_buf, { force = true })

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
