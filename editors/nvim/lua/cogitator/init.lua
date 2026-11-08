--- cogitator.nvim — Neovim integration for the cogitator dbt language
--- server (spec: specs/neovim-plugin/00-plugin.md). Cheap at require time;
--- all probing happens inside setup().

local M = {}

M.config = nil

local function capabilities()
  return vim.tbl_deep_extend("force", vim.lsp.protocol.make_client_capabilities(), {
    -- dbt.openSchema / compiled-SQL opening need showDocument (spec §4.1)
    window = { showDocument = { support = true } },
    -- model rename sends a RenameFile documentChange (spec §6.1) and
    -- dbt.extractModelYaml a CreateFile via workspace/applyEdit
    workspace = {
      workspaceEdit = {
        documentChanges = true,
        resourceOperations = { "create", "rename", "delete" },
      },
    },
  })
end

local function keymaps(bufnr)
  local function map(lhs, rhs, desc)
    vim.keymap.set("n", lhs, rhs, { buffer = bufnr, desc = desc })
  end
  local commands = require("cogitator.commands")
  map("<leader>ds", commands.schema, "dbt: go to schema entry")
  map("<leader>dl", commands.lineage, "dbt: show column lineage")
  map("<leader>dr", commands.run, "dbt: run this model")
  map("<leader>dt", commands.test, "dbt: test this model")
  map("<leader>dc", commands.compile, "dbt: compile this model")
  map("<leader>dL", commands.run_lens, "dbt: run code lens on this line")
  map("<leader>dg", function()
    require("cogitator.graph").open()
  end, "dbt: browse the model graph")
  map("<leader>db", function()
    require("cogitator.browse").open()
  end, "dbt: browse all models (Neovim only)")
  map("<leader>dT", function()
    require("cogitator.profiles").pick_target()
  end, "dbt: pick the active profile/target")
  map("<leader>dy", commands.extract, "dbt: extract model into schema YAML / add version")
  map("<leader>dq", commands.tests, "dbt: jump to a test of this model")
  map("<leader>dS", commands.status, "dbt: language-server status")
  map("<leader>dR", commands.relation, "dbt: yank the relation name under the cursor")
  map("<leader>dx", commands.query, "dbt: bounded SELECT on the relation under the cursor")
  map("<leader>dD", function()
    require("cogitator.dag").open()
  end, "dbt: project DAG (Neovim only)")
  map("<leader>dP", function()
    require("cogitator.manager").pick_project()
  end, "dbt: pick a known dbt project")
end

local function on_attach(client, bufnr, config)
  -- the server advertises these trigger characters itself since the
  -- dbt.relation batch; this block is a back-compat fallback for older
  -- server builds that advertise none (spec §6.3)
  local completion = client.server_capabilities.completionProvider
  if completion and not completion.triggerCharacters then
    completion.triggerCharacters = { ".", "'", '"', " ", "(" }
  end
  if not config.semantic_tokens then
    pcall(vim.lsp.semantic_tokens.stop, bufnr, client.id)
  end
  if config.inlay_hints then
    pcall(vim.lsp.inlay_hint.enable, true, { bufnr = bufnr })
  end
  if config.keymaps then
    keymaps(bufnr)
  end
  require("cogitator.lens").attach(bufnr, config)
end

--- (Re-)register the server definition; assignment form replaces any
--- previous definition outright, which :DbtProfiles relies on.
function M.register(config)
  local server = {
    cmd = config.cmd,
    filetypes = config.filetypes,
    capabilities = capabilities(),
  }
  if config.profiles_dir then
    server.init_options = { profilesDir = vim.fn.expand(config.profiles_dir) }
  end
  -- per-root selections from the manager store win over the global
  -- profiles_dir: they are merged into initializationOptions at attach
  server.before_init = function(params, _)
    local root = params.rootPath
    if params.rootUri and params.rootUri ~= vim.NIL then
      root = vim.uri_to_fname(params.rootUri)
    end
    local stored = require("cogitator.manager").for_root(root)
    local options = params.initializationOptions or {}
    if stored.profiles_dir then
      options.profilesDir = stored.profiles_dir
    end
    if stored.profile then
      options.profile = stored.profile
    end
    if stored.target then
      options.target = stored.target
    end
    if next(options) then
      params.initializationOptions = options
    end
  end
  if config.attach_outside_project then
    local markers = config.root_markers
    server.root_dir = function(bufnr, on_dir)
      local path = vim.api.nvim_buf_get_name(bufnr)
      local found = vim.fs.root(path ~= "" and path or 0, markers)
      on_dir(found or vim.fn.getcwd())
    end
  else
    server.root_markers = config.root_markers
  end
  vim.lsp.config.cogitator = server
end

function M.setup(opts)
  local config = require("cogitator.config").build(opts or {})
  M.config = config

  if vim.fn.has("nvim-0.11") == 0 then
    vim.notify(
      "cogitator.nvim requires Neovim 0.11+ (see :h cogitator-nvim-0.10 for the manual recipe)",
      vim.log.levels.ERROR
    )
    return
  end

  require("cogitator.highlight").setup()
  M.register(config)
  vim.lsp.enable("cogitator")

  vim.api.nvim_create_autocmd("LspAttach", {
    group = vim.api.nvim_create_augroup("CogitatorAttach", { clear = true }),
    callback = function(args)
      local client = vim.lsp.get_client_by_id(args.data.client_id)
      if client and client.name == "cogitator" then
        on_attach(client, args.buf, config)
        -- every attached project registers itself with the manager store
        if client.root_dir then
          require("cogitator.manager").remember(client.root_dir, {})
        end
      end
    end,
  })
end

return M
