--- The tree-sitter-dbt grammar in Neovim: install the revision the
--- server is built with (fetch, compile, copy its queries), then start it
--- on SQL buffers inside a dbt project — the filetype stays `sql`, so
--- every SQL tool keeps working while highlighting sees the Jinja.
---
--- :DbtTreesitter [install|status|start]. The parser lands in
--- stdpath("data")/site/parser/dbt.so and the queries in
--- site/queries/dbt/, both on the default runtimepath.

local M = {}

--- The grammar the server pins (Cargo.toml: tree-sitter-dbt).
M.URL = "https://github.com/sirh3e/tree-sitter-dbt"
M.REV = "ca6ce2d5dec7a8958f2a160637bd2bafa1fcfb98"
M.LANG = "dbt"

local function settings()
  local cogitator = package.loaded["cogitator"]
  local config = cogitator and cogitator.config or require("cogitator.config").build({})
  return config.grammar
end

--- Where installs go; tests point it into their sandbox.
M._root = nil

local function site()
  return M._root and vim.fs.joinpath(M._root, "site") or vim.fs.joinpath(vim.fn.stdpath("data"), "site")
end

local function build_dir()
  return M._root and vim.fs.joinpath(M._root, "build")
    or vim.fs.joinpath(vim.fn.stdpath("cache"), "cogitator", "tree-sitter-dbt")
end

--- Where the parser, the queries and the revision marker go.
---@return {parser: string, queries: string, marker: string}
function M.paths()
  return {
    parser = vim.fs.joinpath(site(), "parser", M.LANG .. ".so"),
    queries = vim.fs.joinpath(site(), "queries", M.LANG),
    marker = vim.fs.joinpath(site(), "parser-info", M.LANG .. ".revision"),
  }
end

--- What is installed and whether Neovim can load it.
---@return {installed: boolean, loadable: boolean, revision: string?, wanted: string, stale: boolean}
function M.status()
  local paths = M.paths()
  local installed = vim.uv.fs_stat(paths.parser) ~= nil
  local ok, lines = pcall(vim.fn.readfile, paths.marker)
  local revision = ok and lines[1] or nil
  local wanted = settings().rev or M.REV
  local loadable = installed and pcall(vim.treesitter.language.add, M.LANG, { path = paths.parser }) or false
  return {
    installed = installed,
    loadable = loadable,
    revision = revision,
    wanted = wanted,
    stale = installed and revision ~= wanted,
  }
end

--- The C compiler to build with: $CC, cc, or zig cc.
---@return string[]?
function M.compiler()
  if vim.env.CC and vim.env.CC ~= "" and vim.fn.executable(vim.env.CC) == 1 then
    return { vim.env.CC }
  end
  for _, cc in ipairs({ "cc", "gcc", "clang" }) do
    if vim.fn.executable(cc) == 1 then
      return { cc }
    end
  end
  if vim.fn.executable("zig") == 1 then
    return { "zig", "cc" }
  end
end

--- The build steps, as argv lists, for a checkout in `dir`.
---@param dir string
---@param cc string[]
---@param out string the parser's path
---@return string[][]
function M.build_commands(dir, cc, out)
  local src = vim.fs.joinpath(dir, "src")
  local cmd = vim.list_extend(vim.deepcopy(cc), {
    "-o",
    out,
    "-shared",
    "-fPIC",
    "-Os",
    "-I" .. src,
    vim.fs.joinpath(src, "parser.c"),
  })
  if vim.uv.fs_stat(vim.fs.joinpath(src, "scanner.c")) then
    table.insert(cmd, vim.fs.joinpath(src, "scanner.c"))
  end
  return { cmd }
end

--- Run argv lists one after another; `done(err)` after the last.
local function chain(steps, done)
  local i = 0
  local function next_step()
    i = i + 1
    local step = steps[i]
    if not step then
      return done(nil)
    end
    vim.system(step.cmd, { cwd = step.cwd, text = true }, function(r)
      vim.schedule(function()
        if r.code ~= 0 then
          return done(("%s failed (%d): %s"):format(step.cmd[1], r.code, vim.trim(r.stderr or "")))
        end
        next_step()
      end)
    end)
  end
  next_step()
end

