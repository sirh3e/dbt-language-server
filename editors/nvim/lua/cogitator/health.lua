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

local profiles = require("cogitator.profiles")

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
  for _, candidate in ipairs(profiles.candidates(config, root)) do
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

  -- the server's own view, when one is attached (dbt.doctor)
  local client = vim.lsp.get_clients({ name = "cogitator" })[1]
  if not client then
    health.info("no live cogitator client — start one for the server-side report (dbt.doctor)")
    return
  end
  local done, report = false, nil
  client:exec_cmd({ command = "dbt.doctor", arguments = { {} } }, {}, function(err, result)
    done = true
    if not err then
      report = result
    end
  end)
  vim.wait(5000, function()
    return done
  end, 50)
  if type(report) ~= "table" then
    health.warn("dbt.doctor did not answer (older server?)")
    return
  end
  health.ok(("server %s — project '%s' (%s), graph %d nodes / %d edges, %d dangling"):format(
    report.serverVersion,
    report.projectName,
    report.dialect,
    report.graph.nodes,
    report.graph.edges,
    report.graph.dangling
  ))
  if report.dbtBinaryFound then
    health.ok("dbt binary on PATH: " .. report.dbtBinary)
  else
    health.warn("dbt binary not found: " .. report.dbtBinary)
  end
  if report.artifacts.runResultsAgeS ~= vim.NIL and report.artifacts.runResultsAgeS then
    health.ok(("run_results.json is %ds old (drift: %d)"):format(
      report.artifacts.runResultsAgeS,
      report.artifacts.driftCount
    ))
  else
    health.info("no target/run_results.json — run dbt once for run-status surfaces")
  end
  health.info(("scan cache persisted: %s · lint: %s · running jobs: %d"):format(
    tostring(report.scanCachePersisted),
    #report.lintEnabled > 0 and table.concat(report.lintEnabled, ",") or "off",
    #report.runningJobs
  ))
end

return M
