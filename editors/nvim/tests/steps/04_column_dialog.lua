-- :DbtColumnLineage's dialog on the booted session over jaffle_shop: the
-- Miller panes, the live previews (dbt SQL and compiled SQL scrolled to
-- the column, the column marked there and in the open model buffer),
-- moving, walking, the fzf filter, <CR> and the DAG drawing beside it.

return function(ctx)
  local H, bufnr = ctx.H, ctx.bufnr
  local dialog = require("cogitator.column_dialog")
  local preview = require("cogitator.column_dialog.preview")

  -- dbt would write target/compiled at runtime; fake it on the fixture copy
  local compiled_dir = vim.fs.joinpath(ctx.fixture, "target", "compiled", "jaffle_shop", "models")
  vim.fn.mkdir(compiled_dir, "p")
  vim.fn.writefile({
    "with customers as (select * from main.stg_customers)",
    "select",
    "    customers.customer_id,",
    "    customers.first_name",
    "from customers",
  }, vim.fs.joinpath(compiled_dir, "customers.sql"))

  local function lines(pane)
    return table.concat(vim.api.nvim_buf_get_lines(dialog._state.bufs[pane], 0, -1, false), "\n")
  end
  local function hits(buf)
    return vim.api.nvim_buf_get_extmarks(buf, preview.NS, 0, -1, { details = true })
  end
  local function hit_line(buf)
    for _, m in ipairs(hits(buf)) do
      if m[4].hl_group == "DbtColumnHit" then
        return m[2] + 1
      end
    end
  end

  vim.api.nvim_set_current_buf(bufnr)
  local src_win = vim.api.nvim_get_current_win()
  vim.cmd("DbtColumnLineage customer_id")
  H.check(dialog._state ~= nil, "column dialog: opens at once, before any answer")
  if
    not H.wait("column dialog: the walk fills the panes", 20000, function()
      local s = dialog._state
      return s and not s.loading and dialog.current() ~= nil
    end, function()
      return dialog._state and vim.inspect({ dialog._state.loading, dialog._state.note })
    end)
  then
    return dialog.close()
  end
  local s = dialog._state
  H.check(
    dialog.current().id == "model:customers::customer_id",
    "column dialog: the cursor starts on the asked column",
    dialog.current().id
  )
  H.check(vim.api.nvim_get_current_win() == s.wins.center, "column dialog: focus is in the middle pane")
  local left = require("cogitator.column_dialog.rows").panes(s.ix, s.nav).left
  H.check(
    left[1]
      and left[1].id == "model:stg_customers::customer_id"
      and lines("left"):find("stg_c", 1, true) ~= nil,
    "column dialog: the left pane lists what feeds the column",
    lines("left")
  )
  H.check(
    lines("center"):find("▦ customers.customer_id", 1, true) ~= nil,
    "column dialog: rows are icon + model.column",
    lines("center")
  )

  H.wait("column dialog: the dbt SQL preview marks the column", 10000, function()
    local l = hit_line(s.bufs.raw)
    return l
      and vim.api.nvim_buf_get_lines(s.bufs.raw, l - 1, l, false)[1]:find("customers.customer_id", 1, true)
  end, function()
    return lines("raw"):sub(1, 200)
  end)
  H.check(
    vim.api.nvim_win_get_cursor(s.wins.raw)[1] == hit_line(s.bufs.raw),
    "column dialog: the dbt SQL preview is scrolled to the column"
  )
  H.wait("column dialog: the compiled SQL preview marks the column", 10000, function()
    return lines("compiled"):find("main.stg_customers", 1, true) and hit_line(s.bufs.compiled) == 3
  end, function()
    return lines("compiled")
  end)
  H.check(hit_line(bufnr) ~= nil, "column dialog: the column is marked in the open model buffer too")

  -- j: the previews follow the cursor
  local before = hit_line(s.bufs.raw)
  -- (feedkeys "x" runs no main loop between keys, so CursorMoved is fired by hand)
  local function moved()
    vim.api.nvim_exec_autocmds("CursorMoved", { buffer = s.bufs.center })
  end
  H.feed("j")
  moved()
  H.wait("column dialog: j moves and the previews follow", 5000, function()
    return dialog.current().id ~= "model:customers::customer_id" and hit_line(s.bufs.raw) ~= before
  end, function()
    return vim.inspect({ dialog.current().id, hit_line(s.bufs.raw), before })
  end)
  H.feed("k")
  moved()

  -- the filter: fzf's syntax, <CR> keeps it, <Esc> clears it
  H.feed("/first_n<CR>")
  H.check(
    s.nav.filter == "first_n" and #s.rows == 1 and dialog.current().column == "first_name",
    "column dialog: / filters the list",
    vim.inspect({ s.nav.filter, #s.rows })
  )
  H.check(
    vim.api.nvim_get_current_win() == s.wins.center,
    "column dialog: <CR> in the prompt returns to the list"
  )
  H.feed("<Esc>")
  H.check(s.nav.filter == "" and #s.rows > 1, "column dialog: <Esc> clears the filter first")
  H.check(dialog._state == s, "column dialog: … and does not close")

  -- h / l: walk, and back to the column one came from
  dialog._state.nav = require("cogitator.column_dialog.model").select(s.nav, "model:customers::customer_id")
  dialog.render(s)
  H.feed("h")
  H.check(
    s.nav.list.parents == "model:customers::customer_id"
      and dialog.current().id:find("stg_customers", 1, true),
    "column dialog: h makes the feeders the list",
    vim.inspect(s.nav)
  )
  H.feed("l")
  H.check(
    dialog.current().id == "model:customers::customer_id" and s.nav.list.model == "customers",
    "column dialog: l comes back"
  )

  -- ? help, and it closes again
  H.feed("?")
  H.check(s.help ~= nil and vim.api.nvim_get_current_win() == s.help, "column dialog: ? shows the keys")
  H.feed("q")
  H.check(
    dialog._state == s and vim.api.nvim_get_current_win() == s.wins.center,
    "column dialog: q closes the help only"
  )

  -- <CR>: the column at its definition, the marks cleared
  H.feed("<CR>")
  H.check(dialog._state == nil, "column dialog: <CR> closes it")
  local line = vim.api.nvim_get_current_line()
  H.check(
    vim.api.nvim_get_current_buf() == bufnr and line:find("customers.customer_id,", 1, true) ~= nil,
    "column dialog: <CR> opens the model on the column",
    line
  )
  H.check(#hits(bufnr) == 0, "column dialog: closing clears the marks in the model buffer")
  H.check(
    vim.api.nvim_get_current_win() == src_win,
    "column dialog: focus returns to the window it came from"
  )

  -- dbt.columnLineageGraph's answer, when the server has it: the edge
  -- kinds it sends (a join), the column's range and the compiled file
  local real = package.loaded["cogitator.lineage_graph"]
  local asked
  local uri = vim.uri_from_bufnr(bufnr)
  local compiled_uri = vim.uri_from_fname(vim.fs.joinpath(compiled_dir, "customers.sql"))
  package.loaded["cogitator.lineage_graph"] = {
    supported = function()
      return true
    end,
    fetch = function(buf, opts, cb)
      asked = { buf = buf, opts = opts }
      vim.schedule(function()
        cb({
          cached = false,
          stale = false,
          nodes = {
            {
              id = "model:customers::customer_id",
              name = "customers.customer_id",
              kind = "model",
              model = "customers",
              column = "customer_id",
              uri = uri,
              compiledUri = compiled_uri,
              range = { start = { line = 0, character = 5 }, ["end"] = { line = 0, character = 14 } },
            },
            {
              id = "model:stg_customers::customer_id",
              name = "stg_customers.customer_id",
              kind = "model",
              model = "stg_customers",
              column = "customer_id",
            },
          },
          edges = {
            { from = "model:stg_customers::customer_id", to = "model:customers::customer_id", kind = "join" },
          },
        })
      end)
    end,
  }
  vim.api.nvim_set_current_buf(bufnr)
  vim.cmd("DbtColumnLineage customer_id")
  H.wait("column dialog: dbt.columnLineageGraph's answer fills it", 10000, function()
    return dialog._state and not dialog._state.loading and dialog.current() ~= nil
  end)
  s = dialog._state
  H.check(
    asked and asked.buf == bufnr and asked.opts.column == "customer_id",
    "column dialog: asks lineage_graph for the column",
    vim.inspect(asked)
  )
  H.check(
    s and lines("left"):find("⋈", 1, true) ~= nil,
    "column dialog: an edge the server calls a join is tagged ⋈",
    s and lines("left")
  )
  H.wait("column dialog: the server's range places the column", 5000, function()
    return s and hit_line(s.bufs.raw) == 1 and hit_line(s.bufs.compiled) == 3
  end, function()
    return s and vim.inspect({ hit_line(s.bufs.raw), hit_line(s.bufs.compiled) })
  end)
  dialog.close()
  package.loaded["cogitator.lineage_graph"] = real

  -- the DAG drawing is still there
  local dag = require("cogitator.dag")
  vim.cmd("DbtColumnLineage graph customer_id")
  H.wait("column dialog: `:DbtColumnLineage graph` draws the DAG", 20000, function()
    return dag._state
      and dag._state.mode == "columns"
      and dag._state.selected == "model:customers::customer_id"
  end)
  dag.close()
  vim.api.nvim_set_current_buf(bufnr)
end
