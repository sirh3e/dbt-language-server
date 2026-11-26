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

local function keymaps(bufnr, config)
  -- once per buffer: installed when a project buffer opens (so the keys
  -- never fall through to Vim's own `d…` while the server starts) and
  -- again on attach
  if vim.b[bufnr].cogitator_keymaps then
    return
  end
  vim.b[bufnr].cogitator_keymaps = true
  local function map(lhs, rhs, desc)
    vim.keymap.set("n", lhs, rhs, { buffer = bufnr, desc = desc })
  end
  local commands = require("cogitator.commands")
  map("<leader>ds", commands.schema, "dbt: go to schema entry")
  map("<leader>dl", commands.lineage, "dbt: column lineage of this model")
  map("<leader>dp", function()
    require("cogitator.deps").open()
  end, "dbt: dependencies — what this model uses and what uses it")
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
  map(
    "<leader>dR",
    commands.relation_menu,
    "dbt: yank the relation under the cursor (ref, source, select, quoted…)"
  )
  map("<leader>dx", commands.query, "dbt: bounded SELECT on the relation under the cursor")
  map("<leader>dD", function()
    require("cogitator.dag").open()
  end, "dbt: project DAG (Neovim only)")
  map("<leader>dP", function()
    require("cogitator.manager").pick_project()
  end, "dbt: pick a known dbt project")
  local function find(source)
    return function()
      require("cogitator.pick").find(source)
    end
  end
  map("<leader>df", find("models"), "dbt: find a model")
  map("<leader>da", find("astgrep"), "dbt: ast-grep — find by a structural pattern")
  map("<leader>dF", find(nil), "dbt: find… (pick what to search)")
  map("<leader>dd", find("definitions"), "dbt: find a definition (model, source, macro, selector)")
  map("<leader>dk", find("columns"), "dbt: find a column")
  map("<leader>du", find("upstream"), "dbt: find upstream of this model")
  map("<leader>dU", find("downstream"), "dbt: find downstream of this model")
  map("<leader>dn", function()
    require("cogitator.explorer").open({ focus_buffer = true })
  end, "dbt: the yazi-style navigator, at this model")
  map("<leader>dw", function()
    require("cogitator.usages").references()
  end, "dbt: where is this used (references in a dialog)")
  map("<leader>dm", function()
    require("cogitator.usages").macros()
  end, "dbt: every macro, then where it is called")
  map("<leader>de", function()
    require("cogitator.rename").rename()
  end, "dbt: rename this model or macro (previewed)")
  map("<leader>di", function()
    require("cogitator.diagnostics").fix()
  end, "dbt: fix the problem under the cursor")
  map("<leader>dQ", commands.test_sql, "dbt: the SQL every test of this model runs")
  map("<leader>dM", commands.incremental_sql, "dbt: what an incremental run of this model executes")
  map("<leader>dI", function()
    require("cogitator.diagnostics").problems()
  end, "dbt: this file's problems in the location list")
  if config.ui.picker then
    -- Neovim's defaults, taken over like `gra`: the dialogs above
    map("grr", function()
      require("cogitator.usages").references()
    end, "dbt: references in a dialog")
    map("grn", function()
      require("cogitator.rename").rename()
    end, "dbt: rename, previewed")
  end
  -- the code-action menu: the server's actions and the plugin's own in
  -- the yazi-style picker; it takes over Neovim's default `gra` here
  local function code_actions()
    if config.ui.picker then
      require("cogitator.code_actions").open()
    else
      vim.lsp.buf.code_action()
    end
  end
  vim.keymap.set({ "n", "x" }, "<leader>dA", code_actions, { buffer = bufnr, desc = "dbt: code actions" })
  if config.ui.picker then
    vim.keymap.set({ "n", "x" }, "gra", code_actions, { buffer = bufnr, desc = "dbt: code actions" })
  end
  vim.keymap.set({ "n", "x" }, "<leader>dv", function()
    require("cogitator.select").expand()
  end, { buffer = bufnr, desc = "dbt: expand the selection (selectionRange)" })
  vim.keymap.set("x", "<leader>dV", function()
    require("cogitator.select").shrink()
  end, { buffer = bufnr, desc = "dbt: shrink the selection back" })
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
    keymaps(bufnr, config)
  end
  require("cogitator.diagnostics").attach(client, { keymaps = config.keymaps })
  require("cogitator.lens").attach(bufnr, config)
  require("cogitator.treesitter").attach(bufnr)
  -- after the grammar: the server paints what Neovim's parsers do not
  require("cogitator.tshl").attach(bufnr)
  -- what completion and :checkhealth read, fetched in the background so
  -- neither ever waits on the server
  M.prefetch_columns(bufnr)
  require("cogitator.health").prefetch(client)
end

local COLUMN_DEBOUNCE_MS = 300

--- Keep `bufnr`'s column list (for :Dbt impact / :DbtColumnLineage
--- completion) fresh: fetched now, and again a moment after the buffer is
--- entered, insert mode is left or it is saved.
---@param bufnr integer
function M.prefetch_columns(bufnr)
  local dispatch = require("cogitator.dispatch")
  local timer
  local function soon()
    if timer then
      timer:stop()
    else
      timer = vim.uv.new_timer()
    end
    timer:start(
      COLUMN_DEBOUNCE_MS,
      0,
      vim.schedule_wrap(function()
        if vim.api.nvim_buf_is_valid(bufnr) then
          dispatch.fetch_columns(bufnr)
        end
      end)
    )
  end
  local group = vim.api.nvim_create_augroup("CogitatorColumns" .. bufnr, { clear = true })
  vim.api.nvim_create_autocmd({ "BufEnter", "InsertLeave", "BufWritePost" }, {
    group = group,
    buffer = bufnr,
    callback = soon,
  })
  vim.api.nvim_create_autocmd("BufWipeout", {
    group = group,
    buffer = bufnr,
    callback = function()
      if timer then
        timer:stop()
        timer:close()
        timer = nil
      end
      dispatch._columns[bufnr] = nil
    end,
  })
  dispatch.fetch_columns(bufnr)
