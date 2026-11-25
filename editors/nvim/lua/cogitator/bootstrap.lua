--- Getting from setup() to an attached server, and saying what stands in
--- the way when it is not there. Neovim reports a server it cannot start
--- only to :LspLog, so without this a missing binary looks like a plugin
--- that does nothing at all.

local M = {}

--- Where install.sh / install.ps1 put the binary when it is not on PATH:
--- $COGITATOR_BIN_DIR, else ~/.local/bin (the installers' default).
---@return string[]
function M.install_dirs()
  local dirs = {}
  if vim.env.COGITATOR_BIN_DIR and vim.env.COGITATOR_BIN_DIR ~= "" then
    dirs[#dirs + 1] = vim.fn.expand(vim.env.COGITATOR_BIN_DIR)
  end
  dirs[#dirs + 1] = vim.fs.joinpath(vim.fn.expand("~"), ".local", "bin")
  return dirs
end

--- The server command with its binary resolved: as given when it is
--- executable, else the installers' copy of a bare `cogitator`. Returns
--- the command and whether its binary can run.
---@param cmd string[]
---@param executable? fun(path: string): boolean (tests inject one)
---@return string[] cmd
---@return boolean found
function M.resolve_cmd(cmd, executable)
  executable = executable or function(path)
    return vim.fn.executable(path) == 1
  end
  if executable(cmd[1]) then
    return cmd, true
  end
  -- only the default name is looked for elsewhere: a path the user wrote
  -- is what they meant, and is reported as it is
  if cmd[1] == "cogitator" or cmd[1] == "cogitator.exe" then
    for _, dir in ipairs(M.install_dirs()) do
      local candidate = vim.fs.joinpath(dir, cmd[1])
      if executable(candidate) then
        local resolved = vim.deepcopy(cmd)
        resolved[1] = candidate
        return resolved, true
      end
    end
  end
  return cmd, false
end

--- The dbt project a buffer belongs to, if any.
---@param bufnr integer
---@param markers string[]
---@return string?
function M.project_root(bufnr, markers)
  local name = vim.api.nvim_buf_get_name(bufnr)
  if name == "" then
    return nil
  end
  return vim.fs.root(name, markers)
end

--- Why `bufnr` has no cogitator client, as one sentence with what to do.
---@param bufnr integer
---@param config table the plugin configuration
---@param found boolean whether the server binary can run
---@return string
function M.why_not_attached(bufnr, config, found)
  local ft = vim.bo[bufnr].filetype
  if not vim.tbl_contains(config.filetypes, ft) then
    return ("cogitator works in %s buffers; this one is %q"):format(
      table.concat(config.filetypes, " and "),
      ft
    )
  end
  local root = M.project_root(bufnr, config.root_markers)
  if not root and not config.attach_outside_project then
    return "this file is not in a dbt project: no "
      .. table.concat(config.root_markers, " or ")
      .. " above it (attach_outside_project = true attaches anyway)"
  end
  if not found then
    return M.missing_message(config.cmd[1])
  end
  for _, client in ipairs(vim.lsp.get_clients({ name = "cogitator" })) do
    if client.root_dir == root then
      if client:is_stopped() then
        return "the cogitator server for "
          .. vim.fn.fnamemodify(root, ":~")
          .. " has exited — see :LspLog, then :e to start it again"
      end
      return "the cogitator server for "
        .. vim.fn.fnamemodify(root, ":~")
        .. " is still starting — try again in a moment"
    end
  end
  return "the cogitator server has not started for this buffer — :e reloads it, :checkhealth cogitator says why"
end

---@param bin string
---@return string
function M.missing_message(bin)
  return ("server %q not found — :DbtInstall installs it (or set cmd in setup())"):format(bin)
end

M._warned = false

--- Warn once per session that the server cannot start, the first time a
--- buffer it would serve is opened.
---@param bufnr integer
---@param config table
function M.warn_missing_once(bufnr, config)
  if M._warned then
    return
  end
  if not (config.attach_outside_project or M.project_root(bufnr, config.root_markers)) then
    return
  end
  M._warned = true
  vim.schedule(function()
    vim.notify("cogitator: " .. M.missing_message(config.cmd[1]), vim.log.levels.WARN)
  end)
end

--- The repository root of this plugin (editors/nvim/lua/cogitator/ →
--- four levels up), where install.sh lives.
---@return string
function M.repo_root()
  local here = debug.getinfo(1, "S").source:sub(2)
  return vim.fs.dirname(vim.fs.dirname(vim.fs.dirname(vim.fs.dirname(vim.fs.dirname(here)))))
end

--- :DbtInstall — run this repository's checksum-verified installer, then
--- start the server on the buffers waiting for it.
---@param on_done fun(ok: boolean)
function M.install(on_done)
  local root = M.repo_root()
  local windows = vim.fn.has("win32") == 1
  local script = vim.fs.joinpath(root, windows and "install.ps1" or "install.sh")
  if not vim.uv.fs_stat(script) then
    vim.notify(
      "cogitator: no installer at " .. script .. " — install the server by hand (see :h cogitator)",
      vim.log.levels.ERROR
    )
    return on_done(false)
  end
  local argv = windows and { "powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", script }
    or { "sh", script }
  vim.notify("cogitator: installing the language server…", vim.log.levels.INFO)
  vim.system(argv, { cwd = root, text = true }, function(res)
    vim.schedule(function()
      if res.code ~= 0 then
        local tail = vim.trim((res.stderr ~= "" and res.stderr or res.stdout or ""):sub(-400))
        vim.notify(
          "cogitator: the installer failed (exit " .. res.code .. ")\n" .. tail,
          vim.log.levels.ERROR
        )
        return on_done(false)
      end
      on_done(true)
    end)
  end)
end

return M
