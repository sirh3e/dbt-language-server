--- :checkhealth cogitator (spec §7). The profiles candidate list mirrors
--- the server's util::profiles_candidates — keep the order in sync.

local M = {}

local health = vim.health

local function binary_check(cmd)
  local bin = cmd[1]
  if vim.fn.executable(bin) == 0 then
    health.error(("'%s' is not executable or not on PATH"):format(bin), {
      "Download a release asset (cogitator-{linux-amd64,darwin-amd64,darwin-arm64,windows-amd64.exe})",
      "or build from source: cargo build --release -p cogitator",
    })
    return
  end
  local out = vim.system({ bin, "--version" }, { text = true }):wait()
  if out.code == 0 then
    health.ok(("%s --version: %s"):format(bin, vim.trim(out.stdout)))
  else
    health.warn(("'%s --version' exited with %d"):format(bin, out.code))
  end
end

local function profiles_candidates(config, root)
  local candidates = {}
  if config.profiles_dir and config.profiles_dir ~= "" then
    table.insert(candidates, vim.fs.joinpath(vim.fn.expand(config.profiles_dir), "profiles.yml"))
  end
  local env = vim.env.DBT_PROFILES_DIR
  if env and env ~= "" then
    table.insert(candidates, vim.fs.joinpath(env, "profiles.yml"))
  end
  if root then
    table.insert(candidates, vim.fs.joinpath(root, "profiles.yml"))
    table.insert(candidates, vim.fs.joinpath(root, ".dbt", "profiles.yml"))
  end
  local home = vim.env.HOME or vim.env.USERPROFILE
  if home and home ~= "" then
    table.insert(candidates, vim.fs.joinpath(home, ".dbt", "profiles.yml"))
  end
  return candidates
end

function M.check()
  health.start("cogitator.nvim")

  if vim.fn.has("nvim-0.11") == 1 then
    health.ok("Neovim " .. tostring(vim.version()))
  else
    health.error("Neovim 0.11+ required", { "see :h cogitator-nvim-0.10 for the manual recipe" })
  end

  local config = require("cogitator").config
  if not config then
    health.warn("setup() has not been called; checking defaults")
    config = require("cogitator.config").build({})
  end

  binary_check(config.cmd)

  local root = vim.fs.root(0, config.root_markers)
  if root then
    health.ok("dbt_project.yml found: " .. root)
  else
    health.warn("no dbt_project.yml found upward from the current buffer/cwd", {
      "cogitator only attaches inside a dbt project unless attach_outside_project = true",
    })
  end

  local found = false
  for _, candidate in ipairs(profiles_candidates(config, root)) do
    if not found and vim.uv.fs_stat(candidate) then
      health.ok("profiles.yml: " .. candidate)
      found = true
    else
      health.info("candidate: " .. candidate)
    end
  end
  if not found then
    health.warn("no profiles.yml found in any candidate location", {
      "set profiles_dir in setup(), DBT_PROFILES_DIR, or create <root>/.dbt/profiles.yml",
      "the SQL dialect stays generic until a profile target resolves",
    })
  end

  if config.fusion ~= false then
    local fusion_bin = type(config.fusion) == "string" and config.fusion or "dbt"
    if vim.fn.executable(fusion_bin) == 1 then
      local out = vim.system({ fusion_bin, "--version" }, { text = true }):wait()
      if out.code == 0 and out.stdout:find("dbt%-fusion") then
        health.ok("dbt Fusion binary: " .. fusion_bin)
      else
        health.warn(("'%s --version' does not report dbt-fusion"):format(fusion_bin), {
          "the server will disable Fusion features and tell you via a message",
        })
      end
    else
      health.error(("fusion binary '%s' not found"):format(fusion_bin))
    end
  end
end

return M