end

--- (Re-)register the server definition; assignment form replaces any
--- previous definition outright, which :DbtProfiles relies on.
--- :DbtInstall: install the server, then start it on the open buffers.
function M.install()
  local config = M.config or require("cogitator.config").build({})
  require("cogitator.bootstrap").install(function(ok)
    if not ok then
      return
    end
    local bootstrap = require("cogitator.bootstrap")
    -- the installers write to a fixed place, which PATH may not hold
    local bare = vim.deepcopy(config.cmd)
    bare[1] = vim.fs.basename(bare[1])
    config.cmd, M.server_found = bootstrap.resolve_cmd(bare)
    if not M.server_found then
      vim.notify(
        "cogitator: installed, but no binary found in " .. table.concat(bootstrap.install_dirs(), " or "),
        vim.log.levels.ERROR
      )
      return
    end
    M.register(config)
    vim.lsp.enable("cogitator", false)
    vim.lsp.enable("cogitator")
    vim.notify("cogitator: installed " .. config.cmd[1] .. " and started it", vim.log.levels.INFO)
  end)
end

function M.register(config)
  local server = {
    cmd = config.cmd,
    filetypes = config.filetypes,
    capabilities = capabilities(),
    -- the compiled SQL a dbt compile leaves is asked about, not opened
    handlers = { ["window/showDocument"] = require("cogitator.show_document").handler },
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

  local bootstrap = require("cogitator.bootstrap")
  config.cmd, M.server_found = bootstrap.resolve_cmd(config.cmd)

  require("cogitator.highlight").setup()
  require("cogitator.hover_jump").setup()
  require("cogitator.health").prefetch_versions(config)
  M.register(config)
  -- a server Neovim cannot spawn is reported only to :LspLog; enable it
  -- once there is a binary (setup, or :DbtInstall)
  if M.server_found then
    vim.lsp.enable("cogitator")
  end

  vim.api.nvim_create_autocmd("FileType", {
    group = vim.api.nvim_create_augroup("CogitatorBuffers", { clear = true }),
    pattern = config.filetypes,
    callback = function(args)
      local in_project = config.attach_outside_project
        or bootstrap.project_root(args.buf, config.root_markers) ~= nil
      if not in_project then
        return
      end
      if config.keymaps then
        keymaps(args.buf, config)
      end
      if not M.server_found then
        bootstrap.warn_missing_once(args.buf, config)
      end
    end,
  })

  -- when the server finishes a piece of work it reports (a project
  -- refresh, a dbt job), what :checkhealth shows is fetched again
  vim.api.nvim_create_autocmd("LspProgress", {
    group = vim.api.nvim_create_augroup("CogitatorProgress", { clear = true }),
    pattern = "end",
    callback = function(args)
      local client = vim.lsp.get_client_by_id(args.data.client_id)
      if client and client.name == "cogitator" then
        require("cogitator.health").prefetch(client)
        require("cogitator.pick.graph").invalidate(client.id)
      end
    end,
  })

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
