--- :checkhealth cogitator (spec §7). The profiles candidate list mirrors
--- the server's util::profiles_candidates — keep the order in sync.

local M = {}

local health = vim.health

--- What :checkhealth shows, gathered in the background so the check
--- itself never waits: each binary's `--version` (started at setup) and
--- each client's dbt.doctor report (started at attach, again after a
--- refresh). Neovim's health checks write their report synchronously, so
--- they read this and say when something is still being fetched.
---@type {versions: table<string, {code: integer, stdout: string}|"pending">, doctor: table<integer, {report: table?, at: integer}|"pending">}
M._cache = { versions = {}, doctor = {} }

--- Run `bin --version` in the background, once per binary.
---@param bin string
function M.prefetch_version(bin)
  if M._cache.versions[bin] or vim.fn.executable(bin) == 0 then
    return
  end
  M._cache.versions[bin] = "pending"
  local ok = pcall(vim.system, { bin, "--version" }, { text = true }, function(out)
    M._cache.versions[bin] = { code = out.code, stdout = out.stdout or "" }
  end)
  if not ok then
    M._cache.versions[bin] = { code = -1, stdout = "" }
  end
end

--- Start every background check setup() can: the server binary's and
--- the Fusion binary's versions.
---@param config table
function M.prefetch_versions(config)
  M.prefetch_version(config.cmd[1])
  if config.fusion ~= false then
    M.prefetch_version(type(config.fusion) == "string" and config.fusion or "dbt")
  end
end

--- Fetch `client`'s dbt.doctor report in the background.
---@param client vim.lsp.Client
function M.prefetch(client)
  if M._cache.doctor[client.id] == "pending" then
    return
  end
  M._cache.doctor[client.id] = "pending"
  -- an empty object, not `{}`: that encodes as a JSON array, which the
  -- server refuses (every dbt.* command takes one argument object)
  client:exec_cmd({ command = "dbt.doctor", arguments = { vim.empty_dict() } }, {}, function(err, result)
    M._cache.doctor[client.id] = {
      report = not err and result or nil,
      err = err and err.message or nil,
      at = os.time(),
    }
  end)
end

local STILL = "still being checked in the background — run :checkhealth cogitator again"

local function binary_check(cmd)
  local bin = cmd[1]
  if vim.fn.executable(bin) == 0 then
    health.error(("'%s' is not executable or not on PATH"):format(bin), {
      "Download a release asset (cogitator-{linux-amd64,darwin-amd64,darwin-arm64,windows-amd64.exe})",
      "or build from source: cargo build --release -p cogitator",
    })
    return
  end
  M.prefetch_version(bin)
  local out = M._cache.versions[bin]
  if out == "pending" then
    health.info(("%s --version: %s"):format(bin, STILL))
  elseif out.code == 0 then
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
      M.prefetch_version(fusion_bin)
      local out = M._cache.versions[fusion_bin]
      if out == "pending" then
        health.info(("'%s --version': %s"):format(fusion_bin, STILL))
      elseif out.code == 0 and out.stdout:find("dbt%-fusion") then
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

  local grammar = require("cogitator.treesitter").status()
  if not grammar.installed then
    health.info("tree-sitter-dbt grammar not installed in Neovim — :DbtTreesitter install (optional)")
  elseif not grammar.loadable then
    health.warn(
      "tree-sitter-dbt is installed but Neovim cannot load it",
      { ":DbtTreesitter install rebuilds it" }
    )
  elseif grammar.stale then
    health.warn(
      ("tree-sitter-dbt is at %s, the server's is %s"):format(
        tostring(grammar.revision):sub(1, 12),
        grammar.wanted:sub(1, 12)
      ),
      { ":DbtTreesitter install" }
    )
  else
    health.ok("tree-sitter-dbt grammar installed at " .. grammar.wanted:sub(1, 12))
  end

  local mode, painted = require("cogitator.tshl").status()
  health.info(
    ("server highlighting: %s — %d buffer(s) coloured by the server (:DbtHighlight! auto|always|off)"):format(
      mode,
      #painted
    )
  )

  -- the server's own view, when one is attached (dbt.doctor)
  local client = vim.lsp.get_clients({ name = "cogitator" })[1]
  if not client then
    health.info("no live cogitator client — start one for the server-side report (dbt.doctor)")
    return
  end
  M.check_capabilities(client)
  local cached = M._cache.doctor[client.id]
  if cached == nil or cached == "pending" then
    M.prefetch(client)
    health.info("dbt.doctor: " .. STILL)
    return
  end
  local report = cached.report
  if type(report) ~= "table" then
    M.prefetch(client)
    health.warn(
      ("dbt.doctor did not answer %ds ago: %s"):format(
        os.time() - cached.at,
        cached.err or "no report (older server?)"
      ),
      { "asked again — run :checkhealth cogitator once more" }
    )
    return
  end
  health.info(("dbt.doctor report from %ds ago"):format(os.time() - cached.at))
  M.prefetch(client) -- the next :checkhealth shows a newer one
  health.ok(
    ("server %s — project '%s' (%s), graph %d nodes / %d edges, %d dangling"):format(
      report.serverVersion,
      report.projectName,
      report.dialect,
      report.graph.nodes,
      report.graph.edges,
      report.graph.dangling
    )
  )
  if report.dbtBinaryFound then
    health.ok("dbt binary on PATH: " .. report.dbtBinary)
  else
    health.warn("dbt binary not found: " .. report.dbtBinary)
  end
  if report.artifacts.runResultsAgeS ~= vim.NIL and report.artifacts.runResultsAgeS then
    health.ok(
      ("run_results.json is %ds old (drift: %d)"):format(
        report.artifacts.runResultsAgeS,
        report.artifacts.driftCount
      )
    )
  else
    health.info("no target/run_results.json — run dbt once for run-status surfaces")
  end
  health.info(
    ("scan cache persisted: %s · lint: %s · running jobs: %d"):format(
      tostring(report.scanCachePersisted),
      #report.lintEnabled > 0 and table.concat(report.lintEnabled, ",") or "off",
      #report.runningJobs
    )
  )
