--- Defaults, validation and derived values (spec §3). Fails loudly at
--- setup(); nothing here touches the filesystem.

local M = {}

M.defaults = {
  cmd = { "cogitator" },
  profiles_dir = nil, -- string → initializationOptions.profilesDir
  manager_path = nil, -- string → where the project/profile store lives
  query_limit = 100, -- :DbtQuery row cap
  fusion = false, -- false | true | "/path/to/dbt"
  tree_sitter = true, -- false → --no-tree-sitter
  debug = false, -- true → --debug
  filetypes = { "sql", "yaml" },
  root_markers = { "dbt_project.yml" },
  attach_outside_project = false,
  lens = { enabled = true, events = { "BufEnter", "CursorHold", "InsertLeave" } },
  keymaps = true,
  semantic_tokens = true,
}

local function check_known_keys(opts)
  -- keys whose default is nil and so invisible to the defaults lookup
  local nil_defaults = { profiles_dir = true, manager_path = true }
  for key in pairs(opts) do
    if M.defaults[key] == nil and not nil_defaults[key] then
      error(("cogitator.nvim: unknown setup key %q"):format(key), 0)
    end
  end
  for key in pairs(opts.lens or {}) do
    if M.defaults.lens[key] == nil then
      error(("cogitator.nvim: unknown setup key lens.%q"):format(key), 0)
    end
  end
end

local function validate(opts)
  vim.validate("cmd", opts.cmd, "table", true)
  vim.validate("profiles_dir", opts.profiles_dir, "string", true)
  vim.validate("manager_path", opts.manager_path, "string", true)
  vim.validate("query_limit", opts.query_limit, "number", true)
  vim.validate("fusion", opts.fusion, { "boolean", "string" }, true)
  vim.validate("tree_sitter", opts.tree_sitter, "boolean", true)
  vim.validate("debug", opts.debug, "boolean", true)
  vim.validate("filetypes", opts.filetypes, "table", true)
  vim.validate("root_markers", opts.root_markers, "table", true)
  vim.validate("attach_outside_project", opts.attach_outside_project, "boolean", true)
  vim.validate("lens", opts.lens, "table", true)
  vim.validate("keymaps", opts.keymaps, "boolean", true)
  vim.validate("semantic_tokens", opts.semantic_tokens, "boolean", true)
end

--- The server command with the flags derived from the options (spec §1.2).
---@param config table the merged options
---@return string[] cmd argv for vim.lsp.config
function M.cmd_with_flags(config)
  local cmd = vim.deepcopy(config.cmd)
  if not config.tree_sitter then
    table.insert(cmd, "--no-tree-sitter")
  end
  if config.debug then
    table.insert(cmd, "--debug")
  end
  if config.fusion == true then
    table.insert(cmd, "--fusion")
  elseif type(config.fusion) == "string" then
    table.insert(cmd, "--fusion")
    table.insert(cmd, config.fusion)
  end
  return cmd
end

--- Merge user options over the defaults; raises on unknown keys or types.
---@param opts table user setup() options
---@return table config the validated, merged configuration
function M.build(opts)
  check_known_keys(opts)
  validate(opts)
  local config = vim.tbl_deep_extend("force", vim.deepcopy(M.defaults), opts)
  config.cmd = M.cmd_with_flags(config)
  return config
end

return M
