-- Try cogitator.nvim without touching your own config:
--
--   nvim -u minimal_init.lua path/to/your/dbt/project/models/some_model.sql
--
-- lazy.nvim and the plugin go to a throwaway directory under your cache
-- (`:echo stdpath("cache") .. "/cogitator-try"`); delete it to start over.
-- COGITATOR_PLUGIN_DIR=/path/to/a/checkout uses a local checkout instead of
-- GitHub. The server must be on PATH (see the README) or in
-- COGITATOR_SERVER.

local root = vim.fn.stdpath("cache") .. "/cogitator-try"
local lazypath = root .. "/lazy/lazy.nvim"
if not vim.uv.fs_stat(lazypath) then
  local out = vim.fn.system({
    "git",
    "clone",
    "--filter=blob:none",
    "--branch=stable",
    "https://github.com/folke/lazy.nvim.git",
    lazypath,
  })
  if vim.v.shell_error ~= 0 then
    error("cloning lazy.nvim failed:\n" .. out)
  end
end
vim.opt.rtp:prepend(lazypath)

local plugin = { "sirh3e/dbt-language-server" }
if vim.env.COGITATOR_PLUGIN_DIR then
  plugin = { dir = vim.env.COGITATOR_PLUGIN_DIR, name = "dbt-language-server" }
end
plugin.opts = {
  cmd = vim.env.COGITATOR_SERVER and { vim.env.COGITATOR_SERVER } or nil,
  manager_path = root .. "/manager.json",
}

require("lazy").setup({ plugin }, {
  root = root .. "/lazy",
  lockfile = root .. "/lazy-lock.json",
  pkg = { cache = root .. "/pkg-cache.lua" },
  state = root .. "/state.json",
  rocks = { enabled = false },
  change_detection = { enabled = false },
})
