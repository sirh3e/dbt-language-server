--- dbt profiles helpers: the candidate list (kept in sync with the
--- server's util::profiles_candidates), the :DbtProfiles report, and
--- switching the directory at runtime (restarts the server, because
--- initializationOptions only apply at initialize).

local M = {}

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
  local root = vim.fs.root(0, config.root_markers or { "dbt_project.yml" })
  local lines = {
    "dbt profiles directory: " .. (config.profiles_dir or "(not set — server search order)"),
  }
  local found = false
  for _, candidate in ipairs(M.candidates(config, root)) do
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

--- Switch the profiles directory and restart cogitator so the server
--- re-initializes with the new initializationOptions.
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
