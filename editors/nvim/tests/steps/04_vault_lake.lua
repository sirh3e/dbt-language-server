-- A project in the shape of a real one (testdata/vault_lake, see its
-- README): CTE chains filtered by subqueries, Markdown doc blocks over
-- many lines, singular tests downstream of a model, a custom
-- generate_schema_name only target/manifest.json knows, a YAML file per
-- folder. Everything here once failed on such a project while the
-- jaffle_shop suite passed.

return function(ctx)
  local H = ctx.H
  local ca = require("cogitator.code_actions")
  local picker = require("cogitator.picker")
  local explorer = require("cogitator.explorer")
  local render = require("cogitator.explorer.render")

  local root = H.copy_fixture(ctx.repo_root, "vault_lake", true)
  local model = vim.fs.joinpath(root, "models", "dim", "dim_items.sql")
  vim.cmd.edit(model)
  local bufnr = vim.api.nvim_get_current_buf()
  if
    not H.wait("vault: a client attaches in a second project", 15000, function()
      return #vim.lsp.get_clients({ bufnr = bufnr, name = "cogitator" }) == 1
    end)
  then
    return
  end
  local client = vim.lsp.get_clients({ bufnr = bufnr, name = "cogitator" })[1]
  H.check(client.root_dir == root, "vault: the client's root is the project's", client.root_dir)
  if not H.wait_refresh(client, bufnr, 1, 30000) then
    return
  end

  local lines = vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)
  local function find(pattern)
    for i, line in ipairs(lines) do
      local col = line:find(pattern)
      if col then
        return i, col
      end
    end
  end

  -- `select *` inside a CTE that a subquery filters: it expands
  local row, col = find("select %* from {{ ref%('bv_item_transitions'%) }}")
  local star = { line = row - 1, character = col - 1 + #"select " }
  local actions = H.request(client, bufnr, "textDocument/codeAction", {
    textDocument = { uri = vim.uri_from_bufnr(bufnr) },
    range = { start = star, ["end"] = star },
    context = { diagnostics = {} },
  }, "vault: code actions at the star") or {}
  local expand
  for _, a in ipairs(actions) do
    if a.title:find("^Expand %*") then
      expand = a
    end
  end
  H.check(
    expand ~= nil and expand.title == "Expand * to 4 column(s)",
    "vault: a star in a CTE filtered by a subquery expands to its relation's columns",
    vim.inspect(vim.tbl_map(function(a)
      return a.title
    end, actions))
  )
  H.check(expand and ca.at_cursor(expand), "vault: the expansion is marked as about the cursor")
  local root_items = ca.items(bufnr, actions, client.offset_encoding)
  H.check(
    root_items[1].label:find("^Expand %*") ~= nil and root_items[1].children == nil,
    "vault: the menu's first row is the expansion, one key away",
    root_items[1].label
  )

  -- on a CTE's name the menu opens on extracting it
  row, col = find("hubs as %(")
  vim.api.nvim_win_set_cursor(0, { row, col })
  ca.open()
  if H.wait("vault: the menu opens on a CTE", 10000, function()
    return picker._state ~= nil
  end) then
    local first = picker._state.stack[1].items[1]
    H.check(
      first.label == "Extract CTE 'hubs' to a model",
      "vault: on a CTE the first row extracts it",
      first.label
    )
    local has_model_group = false
    for _, item in ipairs(picker._state.stack[1].items) do
      has_model_group = has_model_group or item.label == "This model · dim_items"
    end
    H.check(has_model_group, "vault: the model's own actions are one group below it")
    picker.close()
  end

  -- the navigator: doc blocks, tests downstream, the rendered relation
  vim.api.nvim_win_set_cursor(0, { 1, 0 })
  local ok, err = pcall(explorer.open)
  H.check(ok, "vault: the explorer opens", err)
  local s = explorer._state
  if
    not H.wait("vault: the explorer loads dim_items", 15000, function()
      return s and s.info and s.info.model == "dim_items" and s.relation ~= nil
    end, function()
      return vim.inspect(s and { selected = s.selected, info = s.info ~= nil, relation = s.relation })
    end)
  then
    explorer.close()
    return
  end
  local card_lines = vim.api.nvim_buf_get_lines(s.bufs.center, 0, -1, false)
  local card = table.concat(card_lines, "\n")
  H.check(
    -- a type may stand between the name and the description, once inferred
    card:find("• state[^\n]* — What an item is at the end of a version: …") ~= nil,
    "vault: a column's Markdown doc block shows as its first paragraph",
    card
  )
  H.check(not card:find("doc(", 1, true), "vault: no column shows a raw doc() call", card)
  H.check(
    card:find("relation: lake.dim.dim_items", 1, true) ~= nil and not card:find("defaults", 1, true),
    "vault: the relation is the manifest's, custom schema and all",
    card
  )
  local shown, hidden = render.side_entries(s, "right")
  local title = vim.api.nvim_win_get_config(s.wins.right).title
  local title_text = type(title) == "table"
      and table.concat(vim.tbl_map(function(c)
        return c[1]
      end, title))
    or tostring(title)
  H.check(
    #shown == 0 and hidden.test == 2 and title_text:find("+2 tests", 1, true) ~= nil,
    "vault: downstream holds the two singular tests, counted and not walked onto",
    title_text
  )
  local up = vim.tbl_map(function(e)
    return e.name
  end, render.side_entries(s, "left"))
  table.sort(up)
  H.check(
    vim.deep_equal(up, { "bv_item_transitions", "hub_item" }),
    "vault: upstream lists the models it reads",
    vim.inspect(up)
  )
  explorer.close()

  -- from the properties file, the entry under the cursor is the model
  vim.cmd.edit(vim.fs.joinpath(root, "models", "dim", "_dim__models.yml"))
  local yml = vim.api.nvim_get_current_buf()
  H.wait("vault: the YAML buffer attaches", 10000, function()
    return #vim.lsp.get_clients({ bufnr = yml, name = "cogitator" }) == 1
  end)
  local yml_lines = vim.api.nvim_buf_get_lines(yml, 0, -1, false)
  for i, line in ipairs(yml_lines) do
    if line:find("- name: state", 1, true) then
      vim.api.nvim_win_set_cursor(0, { i, 0 })
    end
  end
  explorer.open()
  s = explorer._state
  H.wait("vault: from a column of the YAML entry, the explorer opens on its model", 15000, function()
    return s and s.selected == "dim_items"
  end, function()
    return s and s.selected or "no explorer"
  end)
  H.check(picker._state == nil, "vault: no model picker in the way")
  explorer.close()
  vim.api.nvim_set_current_buf(ctx.bufnr)
  for _, b in ipairs({ bufnr, yml }) do
    pcall(vim.api.nvim_buf_delete, b, { force = true })
  end
end