--- Fetch the pinned revision, compile it and install parser + queries.
---@param done? fun(err: string?)
function M.install(done)
  done = done
    or function(err)
      vim.notify(
        err and ("tree-sitter-dbt install failed: " .. err) or "tree-sitter-dbt installed — :e to highlight",
        err and vim.log.levels.ERROR or vim.log.levels.INFO
      )
    end
  local cc = M.compiler()
  if not cc then
    return done("no C compiler found (set $CC, or install cc or zig)")
  end
  if vim.fn.executable("git") ~= 1 then
    return done("git is not on PATH")
  end
  local opts = settings()
  local rev = opts.rev or M.REV
  local dir = build_dir()
  local paths = M.paths()
  vim.fn.delete(dir, "rf")
  vim.fn.mkdir(dir, "p")
  vim.fn.mkdir(vim.fs.dirname(paths.parser), "p")
  vim.fn.mkdir(vim.fs.dirname(paths.marker), "p")
  local tmp = paths.parser .. ".tmp"
  local steps = {
    { cmd = { "git", "init", "-q" }, cwd = dir },
    { cmd = { "git", "fetch", "-q", "--depth", "1", opts.url or M.URL, rev }, cwd = dir },
    { cmd = { "git", "checkout", "-q", "FETCH_HEAD" }, cwd = dir },
  }
  chain(steps, function(err)
    if err then
      return done(err)
    end
    local build = vim.tbl_map(function(cmd)
      return { cmd = cmd, cwd = dir }
    end, M.build_commands(dir, cc, tmp))
    chain(build, function(build_err)
      if build_err then
        return done(build_err)
      end
      -- a loaded parser cannot be overwritten in place on every OS
      vim.uv.fs_rename(tmp, paths.parser)
      vim.fn.delete(paths.queries, "rf")
      vim.fn.mkdir(paths.queries, "p")
      for name, kind in vim.fs.dir(vim.fs.joinpath(dir, "queries")) do
        if kind == "file" and name:match("%.scm$") then
          vim.uv.fs_copyfile(vim.fs.joinpath(dir, "queries", name), vim.fs.joinpath(paths.queries, name))
        end
      end
      vim.fn.writefile({ rev }, paths.marker)
      done(nil)
    end)
  end)
end

--- Start the dbt grammar on a buffer (no-op when it is not installed).
---@param bufnr integer
---@return boolean started
function M.start(bufnr)
  if vim.bo[bufnr].filetype ~= "sql" then
    return false
  end
  local paths = M.paths()
  if not vim.uv.fs_stat(paths.parser) then
    return false
  end
  if not pcall(vim.treesitter.language.add, M.LANG, { path = paths.parser }) then
    return false
  end
  return pcall(vim.treesitter.start, bufnr, M.LANG)
end

--- On attach: start the grammar when highlighting is on, offering (or,
--- with auto_install, running) the install once per session.
---@param bufnr integer
function M.attach(bufnr)
  local opts = settings()
  if not opts.highlight then
    return
  end
  if M.start(bufnr) then
    return
  end
  if vim.bo[bufnr].filetype ~= "sql" or M._offered then
    return
  end
  M._offered = true
  if opts.auto_install then
    M.install(function(err)
      if err then
        vim.notify("tree-sitter-dbt install failed: " .. err, vim.log.levels.WARN)
      elseif vim.api.nvim_buf_is_valid(bufnr) then
        M.start(bufnr)
      end
    end)
  end
end

--- :DbtTreesitter [install|status|start]
---@param action string
function M.command(action)
  if action == "install" then
    return M.install()
  elseif action == "start" then
    if not M.start(vim.api.nvim_get_current_buf()) then
      vim.notify("tree-sitter-dbt is not installed (:DbtTreesitter install), or this is no sql buffer")
    end
    return
  end
  local s = M.status()
  vim.notify(
    ("tree-sitter-dbt: %s%s (wanted %s)"):format(
      s.installed and (s.loadable and "installed" or "installed but not loadable") or "not installed",
      s.revision and (" at " .. s.revision:sub(1, 12)) or "",
      s.wanted:sub(1, 12)
    ) .. (s.stale and " — :DbtTreesitter install updates it" or "")
  )
end

return M
