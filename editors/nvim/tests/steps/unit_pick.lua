-- The pickers without a server: fzf's query syntax and ranking, which
-- backend shows a source, each adapter on a fake of its plugin, the
-- built-in picker's keys, and the sources' items from a canned graph.

return function(ctx)
  local H = ctx.H
  local query = require("cogitator.pick.query")

  -- query syntax
  local function names(texts, q)
    return vim.tbl_map(function(m)
      return texts[m.index]
    end, query.rank(texts, q))
  end
  local models = { "customers", "stg_customers", "customer_orders", "orders", "stg_orders", "stg_payments" }
  local cases = {
    { "", models },
    { "cust", { "customers", "stg_customers", "customer_orders" } },
    { "stg cust", { "stg_customers" } },
    { "'orders", { "orders", "stg_orders", "customer_orders" } },
    { "^stg", { "stg_orders", "stg_payments", "stg_customers" } },
    { "orders$", { "orders", "stg_orders", "customer_orders" } },
    { "!stg ^cust", { "customers", "customer_orders" } },
    { "pay | ^cust", { "customers", "stg_payments", "customer_orders" } },
    { "zzz", {} },
  }
  for _, c in ipairs(cases) do
    local got = names(models, c[1])
    table.sort(got)
    local want = vim.deepcopy(c[2])
    table.sort(want)
    H.check(
      vim.deep_equal(got, want),
      ("pick: query %q matches what fzf would"):format(c[1]),
      vim.inspect(got)
    )
  end
  H.check(
    names(models, "cust")[1] == "customers",
    "pick: the tightest match ranks first",
    vim.inspect(names(models, "cust"))
  )
  H.check(
    names(models, "^stg")[1] == "stg_orders",
    "pick: equal scores rank the shorter name first",
    vim.inspect(names(models, "^stg"))
  )
  local m = query.rank({ "stg_customers" }, "stg cust")[1]
  table.sort(m.positions)
  H.check(
    vim.deep_equal(m.positions, { 0, 1, 2, 4, 5, 6, 7 }),
    "pick: matched characters are reported for highlighting",
    vim.inspect(m.positions)
  )
  H.check(
    #query.rank({ "Customers" }, "'Cust") == 1 and #query.rank({ "customers" }, "'Cust") == 0,
    "pick: an exact term with a capital is case-sensitive"
  )

  -- which backend
  local backends = require("cogitator.pick.backends")
  local saved = {}
  for _, mod in ipairs({ "fzf-lua", "snacks", "telescope.pickers" }) do
    saved[mod] = package.loaded[mod]
    package.loaded[mod] = nil
    package.preload[mod] = nil
  end
  H.check(backends.resolve("auto").name == "builtin", "pick: with no picker plugin, auto is the built-in one")
  package.loaded["telescope.pickers"] = {}
  H.check(backends.resolve("auto").name == "telescope", "pick: auto finds telescope")
  package.loaded["snacks"] = { picker = {} }
  H.check(backends.resolve("auto").name == "snacks", "pick: auto prefers snacks over telescope")
  package.loaded["fzf-lua"] = {}
  H.check(backends.resolve("auto").name == "fzf-lua", "pick: auto prefers fzf-lua")
  package.loaded["fzf-lua"], package.loaded["snacks"], package.loaded["telescope.pickers"] = nil, nil, nil
  H.check(backends.resolve("fzf-lua").name == "builtin", "pick: a picker asked for but missing falls back")
  H.check(not pcall(backends.resolve, "fzf"), "pick: an unknown picker name is an error")

  -- the adapters, on fakes of their plugins
  local source = {
    title = "models",
    items = function(done)
      done({})
    end,
    preview = function(item, done)
      done({ lines = { "preview of " .. item.text }, filetype = "sql" })
    end,
  }
  local items = { { text = "customers", name = "customers" }, { text = "orders", name = "orders" } }
  local ran = {}
  local function run(action, chosen)
    ran[#ran + 1] = { action, vim.tbl_map(function(i)
      return i.name
    end, chosen) }
  end

  local fzf_opts, fzf_entries
  package.loaded["fzf-lua"] = {
    fzf_exec = function(entries, opts)
      fzf_entries, fzf_opts = entries, opts
    end,
  }
  package.loaded["fzf-lua.previewer.builtin"] = {
    base = {
      extend = function()
        local P = {}
        P.super = { new = function() end }
        return P
      end,
    },
  }
  backends.get("fzf-lua").open(source, items, run)
  fzf_opts.actions["ctrl-v"]({ fzf_entries[2] })
  fzf_opts.actions["default"]({ fzf_entries[1], fzf_entries[2] })
  H.check(
    fzf_entries[1] == "1\tcustomers"
      and fzf_opts.fzf_opts["--with-nth"] == "2.."
      and vim.deep_equal(ran, { { "vsplit", { "orders" } }, { "default", { "customers", "orders" } } }),
    "pick: fzf-lua gets indexed entries and the shared actions",
    vim.inspect({ fzf_entries, ran })
  )
  package.loaded["fzf-lua"], package.loaded["fzf-lua.previewer.builtin"] = nil, nil

  ran = {}
  local snacks_opts
  package.loaded["snacks"] = {
    picker = {
      pick = function(opts)
        snacks_opts = opts
      end,
    },
  }
  backends.get("snacks").open(source, items, run)
  local fake_picker = {
    selected = function()
      return {}
    end,
    close = function() end,
  }
  snacks_opts.confirm(fake_picker, snacks_opts.items[2])
  snacks_opts.actions.cogitator_yank(fake_picker, snacks_opts.items[1])
  local shown
  snacks_opts.preview({
    item = snacks_opts.items[1],
    preview = {
      set_lines = function(_, lines)
        shown = lines
      end,
      highlight = function() end,
    },
  })
  H.check(
    snacks_opts.items[1].text == "customers"
      and snacks_opts.win.input.keys["<C-v>"][1] == "cogitator_vsplit"
      and vim.deep_equal(ran, { { "default", { "orders" } }, { "yank", { "customers" } } })
      and vim.deep_equal(shown, { "preview of customers" }),
    "pick: snacks gets the items, the actions on the same keys and the preview",
    vim.inspect({ ran, shown })
  )
  package.loaded["snacks"] = nil

  ran = {}
  local tele = {}
  local default_action
  package.loaded["telescope.pickers"] = {
    new = function(_, spec)
      tele.spec = spec
      return { find = function() end }
    end,
  }
  package.loaded["telescope.finders"] = {
    new_table = function(t)
      return t
    end,
  }
  package.loaded["telescope.config"] = { values = { generic_sorter = function() end } }
  package.loaded["telescope.previewers"] = {
    new_buffer_previewer = function(p)
      return p
    end,
  }
  package.loaded["telescope.actions"] = {
    select_default = {
      replace = function(_, fn)
        default_action = fn
      end,
    },
    close = function() end,
  }
  local selected = items[2]
  package.loaded["telescope.actions.state"] = {
    get_current_picker = function()
      return {
        get_multi_selection = function()
          return {}
        end,
      }
    end,
    get_selected_entry = function()
      return { value = selected }
    end,
  }
  local mapped = {}
  backends.get("telescope").open(source, items, run)
  tele.spec.attach_mappings(0, function(_, lhs, fn)
    mapped[lhs] = fn
  end)
  default_action()
  mapped["<C-r>"]()
  H.check(
    tele.spec.finder.entry_maker(items[1]).ordinal == "customers"
      and vim.deep_equal(ran, { { "default", { "orders" } }, { "run", { "orders" } } }),
    "pick: telescope gets a table finder and the shared actions",
    vim.inspect(ran)
  )
  for _, mod in ipairs({
    "telescope.pickers",
    "telescope.finders",
    "telescope.config",
    "telescope.previewers",
    "telescope.actions",
    "telescope.actions.state",
  }) do
    package.loaded[mod] = nil
  end
  for mod, value in pairs(saved) do
    package.loaded[mod] = value
  end

  -- the built-in picker: opens at once, filters, marks, accepts
  local builtin = require("cogitator.pick.backends.builtin")
  ran = {}
  local later
  builtin.open({
    title = "models",
    items = function(done)
      later = done
    end,
    preview = function(item, done)
      done({ lines = { "-- " .. item.text } })
    end,
  }, run)
  local st = builtin._state
  local first = table.concat(vim.api.nvim_buf_get_lines(st.bufs.list, 0, -1, false), "\n")
  H.check(
    first:find("loading", 1, true) ~= nil,
    "pick: the built-in picker opens before its items arrive",
    first
  )
  later(vim.tbl_map(function(n)
    return { text = n, name = n }
  end, models))
  builtin._set_query("stg")
  H.check(#st.matches == 3, "pick: the built-in picker filters as typed", #st.matches)
  H.wait("pick: the built-in picker previews the current item", 2000, function()
    return (vim.api.nvim_buf_get_lines(st.bufs.preview, 0, 1, false)[1] or ""):find("^%-%- stg") ~= nil
  end)
  builtin._toggle_mark()
  builtin._toggle_mark()
  builtin._accept("run")
  H.wait("pick: accepting runs the action", 2000, function()
    return #ran == 1
  end)
  H.check(
    builtin._state == nil and ran[1] and ran[1][1] == "run" and #ran[1][2] == 2,
    "pick: marked items are accepted together and the picker closes",
    vim.inspect(ran)
  )

  -- vim.ui.select through the picker
  local pick = require("cogitator.pick")
  local config = require("cogitator").config
  local backend_before = config.picker
  config.picker = "builtin"
  -- the dialogs reach the backend only with the yazi picker off
  local ui_before = config.ui.picker
  config.ui.picker = false
  local answer
  pick.select({ "a", "b", "c" }, { prompt = "letter:" }, function(choice, idx)
    answer = { choice, idx }
  end)
  builtin._set_query("b")
  builtin._accept("default")
  H.wait("pick: select answers like vim.ui.select", 2000, function()
    return answer ~= nil
  end)
  H.check(
    vim.deep_equal(answer, { "b", 2 }),
    "pick: select hands back the item and its index",
    vim.inspect(answer)
  )
  answer = nil
  pick.select({ "a" }, {}, function(choice)
    answer = { choice }
  end)
  builtin.close(true)
  H.check(answer ~= nil and answer[1] == nil, "pick: a cancelled select answers nil")
  config.picker = backend_before
  config.ui.picker = ui_before

  -- sources from a canned graph (no server)
  local graph = require("cogitator.pick.graph")
  local fake = { id = 999001 }
  graph._cache[fake.id] = {
    graph = {
      nodes = {
        {
          id = "model:a",
          name = "a",
          kind = "model",
          uri = "file:///p/a.sql",
          tags = { "x" },
          columns = { { name = "id" } },
        },
        { id = "model:b", name = "b", kind = "model", uri = "file:///p/b.sql", tags = {}, columns = {} },
        { id = "model:c", name = "c", kind = "model", uri = "file:///p/c.sql", tags = { "x" }, columns = {} },
        { id = "source:s.t", name = "s.t", kind = "source", tags = {}, columns = {} },
      },
      edges = {
        { from = "source:s.t", to = "model:a" },
        { from = "model:a", to = "model:b" },
        { from = "model:b", to = "model:c" },
      },
    },
  }
  local sources = require("cogitator.pick.sources")
  local function texts_of(src)
    local out
    src.items(function(list)
      out = vim.tbl_map(function(i)
        return i.text
      end, list)
    end)
    return out
  end
  local sctx = { client = fake, bufnr = vim.api.nvim_get_current_buf() }
  H.check(
    vim.deep_equal(texts_of(sources.models(sctx)), { "a  [model]  #x", "b  [model]", "c  [model]  #x" }),
    "pick: models lists the graph's models with their tags",
    vim.inspect(texts_of(sources.models(sctx)))
  )
  H.check(#texts_of(sources.all(sctx)) == 4, "pick: all lists every kind of node")
  H.check(
    vim.deep_equal(texts_of(sources.columns(sctx)), { "a.id" }),
    "pick: columns lists model.column",
    vim.inspect(texts_of(sources.columns(sctx)))
  )
  local parents, children = graph.adjacency(graph._cache[fake.id].graph)
  H.check(
    vim.deep_equal(
      graph.walk("model:a", children),
      { { id = "model:b", depth = 1 }, { id = "model:c", depth = 2 } }
    )
      and vim.deep_equal(graph.walk("model:c", parents), {
        { id = "model:b", depth = 1 },
        { id = "model:a", depth = 2 },
        { id = "source:s.t", depth = 3 },
      }),
    "pick: the lineage walk is nearest first with its depth"
  )
  H.check(
    vim.deep_equal(texts_of(sources.tags(sctx)), { "#x  (2 nodes)" }),
    "pick: tags counts the nodes carrying each",
    vim.inspect(texts_of(sources.tags(sctx)))
  )
  graph.invalidate(fake.id)
end
