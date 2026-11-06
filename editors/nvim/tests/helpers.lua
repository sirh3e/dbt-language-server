-- Shared helpers for the headless suite: deterministic waits (every wait
-- is labeled and FAILS at its deadline — no stale-state assertions
-- later), a sandboxed HOME, and a per-run fixture copy so the shared
-- testdata/ tree is never mutated.

local H = {}

H.failures = 0
local cleanups = {}

function H.check(ok, label, detail)
  if ok then
    print("ok   " .. label)
  else
    H.failures = H.failures + 1
    print("FAIL " .. label .. (detail and (" — " .. tostring(detail)) or ""))
  end
  return ok
end

--- A labeled, bounded wait: polls `pred` until it holds or `ms` elapse.
--- Timing out FAILS the step right here, so a later assertion never
--- reports stale state with a misleading message.
function H.wait(label, ms, pred, detail)
  local ok = vim.wait(ms, pred, 50)
  if not ok then
    H.failures = H.failures + 1
    local extra = detail and (" — " .. tostring(type(detail) == "function" and detail() or detail)) or ""
    print("FAIL " .. label .. " (timed out after " .. ms .. "ms)" .. extra)
  end
  return ok
end

--- Register a cleanup to run in H.finish() (also on failure).
function H.defer(fn)
  cleanups[#cleanups + 1] = fn
end

--- Point every home-ish env var at a throwaway dir BEFORE the server
--- starts, so neither the plugin nor the server sees the real ~/.dbt.
function H.sandbox()
  local dir = vim.fs.joinpath(vim.uv.os_tmpdir(), "cogitator-nvim-" .. vim.uv.os_getpid())
  vim.fn.delete(dir, "rf")
  vim.fn.mkdir(dir, "p")
  local home = vim.fs.joinpath(dir, "home")
  vim.fn.mkdir(home, "p")
  vim.env.HOME = home
  vim.env.USERPROFILE = home
  vim.env.XDG_CONFIG_HOME = vim.fs.joinpath(home, ".config")
  vim.env.XDG_DATA_HOME = vim.fs.joinpath(home, ".local", "share")
  vim.env.XDG_STATE_HOME = vim.fs.joinpath(home, ".local", "state")
  vim.env.XDG_CACHE_HOME = vim.fs.joinpath(home, ".cache")
  vim.env.DBT_PROFILES_DIR = nil
  H.defer(function()
    vim.fn.delete(dir, "rf")
  end)
  H.tmp = dir
  return dir
end

--- Copy the jaffle fixture (minus target/) into the sandbox; the suite
--- runs ONLY on this copy, so probe files and target/ writes are free.
function H.copy_fixture(repo_root)
  local src = vim.fs.joinpath(repo_root, "testdata", "jaffle_shop_duckdb")
  local dst = vim.fs.joinpath(H.tmp, "jaffle_shop_duckdb")
  local function copy(from, to)
    vim.fn.mkdir(to, "p")
    for name, kind in vim.fs.dir(from) do
      if name ~= "target" then
        local f, t = vim.fs.joinpath(from, name), vim.fs.joinpath(to, name)
        if kind == "directory" then
          copy(f, t)
        else
          vim.uv.fs_copyfile(f, t)
        end
      end
    end
  end
  copy(src, dst)
  return dst
end

--- The server binary: COGITATOR_BIN, else the cargo target dir
--- (CARGO_TARGET_DIR-aware, .exe on Windows).
function H.server_bin(repo_root)
  if vim.env.COGITATOR_BIN then
    return vim.env.COGITATOR_BIN
  end
  local target = vim.env.CARGO_TARGET_DIR or vim.fs.joinpath(repo_root, "target")
  local exe = vim.fn.has("win32") == 1 and ".exe" or ""
  return vim.fs.joinpath(target, "debug", "cogitator" .. exe)
end

--- One LSP request through buf_request_sync; an error or timeout FAILS
--- the given label and returns nil.
function H.request(client, bufnr, method, params, label, ms)
  local res = vim.lsp.buf_request_sync(bufnr, method, params, ms or 10000)
  local entry = res and res[client.id]
  if entry == nil or entry.err ~= nil then
    H.failures = H.failures + 1
    print("FAIL " .. label .. " — " .. method .. " errored: " .. vim.inspect(entry and entry.err))
    return nil
  end
  return entry.result
end

--- workspace/executeCommand through client:exec_cmd, waited: returns the
--- result table, false on a server error, or nil on timeout (FAILed).
function H.exec_cmd(client, bufnr, command, arguments, label, ms)
  local out
  client:exec_cmd({ command = command, arguments = { arguments } }, { bufnr = bufnr }, function(err, result)
    out = err == nil and (result == nil and vim.NIL or result) or false
  end)
  H.wait(label .. " [" .. command .. " answers]", ms or 10000, function()
    return out ~= nil
  end)
  if out == vim.NIL then
    return nil, true -- a null result that DID arrive
  end
  return out
end

--- The readiness barrier: dbt.status refresh.count is monotonic, so
--- `count >= n` means the n-th project refresh COMPLETED (hover, lenses
--- and the graph are all ready). Replaces the code-lens proxy.
function H.wait_refresh(client, bufnr, n, ms)
  local reached = false
  local last
  local ok = H.wait("refresh barrier (count >= " .. n .. ")", ms or 30000, function()
    local status
    client:exec_cmd(
      { command = "dbt.status", arguments = { { uri = vim.uri_from_bufnr(bufnr) } } },
      { bufnr = bufnr },
      function(err, result)
        status = err == nil and result or false
      end
    )
    vim.wait(2000, function()
      return status ~= nil
    end, 50)
    last = status
    reached = type(status) == "table" and status.refresh and (status.refresh.count or 0) >= n
    return reached
  end, function()
    return vim.inspect(last):sub(1, 300)
  end)
  return ok and reached
end

--- Summary + cleanup + exit code.
function H.finish()
  for i = #cleanups, 1, -1 do
    pcall(cleanups[i])
  end
  print(H.failures == 0 and "ALL PASSED" or (H.failures .. " failure(s)"))
  os.exit(H.failures == 0 and 0 or 1)
end

return H
