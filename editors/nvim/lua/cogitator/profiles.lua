--- dbt profiles helpers: the candidate list (kept in sync with the
--- server's util::profiles_candidates), the :DbtProfiles report, and
--- switching the directory at runtime — live through dbt.setProfilesDir
--- when the server has it, with a restart fallback for older builds.

local actions = require("cogitator.actions")

local M = {}

--- The effective profiles dir for the current buffer's project: the
--- manager store's per-root selection wins over the global option.
local function effective_dir(config)
  local root = vim.fs.root(0, config.root_markers or { "dbt_project.yml" })
  local stored = require("cogitator.manager").for_root(root)
  return stored.profiles_dir or config.profiles_dir, root
end

--- Candidate profiles.yml paths in the server's precedence order.
function M.candidates(config, root)
  local list = {}
  if config.profiles_dir and config.profiles_dir ~= "" then
    table.insert(list, vim.fs.joinpath(vim.fn.expand(config.profiles_dir), "profiles.yml"))
  end
  local env = vim.env.DBT_PROFILES_DIR
  if env and env ~= "" then
    table.insert(list, vim.fs.joinpath(env, "profiles.yml"))
  end
  if root then
    table.insert(list, vim.fs.joinpath(root, "profiles.yml"))
    table.insert(list, vim.fs.joinpath(root, ".dbt", "profiles.yml"))
  end
  local home = vim.env.HOME or vim.env.USERPROFILE
  if home and home ~= "" then
    table.insert(list, vim.fs.joinpath(home, ".dbt", "profiles.yml"))
  end
  return list
end

--- The resolution report shown by :DbtProfiles without an argument.
function M.report(config)
  local dir, root = effective_dir(config)
  local lines = {
    "dbt profiles directory: " .. (dir or "(not set — server search order)"),
  }
  local found = false
  local view = vim.tbl_extend("force", {}, config, { profiles_dir = dir })
  for _, candidate in ipairs(M.candidates(view, root)) do
    local exists = vim.uv.fs_stat(candidate) ~= nil
    local marker = "   "
    if exists and not found then
      marker, found = " ✓ ", true
    end
    table.insert(lines, marker .. candidate)
  end
  if not found then
    table.insert(lines, "no profiles.yml found — the SQL dialect stays generic")
  end
  vim.notify(table.concat(lines, "\n"), found and vim.log.levels.INFO or vim.log.levels.WARN)
end

--- :DbtTarget — fetch the dbt.profiles inventory and switch the active
--- profile/target through dbt.setTarget (no restart needed: the server
--- re-resolves the dialect and passes --target to dbt runs).
function M.pick_target()
  local bufnr = vim.api.nvim_get_current_buf()
  local client = actions.client_for(bufnr)
  if not client then
    return
  end
  client:exec_cmd({
    command = "dbt.profiles",
    arguments = { { uri = vim.uri_from_bufnr(bufnr) } },
  }, { bufnr = bufnr }, function(err, report)
    if err or not report or report == vim.NIL then
      vim.notify("dbt.profiles failed" .. (err and (": " .. err.message) or ""), vim.log.levels.ERROR)
      return
    end
    local choices = {}
    for _, profile in ipairs(report.profiles or {}) do
      if profile.allowed then
        for _, target in ipairs(profile.targets or {}) do
          local issues = (target.auth and target.auth.issues) or {}
          local active = profile.name == report.activeProfile and target.name == report.activeTarget
          table.insert(choices, {
            profile = profile.name,
            target = target.name,
            label = ("%s/%s — %s, %s%s%s"):format(
              profile.name,
              target.name,
              target.type ~= vim.NIL and target.type or "?",
              (target.auth and target.auth.method) or "?",
              #issues > 0 and ("  ⚠ " .. #issues .. " issue" .. (#issues > 1 and "s" or "")) or "",
              active and "  (active)" or ""
            ),
          })
        end
      end
    end
    if #choices == 0 then
      vim.notify(
        "No readable targets. Reason: only the project's profile (and the allowlist) is exposed.\n"
          .. "Solution: check profiles.yml, or widen profiles_allow.",
        vim.log.levels.WARN
      )
      return
    end
    vim.ui.select(choices, {
      prompt = "dbt target",
      format_item = function(c)
        return c.label
      end,
    }, function(choice)
      if not choice then
        return
      end
      client:exec_cmd({
        command = "dbt.setTarget",
        arguments = { { target = choice.target, profile = choice.profile } },
      }, { bufnr = bufnr }, function(set_err, result)
        -- persist the selection for this project so the next session's
        -- attach injects it through initializationOptions
        if not set_err and result and result ~= vim.NIL and client.root_dir then
          require("cogitator.manager").remember(client.root_dir, {
            profile = result.profile,
            target = result.target,
          })
        end
      end)
    end)
  end)
end

--- Switch the profiles directory: live through dbt.setProfilesDir when
--- the attached server advertises it (per-project, persisted), otherwise
--- the old restart flow (older servers; initializationOptions only apply
--- at initialize).
function M.set(dir)
  local cogitator = require("cogitator")
  if not cogitator.config then
    vim.notify("cogitator.nvim: call setup() first", vim.log.levels.ERROR)
    return
  end
  local expanded = vim.fn.expand(dir)
  if vim.uv.fs_stat(vim.fs.joinpath(expanded, "profiles.yml")) == nil then
    vim.notify(
      "No profiles.yml in " .. expanded .. " — the server will fall through its search order.",
      vim.log.levels.WARN
    )
  end

  local bufnr = vim.api.nvim_get_current_buf()
  local client = actions.client_for(bufnr, true)
  local commands = client
    and vim.tbl_get(client.server_capabilities, "executeCommandProvider", "commands")
  if client and vim.tbl_contains(commands or {}, "dbt.setProfilesDir") then
    client:exec_cmd({
      command = "dbt.setProfilesDir",
      arguments = { { dir = expanded } },
    }, { bufnr = bufnr }, function(err, result)
      if err or not result or result == vim.NIL then
        vim.notify(
          "dbt.setProfilesDir failed" .. (err and (": " .. err.message) or ""),
          vim.log.levels.ERROR
        )
        return
      end
      if client.root_dir then
        -- the switch reset the server's profile/target overrides too
        require("cogitator.manager").remember(client.root_dir, {
          profiles_dir = expanded,
          profile = vim.NIL,
          target = vim.NIL,
        })
      end
    end)
    return
  end

  -- fallback: old server without the command — global option + restart
  cogitator.config.profiles_dir = expanded
  cogitator.register(cogitator.config)

  -- restart: disable (stops clients), wait until they are really gone so
  -- re-enabling cannot reuse a still-stopping client, then enable again
  local clients = vim.lsp.get_clients({ name = "cogitator" })
  vim.lsp.enable("cogitator", false)
  local function reenable(tries)
    for _, c in ipairs(clients) do
      if not c:is_stopped() then
        if tries > 0 then
          return vim.defer_fn(function()
            reenable(tries - 1)
          end, 100)
        end
        c:stop(true)
      end
    end
    vim.defer_fn(function()
      vim.lsp.enable("cogitator")
      vim.notify("cogitator restarted with profiles dir " .. expanded, vim.log.levels.INFO)
    end, 50)
  end
  reenable(30)
end

return M