end

--- The dbt.* commands the plugin calls, and what stops working without
--- each (the server advertises its own in executeCommandProvider).
M.COMMANDS = {
  ["dbt.run"] = ":DbtRun",
  ["dbt.test"] = ":DbtTest",
  ["dbt.compile"] = ":DbtCompile, the explorer's c",
  ["dbt.parse"] = ":DbtParse",
  ["dbt.deps"] = ":DbtDeps",
  ["dbt.sourceFreshness"] = ":DbtSourceFreshness",
  ["dbt.runSelection"] = ":DbtRunSelection, the navigator's <C-r> on a selection",
  ["dbt.select"] = ":DbtSelect, a dbt selection in the navigator's f and /",
  ["dbt.cancel"] = ":DbtCancel",
  ["dbt.preview"] = ":DbtPreview",
  ["dbt.previewCte"] = ":DbtPreviewCte, the 'Preview rows' lenses",
  ["dbt.columnLineage"] = "the column-lineage graph (:DbtColumnLineage, L)",
  ["dbt.showLineage"] = ":DbtLineage",
  ["dbt.goToSchema"] = ":DbtSchema",
  ["dbt.modelGraph"] = ":DbtGraph",
  ["dbt.projectGraph"] = ":DbtDag, :DbtTags",
  ["dbt.modelInfo"] = ":DbtExplorer",
  ["dbt.compiledSql"] = "the explorer's compiled tab",
  ["dbt.relation"] = ":DbtRelation, :DbtQuery",
  ["dbt.modelTests"] = ":DbtTests",
  ["dbt.extractModelYaml"] = ":DbtExtract",
  ["dbt.status"] = ":DbtStatus",
  ["dbt.profiles"] = ":DbtTarget",
  ["dbt.setTarget"] = ":DbtTarget",
  ["dbt.setProfilesDir"] = ":DbtProfiles <dir> without a restart",
  ["dbt.cache"] = ":DbtCache",
  ["dbt.reloadConfig"] = ":DbtReloadConfig",
  ["dbt.impact"] = ":DbtImpact",
  ["dbt.breakingChanges"] = ":DbtBreakingChanges, the lens",
  ["dbt.deadModels"] = ":DbtDeadModels",
  ["dbt.expandJinja"] = ":DbtExpandJinja",
  ["dbt.queryPlan"] = ":DbtPlan",
  ["dbt.doctor"] = "this report's server half",
  ["dbt.syntaxTree"] = ":DbtSyntaxTree",
  ["dbt.highlight"] = "the explorer's SQL panes, picker previews, :DbtHighlight",
  ["dbt.astGrep"] = ":DbtAstGrep, :DbtFind astgrep",
  ["dbt.search"] = ":DbtSearch",
  ["dbt.nodeInfo"] = ":DbtNode, the explorer's macros and tests",
  ["dbt.diffAt"] = ":DbtDiffAt",
  ["dbt.costReport"] = ":DbtCostReport",
  ["dbt.profileColumn"] = ":DbtProfileColumn",
}

--- The LSP methods the plugin's own features need, beyond the stock ones.
M.METHODS = {
  ["textDocument/selectionRange"] = "<leader>dv / <leader>dV (expand and shrink the selection)",
  ["textDocument/codeLens"] = "code lenses",
  ["textDocument/codeAction"] = "code actions",
}

--- Which of the plugin's commands and methods the server does not offer.
---@param advertised string[] executeCommandProvider.commands
---@param supports fun(method: string): boolean
---@return {name: string, feature: string}[]
function M.missing(advertised, supports)
  local have = {}
  for _, name in ipairs(advertised or {}) do
    have[name] = true
  end
  local out = {}
  for name, feature in pairs(M.COMMANDS) do
    if not have[name] then
      out[#out + 1] = { name = name, feature = feature }
    end
  end
  for method, feature in pairs(M.METHODS) do
    if not supports(method) then
      out[#out + 1] = { name = method, feature = feature }
    end
  end
  table.sort(out, function(a, b)
    return a.name < b.name
  end)
  return out
end

--- Report the commands and methods a live server lacks.
---@param client vim.lsp.Client
function M.check_capabilities(client)
  local advertised = vim.tbl_get(client.server_capabilities, "executeCommandProvider", "commands") or {}
  local missing = M.missing(advertised, function(method)
    return client:supports_method(method)
  end)
  if #missing == 0 then
    health.ok(
      ("the server offers every command and method the plugin uses (%d commands)"):format(#advertised)
    )
    return
  end
  for _, m in ipairs(missing) do
    health.warn(("server does not offer %s — %s will not work"):format(m.name, m.feature), {
      "update the cogitator binary (cogitator --version) to one built with this plugin",
    })
  end
end

return M
