--- Persisted project/profile manager: a JSON store of every dbt project
--- root ever attached, with optional per-root profiles_dir / profile /
--- target selections. Selections are injected as initializationOptions on
--- the next attach (before_init) and applied live through
--- dbt.setProfilesDir / dbt.setTarget while a client runs.

local M = {}

local function store_path()
  local cogitator = package.loaded["cogitator"]
  local configured = cogitator and cogitator.config and cogitator.config.manager_path
  if configured and configured ~= "" then
    return vim.fn.expand(configured)
  end
  return vim.fs.joinpath(vim.fn.stdpath("data"), "cogitator", "manager.json")
end

local function read()
  local ok, lines = pcall(vim.fn.readfile, store_path())
  if not ok or #lines == 0 then
    return { version = 1, projects = {} }
  end
  local decoded, data = pcall(vim.json.decode, table.concat(lines, "\n"), {
    luanil = { object = true, array = true },
  })
  if not decoded or type(data) ~= "table" or type(data.projects) ~= "table" then
    return { version = 1, projects = {} }
  end
  return data
end

local function write(data)
  local path = store_path()
  pcall(vim.fn.mkdir, vim.fs.dirname(path), "p")
  local ok, encoded = pcall(vim.json.encode, data)
  if ok then
    pcall(vim.fn.writefile, vim.split(encoded, "\n"), path)
  end
end

--- The stored selections for one project root ({} when unknown).
function M.for_root(root)
  if not root or root == "" then
    return {}
  end
  local entry = read().projects[root]
  return type(entry) == "table" and entry or {}
end

--- Merge `tbl` into the root's entry and bump last_used.
function M.remember(root, tbl)
  if not root or root == "" then
    return
  end
  local data = read()
  local entry = data.projects[root]
  if type(entry) ~= "table" then
    entry = {}
  end
  for key, value in pairs(tbl or {}) do
    -- vim.NIL clears a stored key (plain nil is invisible to pairs())
    entry[key] = value ~= vim.NIL and value or nil
  end
  entry.last_used = os.time()
  data.projects[root] = entry
  write(data)
end

--- Known project roots, most recently used first.
function M.roots()
  local projects = read().projects
  local list = {}
  for root, entry in pairs(projects) do
    if type(entry) == "table" then
      table.insert(list, root)
    end
  end
  table.sort(list, function(a, b)
    local la = projects[a].last_used or 0
    local lb = projects[b].last_used or 0
    if la ~= lb then
      return la > lb
    end
    return a < b
  end)
  return list
end

--- :DbtProjects — pick a known project; opening its dbt_project.yml lets
--- vim.lsp.enable attach the per-root client (one server per root).
function M.pick_project()
  local current = vim.fs.root(0, { "dbt_project.yml" })
  local roots = M.roots()
  if #roots == 0 then
    vim.notify(
      "No dbt projects remembered yet — open a file in one and it registers itself.",
      vim.log.levels.INFO
    )
    return
  end
  vim.ui.select(roots, {
    prompt = "dbt project",
    format_item = function(root)
      return root .. (root == current and "  (current)" or "")
    end,
  }, function(root)
    if root then
      vim.cmd.edit(vim.fs.joinpath(root, "dbt_project.yml"))
    end
  end)
end

M._store_path = store_path -- test hook

return M
