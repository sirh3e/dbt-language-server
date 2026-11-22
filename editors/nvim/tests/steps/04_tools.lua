-- Selection expand/shrink and the health check against the live server.

return function(ctx)
  local H, bufnr, client = ctx.H, ctx.bufnr, ctx.client
  local select = require("cogitator.select")

  vim.api.nvim_set_current_buf(bufnr)
  -- inside 'stg_customers' of line 3
  local line = vim.api.nvim_buf_get_lines(bufnr, 2, 3, false)[1]
  vim.api.nvim_win_set_cursor(0, { 3, line:find("stg_customers", 1, true) + 2 })
  local function selected()
    local a, b = vim.fn.getpos("v"), vim.fn.getpos(".")
    return { a[2], a[3], b[2], b[3], vim.fn.mode() }
  end
  local function size(sel)
    return (sel[3] - sel[1]) * 1000 + math.abs(sel[4] - sel[2])
  end
  -- expand asks in the background: it returns at once, the selection
  -- changes when the answer comes
  local t = vim.uv.hrtime()
  select.expand()
  H.check((vim.uv.hrtime() - t) / 1e6 < 50, "select: expand returns without waiting for the server")
  H.wait("select: expand starts a visual selection", 10000, function()
    return vim.fn.mode() == "v"
  end)
  local first = selected()
  H.check(first[5] == "v", "select: expand starts a visual selection", vim.inspect(first))
  select.expand()
  H.wait("select: expanding again grows it", 10000, function()
    return size(selected()) > size(first)
  end)
  local second = selected()
  H.check(size(second) > size(first), "select: expanding again grows it", vim.inspect({ first, second }))
  select.shrink()
  local back = selected()
  H.check(
    vim.deep_equal(back, first),
    "select: shrink retraces the last expand",
    vim.inspect({ first, back })
  )
  vim.cmd("normal! \27")

  -- :DbtSyntaxTree: the tree in a split, following the source cursor
  local syntax = require("cogitator.syntax")
  vim.api.nvim_set_current_buf(bufnr)
  vim.cmd("only")
  vim.api.nvim_win_set_cursor(0, { 3, line:find("stg_customers", 1, true) + 2 })
  vim.cmd("DbtSyntaxTree")
  local st = syntax._state
  H.wait("syntax: :DbtSyntaxTree fetches the tree", 10000, function()
    return st and #st.nodes > 0
  end)
  if st and #st.nodes > 0 then
    local row = vim.api.nvim_win_get_cursor(st.win)[1]
    local here = st.nodes[row]
    H.check(
      here.range.start.line <= 2 and here.range["end"].line >= 2 and here.depth > 0,
      "syntax: the tree's cursor sits on the node under the source cursor",
      vim.inspect(here)
    )
    local text = table.concat(vim.api.nvim_buf_get_lines(st.buf, 0, -1, false), "\n")
    H.check(text:find("source_file", 1, true) ~= nil, "syntax: the root is drawn", text:sub(1, 200))
    vim.cmd("DbtSyntaxTree")
    H.check(
      syntax._state == nil and not vim.api.nvim_win_is_valid(st.win),
      "syntax: toggling closes the view"
    )
  end

  -- the grammar in Neovim, built from the server's own checkout (offline)
  local ts = require("cogitator.treesitter")
  -- the sandbox moved HOME; cargo's checkouts are under the real one
  local cargo_home = vim.env.CARGO_HOME or vim.fs.joinpath(vim.uv.os_get_passwd().homedir, ".cargo")
  local checkouts = vim.fn.glob(vim.fs.joinpath(cargo_home, "git/checkouts/tree-sitter-dbt-*/*"), false, true)
  local checkout
  for _, dir in ipairs(checkouts) do
    if vim.uv.fs_stat(vim.fs.joinpath(dir, "src", "parser.c")) then
      local head = vim.system({ "git", "-C", dir, "rev-parse", "HEAD" }, { text = true }):wait()
      if vim.trim(head.stdout or "") == ts.REV then
        checkout = dir
      end
    end
  end
  if not checkout or not ts.compiler() then
    print("skip treesitter: no checkout of tree-sitter-dbt " .. ts.REV:sub(1, 12) .. " or no C compiler")
  else
    ts._root = vim.fs.joinpath(H.tmp, "grammar")
    require("cogitator").config.grammar.url = checkout
    local err, finished = nil, false
    ts.install(function(e)
      err, finished = e, true
    end)
    H.wait("treesitter: install builds the pinned grammar", 120000, function()
      return finished
    end)
    local status = ts.status()
    H.check(
      err == nil and status.loadable and not status.stale,
      "treesitter: the installed grammar loads at the pinned revision",
      vim.inspect({ err, status })
    )
    H.check(
      vim.uv.fs_stat(vim.fs.joinpath(ts.paths().queries, "highlights.scm")) ~= nil,
      "treesitter: the grammar's queries are installed with it"
    )
    vim.api.nvim_set_current_buf(bufnr)
    H.check(
      ts.start(bufnr) and vim.treesitter.highlighter.active[bufnr] ~= nil and vim.bo[bufnr].filetype == "sql",
      "treesitter: the dbt grammar highlights the model, the filetype stays sql"
    )
    pcall(vim.treesitter.stop, bufnr)
    require("cogitator").config.grammar.url = nil
    ts._root = nil
  end

  local missing = require("cogitator.health").missing(
    vim.tbl_get(client.server_capabilities, "executeCommandProvider", "commands") or {},
    function(method)
      return client:supports_method(method)
    end
  )
  H.check(#missing == 0, "health: the server offers every command the plugin uses", vim.inspect(missing))

  -- :checkhealth never waits: the doctor report was fetched at attach
  local health = require("cogitator.health")
  H.wait("health: the dbt.doctor report is fetched in the background", 10000, function()
    local cached = health._cache.doctor[client.id]
    return type(cached) == "table" and type(cached.report) == "table"
  end, function()
    return vim.inspect(health._cache.doctor[client.id])
  end)
  t = vim.uv.hrtime()
  vim.cmd("checkhealth cogitator")
  local took = (vim.uv.hrtime() - t) / 1e6
  local report = table.concat(vim.api.nvim_buf_get_lines(0, 0, -1, false), "\n")
  H.check(
    report:find("project 'jaffle_shop'", 1, true) ~= nil and report:find("report from", 1, true) ~= nil,
    "health: :checkhealth shows the cached dbt.doctor report",
    vim.inspect(vim.tbl_filter(function(l)
      return l:find("doctor", 1, true) or l:find("server", 1, true) or l:find("project", 1, true)
    end, vim.split(report, "\n")))
  )
  H.check(took < 1000, "health: :checkhealth does not wait on the server", took)
  vim.cmd("bwipeout!")

  -- ast-grep through the server: the command, :DbtAstGrep, the picker
  vim.api.nvim_set_current_buf(bufnr)
  local found = H.exec_cmd(client, bufnr, "dbt.astGrep", {
    uri = vim.uri_from_bufnr(bufnr),
    pattern = "{{ ref($M) }}",
  }, "astgrep: dbt.astGrep")
  H.check(
    type(found) == "table" and #found.matches == 4 and found.matches[1].env.M == "'stg_customers'",
    "astgrep: the open model's four refs, with $M bound",
    vim.inspect(found and found.matches[1])
  )
  vim.fn.setqflist({}, "r")
  vim.cmd("DbtAstGrep {{ ref('stg_orders') }}")
  H.wait("astgrep: :DbtAstGrep fills the quickfix list from the project", 10000, function()
    return #vim.fn.getqflist() >= 2
  end, function()
    return vim.inspect(vim.fn.getqflist())
  end)
  local qf = vim.fn.getqflist({ title = 1, items = 1 })
  H.check(
    qf.title:find("ast-grep", 1, true) ~= nil and qf.items[1].text:find("stg_orders", 1, true) ~= nil,
    "astgrep: each quickfix line is a match",
    vim.inspect(qf.items[1])
  )
  vim.cmd("cclose")
  vim.api.nvim_set_current_buf(bufnr)

  local source = require("cogitator.pick.sources").build("astgrep", {
    client = client,
    bufnr = bufnr,
    query = require("cogitator.astgrep").parse("{{ ref('stg_payments') }}"),
  })
  local items
  source.items(function(i)
    items = i
  end)
  H.wait("astgrep: the picker source lists the matches", 10000, function()
    return items ~= nil
  end)
  H.check(
    items and #items >= 1 and items[1].lnum ~= nil and items[1].text:find("stg_payments", 1, true) ~= nil,
    "astgrep: a picker item points at its line",
    vim.inspect(items and items[1])
  )
  local shown
  source.preview(items[1], function(p)
    shown = p
  end)
  H.wait("astgrep: the preview of a match", 5000, function()
    return shown ~= nil
  end)
  H.check(
    shown and shown.code == "dbt",
    "astgrep: the preview is coloured as dbt",
    vim.inspect(shown and shown.code)
  )

  -- highlight on open (specs/highlighting §7.1): a YAML property file
  local tshl = require("cogitator.tshl")
  local schema = vim.fs.joinpath(ctx.fixture, "models", "schema.yml")
  vim.cmd.edit(schema)
  local ybuf = vim.api.nvim_get_current_buf()
  local function groups(buf)
    local out = {}
    for _, m in ipairs(tshl.marks(buf)) do
      out[m[4].hl_group] = (out[m[4].hl_group] or 0) + 1
    end
    return out
  end
  H.wait("highlight: a YAML file is coloured by the server on open", 15000, function()
    local g = groups(ybuf)
    return g["@type.dbt.model"] and g["@property.dbt.column"] and g["@function.dbt.test"]
  end, function()
    return vim.inspect(groups(ybuf))
  end)
  H.check(vim.b[ybuf].cogitator_tshl == "auto", "highlight: auto mode painted it", vim.b[ybuf].cogitator_tshl)

  -- an older answer is dropped: of two requests, only the second paints
  local painted = {}
  tshl.paint(ybuf, { text = "models:\n  - name: old\n", language = "yaml" }, {
    on_done = function()
      painted[#painted + 1] = "old"
    end,
  })
  tshl.paint(ybuf, { uri = vim.uri_from_bufnr(ybuf) }, {
    on_done = function()
      painted[#painted + 1] = "new"
    end,
  })
  H.wait("highlight: the newer answer paints", 10000, function()
    return #painted > 0
  end)
  vim.wait(300)
  H.check(vim.deep_equal(painted, { "new" }), "highlight: an older answer is dropped", vim.inspect(painted))

  -- an edit is painted again, after the debounce
  -- a new source table at the end of the file
  vim.api.nvim_buf_set_lines(ybuf, -1, -1, false, { "      - name: brand_new_table" })
  vim.api.nvim_exec_autocmds("TextChanged", { buffer = ybuf })
  H.wait("highlight: an edit is coloured again", 10000, function()
    local last = vim.api.nvim_buf_line_count(ybuf) - 1
    for _, m in ipairs(tshl.marks(ybuf)) do
      if m[2] == last and m[4].hl_group == "@type.dbt.source_table" then
        return true
      end
    end
    return false
  end)
  vim.cmd("silent! undo")

  vim.cmd("DbtHighlight! off")
  H.check(
    vim.b[ybuf].cogitator_tshl == nil and #tshl.marks(ybuf) == 0,
    "highlight: ! off takes the colours off"
  )
  vim.cmd("DbtHighlight! auto")
  H.wait("highlight: ! auto paints again", 10000, function()
    return #tshl.marks(ybuf) > 0
  end)
  local mode, bufs = tshl.status()
  H.check(mode == "auto" and vim.tbl_contains(bufs, ybuf), "highlight: status lists the painted buffer")

  -- picker previews of macros and of tests in YAML (§8)
  vim.api.nvim_set_current_buf(bufnr)
  local sources = require("cogitator.pick.sources")
  local function first_preview(name, pick_item)
    local src = sources.build(name, { client = client, bufnr = bufnr })
    local listed
    src.items(function(i)
      listed = i
    end)
    H.wait("preview: the " .. name .. " source lists items", 10000, function()
      return listed ~= nil
    end)
    local item = listed and pick_item(listed)
    if not item then
      return nil
    end
    local previewed
    src.preview(item, function(p)
      previewed = p
    end)
    H.wait("preview: the " .. name .. " preview", 5000, function()
      return previewed ~= nil
    end)
    return previewed
  end
  local macro_preview = first_preview("macros", function(list)
    for _, i in ipairs(list) do
      if i.name == "times_five" then
        return i
      end
    end
  end)
  H.check(
    macro_preview and macro_preview.code == "dbt" and macro_preview.lnum and macro_preview.end_lnum,
    "preview: a macro lands on its definition and is coloured",
    vim.inspect(macro_preview and { macro_preview.code, macro_preview.lnum, macro_preview.end_lnum })
  )
  local test_preview = first_preview("tests", function(list)
    return list[1]
  end)
  H.check(
    test_preview and test_preview.code == "yaml" and test_preview.lnum,
    "preview: a YAML test is coloured as YAML and lands on its entry",
    vim.inspect(test_preview and { test_preview.code, test_preview.lnum })
  )
  if macro_preview then
    local pbuf = vim.api.nvim_create_buf(false, true)
    vim.api.nvim_buf_set_lines(pbuf, 0, -1, false, macro_preview.lines)
    tshl.preview(pbuf, macro_preview)
    H.wait("preview: the macro preview is painted", 10000, function()
      return #tshl.marks(pbuf) > 0
    end)
    H.check(
      #vim.api.nvim_buf_get_extmarks(pbuf, tshl.def_ns, 0, -1, {}) > 0,
      "preview: the definition's lines are marked"
    )
    vim.api.nvim_buf_delete(pbuf, { force = true })
  end

  -- :DbtNode: a card in a float
  local wins = #vim.api.nvim_list_wins()
  vim.cmd("DbtNode macro:times_five")
  H.wait(":DbtNode opens the macro's card", 10000, function()
    return #vim.api.nvim_list_wins() > wins
  end)
  local float_text = table.concat(vim.api.nvim_buf_get_lines(0, 0, -1, false), "\n")
  H.check(
    float_text:find("times_five(int_value)", 1, true) ~= nil,
    ":DbtNode shows the signature",
    float_text
  )
  H.feed("q")
  vim.api.nvim_set_current_buf(bufnr)
  vim.cmd("DbtNode assert_positive_order_amount")
  H.wait(":DbtNode finds a test by name alone", 10000, function()
    return table.concat(vim.api.nvim_buf_get_lines(0, 0, -1, false), "\n"):find("tests (1)", 1, true) ~= nil
  end)
  H.feed("q")
  vim.api.nvim_set_current_buf(bufnr)
end
