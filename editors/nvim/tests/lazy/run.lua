-- lazy.nvim installs the plugin from this checkout with nothing but its
-- name (`just nvim-lazy-test`): this driver writes a sandboxed init.lua
-- that bootstraps lazy.nvim and lists the plugin, starts a real headless
-- Neovim on it (lazy.nvim installs and loads during startup, which a
-- `-l` script never reaches), and runs check.lua inside it. Needs git
-- and the network (lazy.nvim is cloned from GitHub, as its own bootstrap
-- does; a local mirror of it trips its lockfile, whose `stable` is a tag).
-- COGITATOR_LAZY_SPEC=example installs with editors/nvim/examples/lazy.lua.

local script = vim.uv.fs_realpath(arg[0] or debug.getinfo(1, "S").source:sub(2))
local here = vim.fs.dirname(script)
local repo = vim.fs.dirname(vim.fs.dirname(vim.fs.dirname(vim.fs.dirname(here))))
local sandbox = vim.fn.tempname()
local env = {}
for _, kind in ipairs({ "config", "data", "state", "cache" }) do
  env["XDG_" .. kind:upper() .. "_HOME"] = sandbox .. "/" .. kind
  vim.fn.mkdir(sandbox .. "/" .. kind, "p")
end

local function run(cmd)
  local out = vim.system(cmd, { text = true }):wait()
  return out.code == 0, vim.trim((out.stdout or "") .. (out.stderr or ""))
end

-- lazy.nvim
local lazypath = sandbox .. "/data/nvim/lazy/lazy.nvim"
local source = "https://github.com/folke/lazy.nvim.git"
local cloned, why =
  run({ "git", "clone", "--quiet", "--filter=blob:none", "--branch=stable", source, lazypath })
if not cloned then
  print("SKIP lazy.nvim could not be cloned from " .. source .. ": " .. why)
  os.exit(0)
end

-- the plugin: a git repository of this checkout's working tree, so an
-- uncommitted change is tested too
local plugin = sandbox .. "/dbt-language-server"
run({ "git", "clone", "--quiet", repo, plugin })
for _, path in ipairs({ "lazy.lua", "editors/nvim" }) do
  run({ "rm", "-rf", plugin .. "/" .. path })
  run({ "cp", "-R", repo .. "/" .. path, plugin .. "/" .. path })
end
run({ "rm", "-f", plugin .. "/editors/nvim/doc/tags" })
run({ "git", "-C", plugin, "add", "-A" })
run({
  "git",
  "-C",
  plugin,
  "-c",
  "user.name=t",
  "-c",
  "user.email=t@t",
  "commit",
  "-qm",
  "wt",
  "--allow-empty",
})

-- the user's config: the one-line spec, or the documented example
local spec
if vim.env.COGITATOR_LAZY_SPEC == "example" then
  -- the example as a user copies it, but from this checkout and without
  -- its build step (that downloads a release into ~/.local/bin)
  spec = ("vim.tbl_extend('force', dofile(%q), { url = %q, build = false, opts = { keymaps = false, inlay_hints = true } })"):format(
    repo .. "/editors/nvim/examples/lazy.lua",
    "file://" .. plugin
  )
else
  spec = ("{ url = %q, opts = { keymaps = false, inlay_hints = true } }"):format("file://" .. plugin)
end
local init = sandbox .. "/config/nvim/init.lua"
vim.fn.mkdir(vim.fs.dirname(init), "p")
vim.fn.writefile({
  ("vim.opt.rtp:prepend(%q)"):format(lazypath),
  "require('lazy').setup({ " .. spec .. " }, {",
  "  change_detection = { enabled = false },",
  "  rocks = { enabled = false },",
  "  install = { missing = true },",
  "})",
}, init)

local function nvim(extra)
  local cmd = { "nvim", "--headless", "-u", init }
  vim.list_extend(cmd, extra)
  local out = vim
    .system(cmd, {
      text = true,
      env = vim.tbl_extend(
        "force",
        env,
        { COGITATOR_LAZY_SPEC = vim.env.COGITATOR_LAZY_SPEC or "", COGITATOR_SANDBOX = sandbox }
      ),
    })
    :wait(120000)
  return (out.stdout or "") .. (out.stderr or ""), out.code
end

-- first start installs; the second is an ordinary start, as for a user
local output, code = nvim({ "+lua dofile('" .. here .. "/check.lua')" })
print(output)
local again, code2 = nvim({ "+lua dofile('" .. here .. "/check.lua')" })
print(again)
vim.fn.delete(sandbox, "rf")
os.exit((code == 0 and code2 == 0) and 0 or 1)
