-- Where things are used and renaming them (cogitator.usages,
-- cogitator.rename): the pure grouping, previews and summaries first,
-- then :DbtReferences!, :DbtMacros and :DbtRename against the live
-- server on the fixture COPY. A rename is previewed and cancelled, never
-- applied, so the steps after this one see the project unchanged.

return function(ctx)
  local H, bufnr = ctx.H, ctx.bufnr
  local picker = require("cogitator.picker")
  local usages = require("cogitator.usages")
  local rename = require("cogitator.rename")

  -- the yazi dialogs filter with fzf's syntax
  H.check(picker.matches({ label = "stg_customers" }, "stgcus"), "usages: the dialog filter is fuzzy")
  H.check(not picker.matches({ label = "stg_customers" }, "!^stg"), "usages: the dialog filter negates")
  H.check(
    picker.matches({ label = "orders", detail = "models/x.sql" }, "x.sql$ 'ord"),
    "usages: exact and suffix"
  )
  H.check(
    picker.matches({ label = "a.sql", children = { { label = "9: from {{ ref('b') }}" } } }, "'ref('b')"),
    "usages: a file matches through its lines"
  )

  -- the grouping of locations
  local files = {
    ["/p/models/b.sql"] = { "select 1", "from {{ ref('a') }}", "join {{ ref('a') }} x" },
    ["/p/models/schema.yml"] = { "models:", "  - to: ref('a')" },
  }
  local loc = function(path, line, char)
    return { uri = vim.uri_from_fname(path), range = { start = { line = line, character = char } } }
  end
  local items = usages.items({
    loc("/p/models/schema.yml", 1, 14),
    loc("/p/models/b.sql", 2, 13),
    loc("/p/models/b.sql", 1, 13),
  }, {
    root = "/p",
    read = function(p)
      return files[p]
    end,
  })
  H.check(
    #items == 2 and items[1].label == "models/b.sql" and items[2].label == "models/schema.yml",
    "usages: a group per file, in path order, relative to the root",
    vim.inspect(vim.tbl_map(function(i)
      return i.label
    end, items))
  )
  H.check(
    items[1].detail == "2 uses" and items[1].children[1].label == "2: from {{ ref('a') }}",
    "usages: each use is its line, in line order",
    vim.inspect(items[1].children[1])
  )
  H.check(
    items[1].children[2].value.lnum == 3 and items[1].children[2].value.col == 13,
    "usages: a use opens at its line and column"
  )
  local shown = usages.preview_lines({ "a", "b", "c" }, 2)
  H.check(
    vim.deep_equal(shown, { "  a", "▶ b", "  c" }),
    "usages: the preview marks the use",
    vim.inspect(shown)
  )
  H.check(
    usages.name_position("{% macro full_name(a, b) %}", "full_name") == 9,
    "usages: a macro's name on its header"
  )
  H.check(usages.name_position("{% macro full_name_x() %}", "full_name") == nil, "usages: not a longer name")
  local macro_items = usages.macro_items({
    { name = "zeta", kind = 12, location = { uri = "file:///p/macros/z.sql", range = {} } },
    { name = "stg", kind = 5, location = { uri = "file:///p/models/stg.sql", range = {} } },
    { name = "alpha", kind = 12, location = { uri = "file:///p/macros/a.sql", range = {} } },
  }, "/p")
  H.check(
    #macro_items == 2 and macro_items[1].label == "alpha" and macro_items[1].detail == "macros/a.sql",
    "usages: the macro list keeps macros, by name"
  )

  -- the rename's summary and confirmation items
  local edit = {
    documentChanges = {
      { textDocument = { uri = "file:///p/models/b.sql" }, edits = { {}, {} } },
      { textDocument = { uri = "file:///p/models/schema.yml" }, edits = { {} } },
      { kind = "rename", oldUri = "file:///p/models/a.sql", newUri = "file:///p/models/c.sql" },
    },
  }
  local s = rename.summary(edit)
  H.check(
    s.files == 2 and s.edits == 3 and #s.renames == 1,
    "rename: the summary counts files, edits, renames"
  )
  local confirm = rename.items(edit, "a", "c", "utf-16")
  H.check(
    confirm[1].value == "apply"
      and confirm[1].detail == "3 edits in 2 files, 1 file rename"
      and confirm[2].value == "cancel"
      and #confirm[3].children == 3,
    "rename: apply, cancel, then a diff per change",
    vim.inspect(confirm[1])
  )
  -- a column renamed from upstream: the second item, before cancel
  local both = rename.upstream({
    model = "b",
    origin = { model = "a", column = "x", chain = { "b", "a" } },
    edit = edit,
  }, nil, "utf-16")
  local with_both = rename.items(edit, "x", "y", "utf-16", both)
  H.check(
    with_both[2].value == "apply_both"
      and with_both[2].label == "apply up- and downstream: from a.x"
      and with_both[2].detail:find("b ← a", 1, true) ~= nil
      and with_both[3].value == "cancel",
    "rename: up- and downstream is offered second",
    vim.inspect(with_both[2])
  )
  H.check(
    rename.upstream({ model = "a", origin = { model = "a", column = "x" }, edit = edit }, nil, "utf-16")
        == nil
      and rename.upstream(vim.NIL, nil, "utf-16") == nil
      and rename.upstream(nil, { message = "other" }, "utf-16") == nil,
    "rename: nothing upstream, no item"
  )
  local refused =
    rename.upstream(nil, { message = "`b.x` cannot be traced upstream: why.\nSolution: …" }, "utf-16")
  H.check(
    refused ~= nil and refused.disabled == "`b.x` cannot be traced upstream: why.",
    "rename: a refused climb says why, disabled",
    vim.inspect(refused)
  )

  -- :DbtReferences! from stg_orders: its call sites, grouped by file
  vim.cmd.edit(vim.fs.joinpath(ctx.fixture, "models", "staging", "stg_orders.sql"))
  local stg = vim.api.nvim_get_current_buf()
  H.wait("usages: the server attaches to stg_orders.sql", 10000, function()
    return require("cogitator.actions").client_for(stg, true) ~= nil
  end)
  usages.references({ bang = true })
  H.wait("usages: :DbtReferences! opens the dialog", 15000, function()
    return picker._state ~= nil
  end)
  local st = picker._state
  local root = st.stack[1].items
  H.check(root[1].value and root[1].value.rename, "usages: the dialog's first entry renames")
  local customers
  for _, it in ipairs(root) do
    if it.label:find("customers.sql", 1, true) then
      customers = it
    end
  end
  H.check(
    customers ~= nil,
    "usages: customers.sql uses stg_orders",
    vim.inspect(vim.tbl_map(function(i)
      return i.label
    end, root))
  )
  H.check(
    customers and customers.children[1].label:find("ref('stg_orders')", 1, true) ~= nil,
    "usages: the use is the ref() line",
    customers and customers.children[1].label
  )
  picker.set_filter("'customers.sql")
  H.feed("l")
  H.feed("<CR>")
  H.wait("usages: choosing a use opens its file", 5000, function()
    return vim.api.nvim_buf_get_name(0):find("customers.sql", 1, true) ~= nil
  end, function()
    return vim.api.nvim_buf_get_name(0)
  end)
  local row = vim.api.nvim_win_get_cursor(0)[1]
  H.check(
    vim.api.nvim_get_current_line():find("ref('stg_orders')", 1, true) ~= nil,
    "usages: … at the line of the use",
    row
  )

  -- :DbtMacros: every macro, then the callers of the chosen one
  usages.macros()
  H.wait("usages: :DbtMacros opens the dialog", 15000, function()
    return picker._state ~= nil
  end)
  picker.set_filter("'full_name")
  H.check(
    picker._state.stack[1].shown[1] and picker._state.stack[1].shown[1].label == "full_name",
    "usages: the macro list filters to full_name"
  )
  H.feed("<CR>")
  H.wait("usages: choosing a macro lists its callers", 15000, function()
    return picker._state ~= nil and picker._state.opts.title:find("full_name", 1, true) ~= nil
  end)
  local callers = picker._state.stack[1].items
  local called = false
  for _, it in ipairs(callers) do
    called = called or it.label:find("customers.sql", 1, true) ~= nil
  end
  H.check(called, "usages: full_name is called in customers.sql")
  picker.close()

  -- :DbtRename! previews the model rename and cancels: nothing changes
  vim.api.nvim_set_current_buf(stg)
  rename.rename({ bang = true, args = "stg_orders_v2" })
  H.wait("rename: the confirmation dialog opens", 15000, function()
    return picker._state ~= nil and picker._state.opts.title:find("stg_orders_v2", 1, true) ~= nil
  end)
  local first = picker._state.stack[1].items[1]
  H.check(
    first.value == "apply" and first.detail:find("file rename", 1, true) ~= nil,
    "rename: the model rename renames its file",
    first.detail
  )
  local diff = table.concat(first.preview().lines, "\n")
  H.check(diff:find("+.*ref%('stg_orders_v2'%)") ~= nil, "rename: the preview shows the new ref()", diff)
  H.feed("j<CR>")
  H.check(picker._state == nil, "rename: cancel closes the dialog")
  H.check(
    vim.uv.fs_stat(vim.fs.joinpath(ctx.fixture, "models", "staging", "stg_orders.sql")) ~= nil
      and not vim.bo[stg].modified,
    "rename: cancelled, nothing changed"
  )
  -- :DbtRename on customers.customer_id offers the rename from stg_customers
  vim.cmd.edit(vim.fs.joinpath(ctx.fixture, "models", "customers.sql"))
  local customers_buf = vim.api.nvim_get_current_buf()
  vim.fn.search("customers.customer_id,")
  vim.cmd("normal! 10l")
  rename.rename({ args = "cust_id" })
  H.wait("rename: the column's confirmation dialog opens", 15000, function()
    return picker._state ~= nil and picker._state.opts.title:find("cust_id", 1, true) ~= nil
  end)
  local second = picker._state.stack[1].items[2]
  H.check(
    second.value == "apply_both" and second.label:find("from stg_customers.customer_id", 1, true) ~= nil,
    "rename: a column from upstream offers up- and downstream",
    vim.inspect(second)
  )
  local up_diff = table.concat(second.preview().lines, "\n")
  H.check(up_diff:find("+.*as cust_id") ~= nil, "rename: its preview renames stg_customers", up_diff)
  picker.close()
  H.check(not vim.bo[customers_buf].modified, "rename: closed, nothing changed")
  vim.api.nvim_set_current_buf(bufnr)
end
