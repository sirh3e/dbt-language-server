-- Nothing in the plugin makes the editor wait: the request helper's
-- timeout, staleness and cancel rules on a fake client, the chunked DAG
-- painter, and a scan of the plugin for blocking calls.

return function(ctx)
  local H = ctx.H
  local actions = require("cogitator.actions")

  --- A client whose answers the test hands out by hand.
  local function fake_client()
    local c = { pending = {}, cancelled = {} }
    function c.request(_, method, _, handler)
      local id = #c.pending + 1
      c.pending[id] = { method = method, handler = handler }
      return true, id
    end
    function c.cancel_request(_, id)
      c.cancelled[#c.cancelled + 1] = id
    end
    function c.answer(id, err, result)
      c.pending[id].handler(err, result)
    end
    return c
  end

  -- an answer arrives once
  local client = fake_client()
  local got = {}
  actions.request(client, 0, "m", {}, {}, function(err, result, reason)
    got[#got + 1] = { err, result, reason }
  end)
  client.answer(1, nil, "ok")
  client.answer(1, nil, "again")
  H.check(#got == 1 and got[1][2] == "ok", "async: an answer arrives once", vim.inspect(got))

  -- a timeout, then a late answer that is dropped
  client, got = fake_client(), {}
  actions.request(client, 0, "m", {}, { timeout_ms = 20 }, function(_, result, reason)
    got[#got + 1] = reason or result
  end)
  H.wait("async: a request times out", 2000, function()
    return #got == 1
  end)
  client.answer(1, nil, "late")
  H.check(
    vim.deep_equal(got, { "timeout" }),
    "async: a late answer after a timeout is dropped",
    vim.inspect(got)
  )

  -- a stale answer is dropped
  client, got = fake_client(), {}
  actions.request(client, 0, "m", {}, {
    stale = function()
      return true
    end,
  }, function()
    got[#got + 1] = true
  end)
  client.answer(1, nil, "x")
  H.check(#got == 0, "async: an answer that no longer applies is dropped")

  -- cancel stops waiting and tells the server
  client, got = fake_client(), {}
  local cancel = actions.request(client, 0, "m", {}, { timeout_ms = 20 }, function()
    got[#got + 1] = true
  end)
  cancel()
  client.answer(1, nil, "x")
  vim.wait(60)
  H.check(
    #got == 0 and vim.deep_equal(client.cancelled, { 1 }),
    "async: cancel drops the answer and cancels the request",
    vim.inspect({ got, client.cancelled })
  )

  -- the chunked painter: a large graph returns at once and paints it all
  local paint = require("cogitator.dag_paint")
  local function graph(n)
    local boxes, decor = {}, { boxes = {}, edges = {} }
    for i = 1, n do
      local id = "m" .. i
      boxes[id] = { node = { kind = "model", uri = "file:///m.sql" } }
      decor.boxes[id] = { { i - 1, 0, 4 } }
      decor.edges[i] = { { i - 1, 5, 7 } }
    end
    return boxes, decor
  end
  local buf = vim.api.nvim_create_buf(false, true)
  local lines = {}
  for i = 1, 3000 do
    lines[i] = string.rep("x", 10) .. i
  end
  vim.api.nvim_buf_set_lines(buf, 0, -1, false, lines)
  local function marks()
    return #vim.api.nvim_buf_get_extmarks(buf, paint.STATIC_NS, 0, -1, {})
  end
  -- slices of the smallest size, so the slicing does not depend on how
  -- fast this machine paints
  local slice_ms = paint.SLICE_MS
  paint.SLICE_MS = 0
  local boxes, decor = graph(3000)
  local done = false
  local t = vim.uv.hrtime()
  paint.static_async(buf, nil, boxes, decor, function()
    done = true
  end)
  local returned_ms = (vim.uv.hrtime() - t) / 1e6
  H.check(not done and marks() < 6000, "async: a large graph is painted in slices", marks())
  H.check(returned_ms < 50, "async: painting a large graph returns at once", returned_ms)
  H.wait("async: every range of a large graph is painted", 10000, function()
    return done
  end)
  H.check(marks() == 6000, "async: no range is left unpainted", marks())

  -- a newer paint stops an older one
  local first_done, second_done = false, false
  paint.static_async(buf, nil, boxes, decor, function()
    first_done = true
  end)
  paint.static_async(buf, nil, boxes, decor, function()
    second_done = true
  end)
  H.wait("async: the newer paint finishes", 10000, function()
    return second_done
  end)
  vim.wait(50)
  H.check(not first_done and marks() == 6000, "async: a newer paint stops the older one", marks())

  paint.SLICE_MS = slice_ms

  -- a small graph paints at once, as before
  local small_boxes, small_decor = graph(10)
  local small_done = false
  paint.static_async(buf, nil, small_boxes, small_decor, function()
    small_done = true
  end)
  H.check(small_done and marks() == 20, "async: a small graph paints synchronously", marks())
  vim.api.nvim_buf_delete(buf, { force = true })

  -- column completion answers at once, from what was fetched
  local dispatch = require("cogitator.dispatch")
  local probe = vim.api.nvim_create_buf(false, true)
  vim.api.nvim_set_current_buf(probe)
  dispatch._columns[probe] = { tick = vim.api.nvim_buf_get_changedtick(probe), names = { "a", "b" } }
  t = vim.uv.hrtime()
  local names = dispatch.column_names()
  H.check(
    vim.deep_equal(names, { "a", "b" }) and (vim.uv.hrtime() - t) / 1e6 < 50,
    "async: column completion answers from the cache at once",
    vim.inspect(names)
  )
  H.check(#dispatch.column_names() == 2, "async: no client needed to answer from the cache")
  vim.api.nvim_buf_delete(probe, { force = true })

  -- the plugin never blocks: no synchronous requests or waits
  local plugin = vim.fs.joinpath(ctx.repo_root, "editors/nvim")
  local blocking = {}
  for _, dir in ipairs({ "lua", "plugin" }) do
    for name, kind in vim.fs.dir(vim.fs.joinpath(plugin, dir), { depth = 10 }) do
      if kind == "file" and name:match("%.lua$") then
        local path = vim.fs.joinpath(plugin, dir, name)
        for n, text in ipairs(vim.fn.readfile(path)) do
          local code = text:gsub("%-%-.*$", "")
          if
            code:find("request_sync", 1, true)
            or code:find("vim.wait(", 1, true)
            or code:find(":wait(", 1, true)
          then
            blocking[#blocking + 1] = ("%s/%s:%d: %s"):format(dir, name, n, vim.trim(text))
          end
        end
      end
    end
  end
  H.check(
    #blocking == 0,
    "async: the plugin makes no synchronous request or wait (use actions.request / callbacks)",
    table.concat(blocking, "\n")
  )
end
