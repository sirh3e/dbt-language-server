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
---@param root string?
---@return {profiles_dir?: string, profile?: string, target?: string, last_used?: integer}
function M.for_root(root)
  if not root or root == "" then
    return {}
  end
  local entry = read().projects[root]
  return type(entry) == "table" and entry or {}
end

--- Merge `tbl` into the root's entry and bump last_used.
---@param root string?
---@param tbl table<string, any>? vim.NIL values clear their keys
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
---@return string[]
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
  require("cogitator.pick").select(roots, {
    prompt = "dbt project",
    format_item = function(root)
      return M.describe(root, current)
    end,
  }, function(root)
    if root then
      vim.cmd.edit(vim.fs.joinpath(root, "dbt_project.yml"))
    end
  end)
end

--- Drop a project from the store (its files are untouched).
---@param root string
---@return boolean removed
function M.forget(root)
  local data = read()
  if data.projects[root] == nil then
    return false
  end
  data.projects[root] = nil
  write(data)
  return true
end

--- Give a project a name for the pickers ("" or nil clears it).
---@param root string
---@param label string?
function M.rename(root, label)
  M.remember(root, { label = (label and label ~= "") and label or vim.NIL })
end

--- How a root reads in the pickers: its label and the path.
---@param root string
---@param current string?
---@return string
function M.describe(root, current)
  local entry = M.for_root(root)
  local parts = {}
  if entry.label then
    parts[#parts + 1] = entry.label .. " —"
  end
  parts[#parts + 1] = root
  if entry.target or entry.profile then
    parts[#parts + 1] = ("[%s%s]"):format(entry.profile or "", entry.target and (":" .. entry.target) or "")
  end
  if not vim.uv.fs_stat(vim.fs.joinpath(root, "dbt_project.yml")) then
    parts[#parts + 1] = "(missing)"
  end
  if root == current then
    parts[#parts + 1] = "(current)"
  end
  return table.concat(parts, " ")
end

--- The live cogitator client of a root, if one runs.
---@param root string
---@return vim.lsp.Client?
local function client_of(root)
  for _, client in ipairs(vim.lsp.get_clients({ name = "cogitator" })) do
    if client.root_dir == root then
      return client
    end
  end
end

--- What :DbtProjects! offers for one project.
---@param root string
---@return {label: string, run: fun()}[]
function M.project_actions(root)
  return {
    {
      label = "open",
      run = function()
        vim.cmd.edit(vim.fs.joinpath(root, "dbt_project.yml"))
      end,
    },
    {
      label = "set profiles directory",
      run = function()
        require("cogitator.picker").input(
          { prompt = "profiles.yml directory: ", completion = "dir" },
          function(dir)
            if not dir or dir == "" then
              return
            end
            local expanded = vim.fn.expand(dir)
            M.remember(root, { profiles_dir = expanded, profile = vim.NIL, target = vim.NIL })
            local client = client_of(root)
            if client then
              client:exec_cmd({ command = "dbt.setProfilesDir", arguments = { { dir = expanded } } })
            else
              vim.notify("stored; applied when " .. root .. " next attaches")
            end
          end
        )
      end,
    },
    {
      label = "clear profile/target selections",
      run = function()
        M.remember(root, { profiles_dir = vim.NIL, profile = vim.NIL, target = vim.NIL })
        vim.notify("cleared; the server's own profiles resolution applies on the next attach")
      end,
    },
    {
      label = "rename",
      run = function()
        require("cogitator.picker").input(
          { prompt = "name: ", default = M.for_root(root).label or "" },
          function(label)
            if label then
              M.rename(root, label)
            end
          end
        )
      end,
    },
    {
      label = "forget",
      run = function()
        require("cogitator.pick").select(
          { "no", "yes" },
          { prompt = "forget " .. root .. "?" },
          function(answer)
            if answer == "yes" then
              M.forget(root)
              vim.notify("forgot " .. root)
            end
          end
        )
      end,
    },
  }
end

--- :DbtProjects! — pick a project, then what to do with it.
function M.manage()
  local current = vim.fs.root(0, { "dbt_project.yml" })
  local roots = M.roots()
  if #roots == 0 then
    vim.notify("No dbt projects remembered yet.", vim.log.levels.INFO)
    return
  end
  require("cogitator.pick").select(roots, {
    prompt = "dbt project",
    format_item = function(root)
      return M.describe(root, current)
    end,
  }, function(root)
    if not root then
      return
    end
    local choices = M.project_actions(root)
    require("cogitator.pick").select(choices, {
      prompt = root,
      format_item = function(choice)
        return choice.label
      end,
    }, function(choice)
      if choice then
        choice.run()
      end
    end)
  end)
end

M._store_path = store_path -- test hook

return M
