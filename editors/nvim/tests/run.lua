-- Headless test suite: `nvim --headless -l tests/run.lua` (spec §8).
-- Deterministic by construction: a sandboxed HOME, a per-run fixture
-- COPY (testdata/ is never touched), the dbt.status refresh barrier as
-- the readiness signal, and labeled waits that fail at their deadline
-- (helpers.lua). The steps run in order; 05 mutates session state and
-- therefore stays last.

local script = vim.uv.fs_realpath(arg[0] or debug.getinfo(1, "S").source:sub(2))
local tests_dir = vim.fs.dirname(script)
local plugin_dir = vim.fs.dirname(tests_dir)
local repo_root = vim.fs.dirname(vim.fs.dirname(plugin_dir))

vim.opt.runtimepath:prepend(plugin_dir)
vim.cmd.runtime("plugin/cogitator.lua")
-- `nvim -l` starts without filetype detection; vim.lsp.enable attaches on
-- the FileType event, so turn it on for the integration half
vim.cmd("filetype on")

local H = dofile(vim.fs.joinpath(tests_dir, "helpers.lua"))

-- sandbox FIRST: the server child inherits vim.env, so HOME/XDG_* must
-- point into the throwaway dir before the client ever spawns
local sandbox = H.sandbox()

local bin = H.server_bin(repo_root)
if not vim.uv.fs_stat(bin) then
  print("FAIL integration: server binary missing at " .. bin .. " (cargo build -p cogitator)")
  os.exit(1)
end

local fixture = H.copy_fixture(repo_root)

require("cogitator").setup({
  cmd = { bin },
  profiles_dir = vim.fs.joinpath(fixture, ".dbt"),
  manager_path = vim.fs.joinpath(sandbox, "manager.json"),
})

local ctx = { H = H, fixture = fixture, repo_root = repo_root }
for _, step in ipairs({ "01_unit", "02_session", "03_features", "04_browser", "05_profiles" }) do
  dofile(vim.fs.joinpath(tests_dir, "steps", step .. ".lua"))(ctx)
end

H.finish()
