--- cogitator.nvim — Neovim integration for the cogitator dbt language
--- server (spec: specs/neovim-plugin/00-plugin.md). Cheap at require time;
--- all probing happens inside setup().

local M = {}

M.config = nil

local function capabilities()
  return vim.tbl_deep_extend("force", vim.lsp.protocol.make_client_capabilities(), {
    -- dbt.openSchema / compiled-SQL opening need showDocument (spec §4.1)
    window = { showDocument = { support = true } },
    -- model rename includes a RenameFile documentChange (spec §6.1)
    workspace = {
      workspaceEdit = {
        documentChanges = true,
        resourceOperations = { "rename" },
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
end

local function on_attach(client, bufnr, config)
  -- the server advertises no trigger characters; set client-side ones so
  -- completion fires in member-access / ref(' / post-keyword positions
  -- (spec §6.3 — revisit if the server grows triggerCharacters)
  local completion = client.server_capabilities.completionProvider
  if completion and not completion.triggerCharacters then
    completion.triggerCharacters = { ".", "'", '"', " ", "(" }
  end
  if not config.semantic_tokens then
    pcall(vim.lsp.semantic_tokens.stop, bufnr, client.id)
  end
  if config.keymaps then
    keymaps(bufnr)
  end
  require("cogitator.lens").attach(bufnr, config)
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

  local server = {
    cmd = config.cmd,
    filetypes = config.filetypes,
    capabilities = capabilities(),
  }
  if config.profiles_dir then
    server.init_options = { profilesDir = config.profiles_dir }
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

  vim.lsp.config("cogitator", server)
  vim.lsp.enable("cogitator")

  vim.api.nvim_create_autocmd("LspAttach", {
    group = vim.api.nvim_create_augroup("CogitatorAttach", { clear = true }),
    callback = function(args)
      local client = vim.lsp.get_client_by_id(args.data.client_id)
      if client and client.name == "cogitator" then
        on_attach(client, args.buf, config)
      end
    end,
  })
end

return M
