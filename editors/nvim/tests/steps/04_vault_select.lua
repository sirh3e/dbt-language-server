-- dbt selections in the navigator's finder, on testdata/vault_lake: what
-- a selection lists (dbt.select's answer, in the built-in picker), the
-- counts and an error in the prompt, <Tab> completion, choosing a test
-- the graph has no node of, <C-a> marking every match and `r` then
-- running the selection itself, --exclude included.

return function(ctx)
  local H = ctx.H
  local explorer = require("cogitator.explorer")
  local builtin = require("cogitator.pick.backends.builtin")
  local live = require("cogitator.pick.backends.builtin_live")
  local render = require("cogitator.explorer.render")

  local root = H.copy_fixture(ctx.repo_root, "vault_lake", true)
  vim.cmd.edit(vim.fs.joinpath(root, "models", "dim", "dim_items.sql"))
  local bufnr = vim.api.nvim_get_current_buf()
  if
    not H.wait("select: a client attaches", 15000, function()
      return #vim.lsp.get_clients({ bufnr = bufnr, name = "cogitator" }) == 1
    end)
  then
    return
  end
  local client = vim.lsp.get_clients({ bufnr = bufnr, name = "cogitator" })[1]
  if not H.wait_refresh(client, bufnr, 1, 30000) then
    return
  end

  -- the server's answer, as the editors ask it
  local answer = H.exec_cmd(client, bufnr, "dbt.select", {
    uri = vim.uri_from_bufnr(bufnr),
    select = "tag:nightly --exclude resource_type:unit_test",
    vocabulary = true,
  }, "select: dbt.select over the wire")
  H.check(
    answer
      and answer.counts
      and answer.counts.model == 1
      and answer.counts.test == 5
      and not answer.counts.unit_test,
    "select: dbt.select answers what dbt ls would, the exclusion applied",
    vim.inspect(answer and answer.counts)
  )
  H.check(
    answer and answer.vocabulary and vim.tbl_contains(answer.vocabulary.values.tag or {}, "nightly"),
    "select: its vocabulary offers the project's tags"
  )

  explorer.open()
  local s = explorer._state
  if
    not H.wait("select: the navigator opens on the model", 15000, function()
      return s and s.selected == "dim_items"
    end)
  then
    explorer.close()
    return
  end

  -- type a selection in `f`: the list is what it selects
  local function typed(query, label)
    builtin._set_query(query)
    local b = builtin._state
    H.wait(label, 10000, function()
      return b == builtin._state and b.base ~= nil and b.live_status ~= "…" and b.query == query
    end, function()
      return vim.inspect(b and { b.live_status, b.live_error })
    end)
    return vim.tbl_map(function(i)
      return i.text
    end, (builtin._state or {}).items or {})
  end
  local function title()
    local b = builtin._state
    return b and require("cogitator.pick.backends.builtin_live").title(b) or ""
  end
  explorer._find()
  H.wait("select: the finder opens (built-in, as it evaluates selections)", 10000, function()
    return builtin._state ~= nil and builtin._state.items ~= nil
  end)
  local texts =
    typed("resource_type:seed resource_type:snapshot", "select: a union of resource types is evaluated")
  table.sort(texts)
  H.check(
    vim.deep_equal(texts, {
      "item_states  [seed]  #reference",
      "item_versions_snapshot  [snapshot]  #history",
      "items_snapshot  [snapshot]  #history",
      "not_null_item_versions_snapshot_item_id  [test]",
    }),
    "select: seeds and snapshots, a YAML snapshot among them, and its test (dbt's eager rule)",
    vim.inspect(texts)
  )
  H.check(
    title():find("dbt select · 4 · 1 seed, 2 snapshots, 1 test", 1, true) ~= nil,
    "select: the prompt counts them",
    title()
  )

  typed("bogus:x", "select: a wrong method answers an error")
  H.check(
    title():find("✗ dbt has no selection method `bogus:`", 1, true) ~= nil,
    "select: the error is in the prompt",
    title()
  )
  local marks = vim.api.nvim_buf_get_extmarks(
    builtin._state.bufs.prompt,
    vim.api.nvim_create_namespace("cogitator.pick.live"),
    0,
    -1,
    { details = true }
  )
  H.check(
    #marks == 1 and marks[1][3] == 0 and marks[1][4].end_col == 6,
    "select: the error underlines what it is about",
    vim.inspect(marks)
  )

  -- <Tab> completes a method's value from the vocabulary
  builtin._set_query("tag:his")
  pcall(vim.api.nvim_win_set_cursor, builtin._state.wins.prompt, { 1, #"tag:his" })
  live.complete(builtin._state, builtin._set_query)
  H.wait("select: <Tab> completes a tag", 10000, function()
    return builtin._state and builtin._state.query == "tag:history "
  end, function()
    return builtin._state and builtin._state.query
  end)

  -- fuzzy words are fuzzy again
  builtin._set_query("dim")
  H.check(builtin._state.base == nil, "select: plain words leave the selection")

  -- choosing a generic test, which the graph has no node of, opens it
  local tests = typed("test_type:generic,+dim_items", "select: generic tests of dim_items and above")
  local at
  for i, t in ipairs(tests) do
    if t:find("^not_null_dim_items_item_version_id") then
      at = i
    end
  end
  H.check(at ~= nil, "select: a column's test is listed", vim.inspect(tests))
  if at then
    builtin._state.cursor = at
    builtin._accept("default")
    H.wait("select: choosing a YAML test opens its properties file", 10000, function()
      return vim.api.nvim_buf_get_name(0):find("_dim__models.yml", 1, true) ~= nil
    end)
    H.check(
      vim.api.nvim_get_current_line():find("unique, not_null", 1, true) ~= nil,
      "select: … on the test's line",
      vim.api.nvim_get_current_line()
    )
  end

  -- <C-a> marks every match; `r` then runs the selection itself
  vim.cmd.edit(vim.fs.joinpath(root, "models", "dim", "dim_items.sql"))
  explorer.open()
  s = explorer._state
  H.wait("select: the navigator again", 15000, function()
    return s and s.selected == "dim_items"
  end)
  explorer._find()
  H.wait("select: the finder again", 10000, function()
    return builtin._state ~= nil
  end)
  local listed = typed("+dim_items --exclude resource_type:test", "select: ancestors, without tests")
  H.check(#listed == 7 and not vim.iter(listed):any(function(t)
    return t:find("%[test%]") ~= nil
  end), "select: --exclude leaves the tests out", vim.inspect(listed))
  local mark_all = builtin._state.source.keys[1]
  H.check(mark_all.lhs == "<C-a>", "select: the finder's <C-a> is the navigator's")
  builtin._all(mark_all.fn)
  H.wait("select: <C-a> marks every match", 10000, function()
    return vim.tbl_count(s.marked) == 7
  end, function()
    return vim.inspect(s.marked)
  end)
  H.check(
    render.center_title(s):find("dbt select +dim_items --exclude resource_type:test (7)", 1, true) ~= nil,
    "select: the centre says which selection the marks are",
    render.center_title(s)
  )
  local commands = require("cogitator.commands")
  local real, ran = commands.run_selection, nil
  commands.run_selection = function(opts)
    ran = opts
  end
  vim.api.nvim_set_current_win(s.wins.center)
  H.feed("r")
  H.wait("select: r runs", 5000, function()
    return ran ~= nil
  end)
  commands.run_selection = real
  H.check(
    ran and ran.args == "+dim_items" and ran.exclude == "resource_type:test" and ran.ask_kind,
    "select: r runs the selection and its exclusion, not the names",
    vim.inspect(ran)
  )
  explorer.close()
  vim.api.nvim_set_current_buf(ctx.bufnr)
end
