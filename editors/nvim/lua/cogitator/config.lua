--- Defaults, validation and derived values (spec §3). Fails loudly at
--- setup(); nothing here touches the filesystem.

local M = {}

M.defaults = {
  cmd = { "cogitator" },
  profiles_dir = nil, -- string → initializationOptions.profilesDir
  manager_path = nil, -- string → where the project/profile store lives
  query_limit = 100, -- :DbtQuery row cap
  compiled_sql = "ask", -- after dbt compile: "ask" | "open" | "never"
  fusion = false, -- false | true | "/path/to/dbt"
  tree_sitter = true, -- false → --no-tree-sitter
  debug = false, -- true → --debug
  filetypes = { "sql", "yaml" },
  root_markers = { "dbt_project.yml" },
  attach_outside_project = false,
  lens = { enabled = true, events = { "BufEnter", "CursorHold", "InsertLeave" } },
  keymaps = true,
  semantic_tokens = true,
  inlay_hints = true, -- vim.lsp.inlay_hint on attach (column lineage, ref relations, vars)
  -- column lineage graphs: hops each way, dbt.columnLineage calls at most
  lineage = { depth = 3, max_requests = 60 },
  -- :DbtDag: hops shown around the selection when focused (f, +/-),
  -- follow the current buffer's model, redraw after a save
  dag = { focus_depth = 2, follow = false, refresh_on_save = true },
  -- the tree-sitter-dbt grammar in Neovim (:DbtTreesitter install): start
  -- it on sql buffers in a project; url/rev override the pinned grammar
  grammar = { highlight = true, auto_install = false, url = nil, rev = nil },
  -- the fuzzy pickers (:DbtFind): "auto" takes fzf-lua, snacks.picker or
  -- telescope when installed, else the built-in one
  picker = "auto",
  -- the yazi-style navigator (:DbtBrowse, :DbtGraph): list sources,
  -- exposures… beside the models
  navigator = { show_all_kinds = false },
  -- colours from the server (dbt.highlight): "auto" paints SQL buffers
  -- Neovim's tree-sitter-dbt does not, and every YAML file of the
  -- project; "always" every buffer; false only :DbtHighlight. A layer set
  -- to false is never painted.
  highlight = { server = "auto", layers = { yaml = true, sql = true, jinja = true, dbt = true } },
  -- the dialogs (choices, prompts, :Dbt) and the code-action menu: the
  -- yazi-style picker walked with h/j/k/l; false → the `picker` backend
  -- for choices, vim.ui.input for prompts, the stock code-action menu
  ui = { picker = true },
  -- :DbtPreviewCte: rows in a "split" or a "float" at the cursor (the
  -- bang and <leader>dC always float)
  preview = { cte = "split" },
}

-- the nested tables whose keys are checked like the top level's
local NESTED = { "lens", "lineage", "dag", "grammar", "navigator", "highlight", "ui", "preview" }
-- nested keys whose default is nil (invisible to the defaults lookup)
local NESTED_NIL = { grammar = { url = true, rev = true } }

local function check_known_keys(opts)
  -- keys whose default is nil and so invisible to the defaults lookup
  local nil_defaults = { profiles_dir = true, manager_path = true }
  for key in pairs(opts) do
    if M.defaults[key] == nil and not nil_defaults[key] then
      error(("cogitator.nvim: unknown setup key %q"):format(key), 0)
    end
  end
  for _, section in ipairs(NESTED) do
    if type(opts[section]) == "table" then
      for key in pairs(opts[section]) do
        if M.defaults[section][key] == nil and not (NESTED_NIL[section] or {})[key] then
          error(("cogitator.nvim: unknown setup key %s.%q"):format(section, key), 0)
        end
      end
    end
  end
end

local function validate(opts)
  vim.validate("cmd", opts.cmd, "table", true)
  vim.validate("profiles_dir", opts.profiles_dir, "string", true)
  vim.validate("manager_path", opts.manager_path, "string", true)
  vim.validate("query_limit", opts.query_limit, "number", true)
  vim.validate("compiled_sql", opts.compiled_sql, function(v)
    return v == nil or v == "ask" or v == "open" or v == "never"
  end, '"ask", "open" or "never"')
  vim.validate("fusion", opts.fusion, { "boolean", "string" }, true)
  vim.validate("tree_sitter", opts.tree_sitter, "boolean", true)
  vim.validate("debug", opts.debug, "boolean", true)
  vim.validate("filetypes", opts.filetypes, "table", true)
  vim.validate("root_markers", opts.root_markers, "table", true)
  vim.validate("attach_outside_project", opts.attach_outside_project, "boolean", true)
  vim.validate("lens", opts.lens, "table", true)
  vim.validate("keymaps", opts.keymaps, "boolean", true)
  vim.validate("semantic_tokens", opts.semantic_tokens, "boolean", true)
  vim.validate("lineage", opts.lineage, "table", true)
  vim.validate("dag", opts.dag, "table", true)
  local lineage, dag = opts.lineage or {}, opts.dag or {}
  vim.validate("lineage.depth", lineage.depth, "number", true)
  vim.validate("lineage.max_requests", lineage.max_requests, "number", true)
  vim.validate("dag.focus_depth", dag.focus_depth, "number", true)
  vim.validate("dag.follow", dag.follow, "boolean", true)
  vim.validate("dag.refresh_on_save", dag.refresh_on_save, "boolean", true)
  vim.validate("picker", opts.picker, function(v)
    return v == nil or v == "auto" or vim.tbl_contains(require("cogitator.pick.backends").NAMES, v)
  end, "auto, builtin, fzf-lua, snacks or telescope")
  vim.validate("navigator", opts.navigator, "table", true)
  vim.validate("navigator.show_all_kinds", (opts.navigator or {}).show_all_kinds, "boolean", true)
  vim.validate("grammar", opts.grammar, "table", true)
  local grammar = opts.grammar or {}
  vim.validate("grammar.highlight", grammar.highlight, "boolean", true)
  vim.validate("grammar.auto_install", grammar.auto_install, "boolean", true)
  vim.validate("grammar.url", grammar.url, "string", true)
  vim.validate("grammar.rev", grammar.rev, "string", true)
  vim.validate("preview", opts.preview, "table", true)
  vim.validate("preview.cte", (opts.preview or {}).cte, function(v)
    return v == nil or v == "split" or v == "float"
  end, '"split" or "float"')
  vim.validate("ui", opts.ui, "table", true)
  vim.validate("ui.picker", (opts.ui or {}).picker, "boolean", true)
  vim.validate("highlight", opts.highlight, "table", true)
  local highlight = opts.highlight or {}
  vim.validate("highlight.server", highlight.server, function(v)
    return v == nil or v == "auto" or v == "always" or v == false
  end, '"auto", "always" or false')
  vim.validate("highlight.layers", highlight.layers, "table", true)
  for layer, on in pairs(highlight.layers or {}) do
    if not vim.tbl_contains({ "yaml", "sql", "jinja", "dbt" }, layer) then
      error(("cogitator.nvim: unknown setup key highlight.layers.%q"):format(layer), 0)
    end
    vim.validate("highlight.layers." .. layer, on, "boolean")
  end
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
