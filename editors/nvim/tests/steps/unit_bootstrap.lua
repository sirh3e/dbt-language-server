-- From setup() to an attached server (cogitator.bootstrap): finding the
-- binary, saying why a buffer has no client, and — in a child Neovim —
-- what a user without the binary sees: one warning that says what to do,
-- the keymaps all the same (never Vim's `d` + `n`), and commands that
-- explain themselves instead of doing nothing.

return function(ctx)
  local H = ctx.H
  local bootstrap = require("cogitator.bootstrap")

  -- the binary: as given, else where the installers put a bare one
  local home_bin = vim.fs.joinpath(vim.fn.expand("~"), ".local", "bin", "cogitator")
  local only = function(path)
    return function(candidate)
      return candidate == path
    end
  end
  local cmd, found = bootstrap.resolve_cmd({ "cogitator", "--debug" }, only("cogitator"))
  H.check(found and cmd[1] == "cogitator", "bootstrap: a binary on PATH is used as it is")
  cmd, found = bootstrap.resolve_cmd({ "cogitator", "--debug" }, only(home_bin))
  H.check(
    found and cmd[1] == home_bin and cmd[2] == "--debug",
    "bootstrap: a bare cogitator not on PATH is found where install.sh puts it",
    vim.inspect(cmd)
  )
  cmd, found = bootstrap.resolve_cmd({ "/opt/my/cogitator" }, only(home_bin))
  H.check(
    not found and cmd[1] == "/opt/my/cogitator",
    "bootstrap: a path the user wrote is not swapped for another binary"
  )
  local saved = vim.env.COGITATOR_BIN_DIR
  vim.env.COGITATOR_BIN_DIR = vim.fs.joinpath(H.tmp, "bins")
  cmd, found = bootstrap.resolve_cmd({ "cogitator" }, only(vim.fs.joinpath(H.tmp, "bins", "cogitator")))
  H.check(found, "bootstrap: $COGITATOR_BIN_DIR is looked in first", vim.inspect(cmd))
  vim.env.COGITATOR_BIN_DIR = saved

  -- why a buffer has no client: one sentence that says what to do
  local config = require("cogitator.config").build({})
  local function buffer(path, ft)
    local buf = vim.api.nvim_create_buf(false, true)
    vim.api.nvim_buf_set_name(buf, path)
    vim.bo[buf].filetype = ft
    return buf
  end
  local outside = buffer(vim.fs.joinpath(H.tmp, "loose", "x.sql"), "sql")
  vim.fn.mkdir(vim.fs.joinpath(H.tmp, "loose"), "p")
  local why = bootstrap.why_not_attached(outside, config, true)
  H.check(why:find("not in a dbt project", 1, true) ~= nil, "bootstrap: outside a project, it says so", why)
  local lua_buf = buffer(vim.fs.joinpath(ctx.fixture, "notes.lua"), "lua")
  why = bootstrap.why_not_attached(lua_buf, config, true)
  H.check(
    why:find("sql and yaml", 1, true) ~= nil,
    "bootstrap: in another filetype, it names the ones it serves",
    why
  )
  local model = buffer(vim.fs.joinpath(ctx.fixture, "models", "probe_why.sql"), "sql")
  why = bootstrap.why_not_attached(model, config, false)
  H.check(
    why:find(":DbtInstall", 1, true) ~= nil and why:find("not found", 1, true) ~= nil,
    "bootstrap: without the binary, it names the fix",
    why
  )
  for _, b in ipairs({ outside, lua_buf, model }) do
    vim.api.nvim_buf_delete(b, { force = true })
  end
  H.check(vim.fn.exists(":DbtInstall") == 2, "bootstrap: :DbtInstall is defined")
  H.check(
    vim.uv.fs_stat(vim.fs.joinpath(bootstrap.repo_root(), "install.sh")) ~= nil,
    "bootstrap: the installer it runs is where it looks for it"
  )

  -- the user's case end to end, in a child Neovim: no binary at all
  local plugin_dir = vim.fs.joinpath(ctx.repo_root, "editors", "nvim")
  local script = vim.fs.joinpath(H.tmp, "no-binary.lua")
  local report = vim.fs.joinpath(H.tmp, "no-binary.json")
  vim.fn.writefile({
    "vim.opt.runtimepath:prepend(" .. vim.inspect(plugin_dir) .. ")",
    "vim.cmd.runtime('plugin/cogitator.lua')",
    "vim.cmd('filetype on')",
    "vim.g.mapleader = ' '",
    "local seen = {}",
    "vim.notify = function(msg, level) seen[#seen + 1] = { msg = msg, level = level } end",
    "require('cogitator').setup({ cmd = { 'cogitator-that-is-not-installed' } })",
    "vim.cmd.edit(" .. vim.inspect(vim.fs.joinpath(ctx.fixture, "models", "customers.sql")) .. ")",
    "vim.cmd.edit(" .. vim.inspect(vim.fs.joinpath(ctx.fixture, "models", "orders.sql")) .. ")",
    "vim.wait(300, function() return false end)",
    "local map = vim.fn.maparg(' dn', 'n', false, true)",
    "local before = #seen",
    "vim.cmd('DbtBrowse')",
    "vim.wait(100, function() return false end)",
    "local lines = vim.api.nvim_buf_get_lines(0, 0, -1, false)",
    "vim.fn.writefile({ vim.json.encode({ seen = seen, before = before, mapped = map.buffer == 1,",
    "  clients = #vim.lsp.get_clients(), lines = #lines }) }, " .. vim.inspect(report) .. ")",
    "vim.cmd('qa!')",
  }, script)
  local res = vim
    .system({ vim.v.progpath, "--headless", "--clean", "-l", script }, { text = true })
    :wait(30000)
  local ok_read, got = pcall(function()
    return vim.json.decode(table.concat(vim.fn.readfile(report), "\n"))
  end)
  if
    not H.check(ok_read, "bootstrap: the child Neovim reports", (res.stderr or "") .. (res.stdout or ""))
  then
    return
  end
  local warnings = vim.tbl_filter(function(n)
    return n.msg:find("not found", 1, true) ~= nil
  end, vim.list_slice(got.seen, 1, got.before))
  H.check(
    #warnings == 1 and warnings[1].level == vim.log.levels.WARN,
    "bootstrap: without the binary, opening two models warns once",
    vim.inspect(got.seen)
  )
  H.check(got.mapped, "bootstrap: the keymaps are there without a server (<leader>dn is not `d` + `n`)")
  H.check(got.clients == 0, "bootstrap: no client is started for a binary that is not there", got.clients)
  local browse = got.seen[got.before + 1]
  H.check(
    browse ~= nil and browse.msg:find(":DbtInstall", 1, true) ~= nil,
    "bootstrap: :DbtBrowse without a server says why and what to do",
    vim.inspect(got.seen)
  )
end
