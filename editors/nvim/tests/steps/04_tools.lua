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
end
