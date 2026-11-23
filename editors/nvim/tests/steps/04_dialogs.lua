-- The yazi-style picker (cogitator.picker) with real keypresses, the
-- :Dbt groups, and the code-action menu (cogitator.code_actions): pure
-- grouping and previews first, then the menu against the live server.

return function(ctx)
  local H, bufnr = ctx.H, ctx.bufnr
  local picker = require("cogitator.picker")
  local dispatch = require("cogitator.dispatch")
  local ca = require("cogitator.code_actions")
  vim.api.nvim_set_current_buf(bufnr)

  -- pure helpers
  H.check(
    picker.matches({ label = "Remove the trailing comma" }, "trail com"),
    "picker: every word must match"
  )
  H.check(not picker.matches({ label = "dbt run" }, "test"), "picker: a non-match is dropped")
  H.check(
    picker.matches({ label = "Refactor", children = { { label = "Expand c.* to 3 column(s)" } } }, "expand"),
    "picker: a group matches through its children"
  )
  local line = picker.format_line({ label = "Quick fixes", detail = "3", children = {} }, 30)
  H.check(
    vim.fn.strdisplaywidth(line) == 30 and line:find("Quick fixes", 1, true) and line:find("3 ›$"),
    "picker: a group row is padded, counted and arrowed",
    line
  )
  local long = picker.format_line({ label = string.rep("x", 80) }, 20)
  H.check(
    vim.fn.strdisplaywidth(long) <= 20 and long:find("…", 1, true),
    "picker: a long label is cut",
    long
  )

  -- the tree, walked with h/j/k/l
  local chosen
  picker.open({
    title = "test",
    items = {
      {
        label = "alpha",
        children = { { label = "a1" }, { label = "a2" }, { label = "a3", disabled = "no" } },
      },
      { label = "beta", children = { { label = "b1", value = 1 } } },
      { label = "gamma" },
    },
    on_choose = function(item, path)
      chosen = { item = item, path = path }
    end,
  })
  local st = picker._state
  H.check(
    st ~= nil and vim.api.nvim_get_current_win() == st.wins.current,
    "picker: opens focused on the list"
  )
  local function preview_text()
    return table.concat(vim.api.nvim_buf_get_lines(st.bufs.preview, 0, -1, false), "\n")
  end
  H.check(preview_text():find("a1", 1, true) ~= nil, "picker: a group's preview lists its children")
  H.feed("l")
  H.check(#st.stack == 2 and st.stack[2].label == "alpha", "picker: l enters the group")
  local parent = table.concat(vim.api.nvim_buf_get_lines(st.bufs.parent, 0, -1, false), "\n")
  H.check(parent:find("beta", 1, true) ~= nil, "picker: the parent column shows the level above")
  H.feed("2j")
  H.check(st.stack[2].cursor == 3, "picker: 2j moves two rows", st.stack[2].cursor)
  H.feed("l")
  H.check(picker._state ~= nil and chosen == nil, "picker: a disabled item is not chosen")
  H.feed("j")
  H.check(st.stack[2].cursor == 1, "picker: j wraps at the bottom")
  H.feed("h")
  H.check(#st.stack == 1, "picker: h goes back up")
  H.feed("h")
  H.check(#st.stack == 1 and picker._state ~= nil, "picker: h at the root stays")
  H.feed("G")
  H.check(st.stack[1].cursor == 3, "picker: G jumps to the last")
  H.feed("gg")
  H.check(st.stack[1].cursor == 1, "picker: gg jumps to the first")
  picker.set_filter("b1")
  H.check(
    #st.stack[1].shown == 1 and st.stack[1].shown[1].label == "beta",
    "picker: the filter keeps groups by their children"
  )
  H.feed("l")
  H.check(#st.stack[2].shown == 1, "picker: an entered group keeps the filter")
  H.feed("<CR>")
  H.check(
    chosen and chosen.item.value == 1 and #chosen.path == 2 and chosen.path[1].label == "beta",
    "picker: <CR> chooses with the path",
    chosen and chosen.item.label
  )
  H.check(picker._state == nil, "picker: closes on a choice")

  -- the vim.ui.select drop-in, and q cancels
  local got = "unset"
  picker.select({ "one", "two" }, { prompt = "pick:" }, function(item, idx)
    got = { item, idx }
  end)
  H.feed("j<CR>")
  H.check(
    type(got) == "table" and got[1] == "two" and got[2] == 2,
    "picker: select answers item and index",
    vim.inspect(got)
  )
  picker.select({ "one" }, {}, function(item)
    got = item
  end)
  H.feed("q")
  H.check(got == nil and picker._state == nil, "picker: q cancels with nil")

  -- pick.select, every choice dialog of the plugin, opens the yazi picker
  local pick_got = "unset"
  require("cogitator.pick").select({ "x", "y" }, { prompt = "dialog" }, function(item)
    pick_got = item
  end)
  H.check(picker._state ~= nil, "picker: pick.select opens the yazi picker by default")
  H.feed("j<CR>")
  H.check(pick_got == "y", "picker: pick.select answers through it", pick_got)

  -- the vim.ui.input drop-in
  local typed = "unset"
  picker.input({ prompt = "name: ", default = "abc" }, function(text)
    typed = text
  end)
  H.feed("<End>d<CR>")
  H.check(typed == "abcd", "picker: input accepts what was typed", typed)
  picker.input({ prompt = "name" }, function(text)
    typed = text
  end)
  H.feed("x<Esc><Esc>")
  H.check(typed == nil, "picker: input <Esc> <Esc> cancels", typed)
  vim.api.nvim_set_current_buf(bufnr)

  -- every :Dbt subcommand is in exactly one group
  local seen = {}
  for _, g in ipairs(dispatch.GROUPS) do
    for _, name in ipairs(g.subs) do
      H.check(dispatch.SUBCOMMANDS[name] ~= nil, "dispatch: group entry " .. name .. " is a subcommand")
      H.check(not seen[name], "dispatch: " .. name .. " is in one group")
      seen[name] = true
    end
  end
  for name in pairs(dispatch.SUBCOMMANDS) do
    H.check(seen[name] or dispatch.UNGROUPED[name], "dispatch: " .. name .. " is grouped")
  end

  -- grouping of server answers, and the previews
  local fixture_actions = {
    {
      title = "dbt run",
      command = { title = "dbt run", command = "dbt.run", arguments = { { uri = "file:///m.sql" } } },
    },
    { title = "Remove the trailing comma", kind = "quickfix", isPreferred = true, edit = { changes = {} } },
    { title = "Expand c.* to 2 column(s)", kind = "refactor.rewrite", edit = { changes = {} } },
    { title = "Fix all", kind = "source.fixAll", edit = { changes = {} } },
  }
  local items, skip = ca.server_items(fixture_actions, "utf-16")
  local names = vim.tbl_map(function(i)
    return i.label
  end, items)
  H.check(
    vim.deep_equal(names, { "Quick fixes", "Refactor", "Source", "dbt" }),
    "actions: server actions grouped by kind, fixes first",
    vim.inspect(names)
  )
  H.check(skip.run == true, "actions: a server dbt.run hides the plugin's duplicate")
  H.check(items[1].children[1].icon == "★", "actions: a preferred fix is starred")

  local probe = vim.fs.joinpath(H.tmp, "preview.sql")
  vim.fn.writefile({ "select a,", "from t" }, probe)
  local uri = vim.uri_from_fname(probe)
  local diff = table.concat(
    ca.edit_lines({
      documentChanges = {
        {
          textDocument = { uri = uri, version = vim.NIL },
          edits = {
            {
              range = { start = { line = 0, character = 8 }, ["end"] = { line = 0, character = 9 } },
              newText = "",
            },
          },
        },
        { kind = "create", uri = vim.uri_from_fname(vim.fs.joinpath(H.tmp, "new.sql")) },
      },
    }, "utf-16"),
    "\n"
  )
  H.check(
    diff:find("-select a,", 1, true) and diff:find("+select a", 1, true) and diff:find("new file", 1, true),
    "actions: an edit previews as a diff and its file operations",
    diff
  )
  local cmd_preview = table.concat(ca.action_preview(fixture_actions[1], "utf-16").lines, "\n")
  H.check(
    cmd_preview:find("runs dbt.run", 1, true) ~= nil,
    "actions: a command previews what it runs",
    cmd_preview
  )

  -- what is about the cursor: the server marks it in `data`
  local mark = { cogitator = { atCursor = true } }
  local server = {
    fixture_actions[1],
    { title = "Add version 2 of 'm'", kind = "refactor.extract", edit = { changes = {} } },
    {
      title = "Open schema of 'stg_orders'",
      command = { command = "dbt.openSchema", arguments = {} },
      data = mark,
    },
    fixture_actions[2],
    { title = "Expand * to 2 column(s)", kind = "refactor.rewrite", edit = { changes = {} }, data = mark },
  }
  local function titles(list)
    return vim.tbl_map(function(a)
      return a.title or a.label
    end, list)
  end
  local at_cursor, model_wide = ca.split(server)
  H.check(
    vim.deep_equal(titles(at_cursor), {
      "Open schema of 'stg_orders'",
      "Remove the trailing comma",
      "Expand * to 2 column(s)",
    }) and vim.deep_equal(titles(model_wide), { "dbt run", "Add version 2 of 'm'" }),
    "actions: the server's marked actions and every fix are about the cursor",
    vim.inspect({ titles(at_cursor), titles(model_wide) })
  )
  local root = ca.items(bufnr, server, "utf-16")
  local labels = titles(root)
  H.check(
    labels[1] == "Remove the trailing comma"
      and root[1].children == nil
      and root[1].icon == "★"
      and labels[2] == "Expand * to 2 column(s)"
      and labels[3] == "Open schema of 'stg_orders'",
    "actions: the cursor's actions are the first rows, a preferred fix first, each chosen with one key",
    vim.inspect(labels)
  )
  local this_model
  for _, item in ipairs(root) do
    if item.label:find("^This model") then
      this_model = item
    end
  end
  H.check(
    not vim.tbl_contains(labels, "dbt") and not vim.tbl_contains(labels, "Model"),
    "actions: no dbt or Model group beside the cursor's actions",
    vim.inspect(labels)
  )
  local model_titles = this_model and titles(this_model.children) or {}
  H.check(
    vim.tbl_contains(model_titles, "dbt run") and vim.tbl_contains(model_titles, "Add version 2 of 'm'"),
    "actions: the file's actions and the plugin's model commands are one group, This model",
    vim.inspect(model_titles)
  )
  local flat = ca.items(bufnr, { fixture_actions[1] }, "utf-16")
  H.check(
    flat[1].label == "dbt run" and flat[1].children == nil,
    "actions: with nothing at the cursor the model's actions are the top level",
    flat[1].label
  )

  -- the menu against the live server, on the customers model
  vim.api.nvim_set_current_buf(bufnr)
  vim.api.nvim_win_set_cursor(0, { 1, 0 })
  ca.open()
  H.wait("actions: the menu opens", 10000, function()
    return picker._state ~= nil
  end)
  local menu = picker._state
  if menu then
    local top = {}
    for _, item in ipairs(menu.stack[1].items) do
      top[item.label] = item
    end
    -- the model's actions: the top level itself, or "This model" when
    -- something is about the cursor (here a fix of line 1's diagnostic)
    local model_level = menu.stack[1].items
    for label, item in pairs(top) do
      if label:find("^This model") then
        model_level = item.children
      end
    end
    local details, model_labels = {}, {}
    for _, c in ipairs(model_level) do
      details[#details + 1] = c.detail or ""
      model_labels[#model_labels + 1] = c.label
    end
    H.check(
      top.dbt == nil and top.Model == nil and top.Views ~= nil and vim.tbl_contains(model_labels, "dbt run"),
      "actions: the model's actions are one level, the plugin's views a group",
      vim.inspect(vim.tbl_keys(top))
    )
    H.check(
      vim.tbl_contains(details, ":Dbt plan") and not vim.tbl_contains(details, ":Dbt run"),
      "actions: the plugin's model commands beside the server's, not their twins",
      vim.inspect(details)
    )
    local first = menu.stack[1].items[1]
    H.check(first.children == nil, "actions: the menu opens on an action, not on a group", first.label)
    picker.close()
  end
  H.check(picker._state == nil, "actions: the menu closes")
end
