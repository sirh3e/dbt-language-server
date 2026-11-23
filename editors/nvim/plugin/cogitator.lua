-- User commands (spec §5.1). Defined globally; each errors cleanly when no
-- cogitator client is attached to the current buffer.

if vim.g.loaded_cogitator then
  return
end
vim.g.loaded_cogitator = true

local function cmd(name, fn, desc)
  vim.api.nvim_create_user_command(name, function()
    require("cogitator.commands")[fn]()
  end, { desc = desc })
end

cmd("DbtRun", "run", "dbt run for the current model")
cmd("DbtTest", "test", "dbt test for the current model")
cmd("DbtCompile", "compile", "dbt compile for the current model (opens compiled SQL)")
cmd("DbtParse", "parse", "dbt parse for the whole project (refreshes artifacts + context)")
cmd(
  "DbtSourceFreshness",
  "source_freshness",
  "dbt source freshness for the project (feeds the freshness lenses)"
)
vim.api.nvim_create_user_command("DbtRunSelection", function(opts)
  require("cogitator.commands").run_selection(opts)
end, { nargs = "?", desc = "dbt run with an arbitrary selector (tag:…, state:modified+)" })
vim.api.nvim_create_user_command("DbtPreview", function(opts)
  require("cogitator.commands").preview(opts)
end, { nargs = "?", desc = "dbt show for the current model (optional row limit, default 50)" })
cmd("DbtExpandJinja", "expand_jinja", "render this model's Jinja (stubbed subset) in a split")
cmd("DbtIncrementalDiff", "incremental_diff", "is_incremental() false vs true, as a diff in a split")
cmd("DbtPlan", "plan", "this model's IR levels and rule trace (dbt.queryPlan)")
cmd("DbtDoctor", "doctor", "the server's health report (dbt.doctor)")
cmd("DbtDeadModels", "dead_models", "list models nothing downstream uses (quickfix)")
vim.api.nvim_create_user_command("DbtUnresolved", function(opts)
  require("cogitator.commands").unresolved(opts)
end, {
  nargs = "*",
  complete = function()
    return { "ref", "source", "source_table", "doc", "macro", "var" }
  end,
  desc = "every unresolved ref, source, doc, macro and var (quickfix; optional kinds)",
})
vim.api.nvim_create_user_command("DbtFind", function(opts)
  require("cogitator.pick").find(opts.args)
end, {
  nargs = "?",
  complete = function()
    return require("cogitator.pick.sources").ORDER
  end,
  desc = "fuzzy-find models, definitions, columns, lineage… (fzf-lua/snacks/telescope or built-in)",
})
vim.api.nvim_create_user_command("DbtSelect", function(opts)
  require("cogitator.explorer").open({ select = opts.args ~= "" and opts.args or nil })
end, {
  nargs = "*",
  complete = function(lead)
    return require("cogitator.commands").select_completion(lead)
  end,
  desc = "dbt node selection in the navigator: tag:x +m @m a,b stg_* --exclude x (dbt.select)",
})
cmd("DbtSplit", "split", "split this table model at its incrementality frontier (dbt.splitPlan)")
cmd("DbtIncrementality", "incrementality", "table models that could be incremental (quickfix)")
vim.api.nvim_create_user_command("DbtImpact", function(opts)
  require("cogitator.commands").impact(opts)
end, { nargs = "?", desc = "downstream impact of this model (optionally of one column)" })
vim.api.nvim_create_user_command("DbtActions", function()
  require("cogitator.code_actions").open()
end, { range = true, desc = "code actions at the cursor: the server's and the plugin's (yazi-style)" })
vim.api.nvim_create_user_command("DbtCancel", function()
  require("cogitator.commands").cancel()
end, { desc = "cancel a running dbt job started from this session" })
cmd("DbtRelation", "relation", "resolve database.schema.identifier under the cursor (yanks it)")
cmd("DbtQuery", "query", "bounded SELECT on the relation under the cursor (dadbod or scratch)")
cmd("DbtSchema", "schema", "jump to the model's properties-YAML entry")
cmd("DbtLineage", "lineage", "show column lineage for the current model")
cmd("DbtExtract", "extract", "extract the model into its schema YAML (or add a version)")
cmd("DbtTests", "tests", "pick a test defined for this model/version and jump to it")
cmd("DbtStatus", "status", "show what the language server knows and has been doing")
cmd("DbtTags", "tags", "pick a dbt tag, then a model carrying it")
cmd("DbtSelectors", "selectors", "pick a selectors.yml entry and jump to it")

vim.api.nvim_create_user_command("DbtCache", function(opts)
  require("cogitator.commands").cache(opts.args)
end, {
  nargs = "?",
  complete = function()
    return { "clear" }
  end,
  desc = "derived-cache counters (argument 'clear' empties them)",
})

vim.api.nvim_create_user_command("DbtProjects", function(opts)
  local manager = require("cogitator.manager")
  if opts.bang then
    manager.manage()
  else
    manager.pick_project()
  end
end, { bang = true, desc = "pick a known dbt project; with ! manage it (profiles, rename, forget)" })

vim.api.nvim_create_user_command("DbtExplorer", function()
  require("cogitator.explorer").open()
end, { desc = "project-wide model explorer (alias of :DbtBrowse)" })

vim.api.nvim_create_user_command("DbtDag", function()
  require("cogitator.dag").open()
end, { desc = "project DAG with columns, dbt-docs style (Neovim only)" })

vim.api.nvim_create_user_command("DbtColumnLineage", function(opts)
  require("cogitator.dag").open({ columns = true, column = opts.args ~= "" and opts.args or nil })
end, { nargs = "?", desc = "column-lineage graph of this model (optionally of one column)" })

vim.api.nvim_create_user_command("DbtGraph", function()
  require("cogitator.graph").open()
end, { desc = "browse the model graph (upstream | model | downstream)" })

vim.api.nvim_create_user_command("DbtBrowse", function()
  require("cogitator.browse").open()
end, { desc = "project-wide model browser: search, lineage, SQL, compiled, versions, tests (Neovim only)" })

vim.api.nvim_create_user_command("DbtTarget", function()
  require("cogitator.profiles").pick_target()
end, { desc = "pick the active dbt profile/target (auth-aware)" })

vim.api.nvim_create_user_command("DbtProfiles", function(opts)
  local profiles = require("cogitator.profiles")
  if opts.args == "" then
    profiles.report(require("cogitator").config or require("cogitator.config").build({}))
  else
    profiles.set(opts.args)
  end
end, {
  nargs = "?",
  complete = "dir",
  desc = "show the dbt profiles resolution, or switch the profiles directory",
})

vim.api.nvim_create_user_command("DbtInstall", function()
  require("cogitator").install()
end, { desc = "install (or update) the cogitator language server, then start it" })

vim.api.nvim_create_user_command("DbtDeps", function()
  require("cogitator.dispatch").deps()
end, { desc = "dbt deps for the project (cancellable with :DbtCancel)" })

vim.api.nvim_create_user_command("DbtReloadConfig", function()
  require("cogitator.dispatch").reload_config()
end, { desc = "re-read cogitator.yml now" })

vim.api.nvim_create_user_command("Dbt", function(opts)
  require("cogitator.dispatch").run(opts.args)
end, {
  nargs = "*",
  complete = function(lead, cmdline)
    return require("cogitator.dispatch").complete(lead, cmdline)
  end,
  desc = "every :Dbt* command as :Dbt <subcommand> [args]",
})

vim.api.nvim_create_user_command("DbtHighlight", function(opts)
  require("cogitator.tshl").command(opts)
end, {
  bang = true,
  nargs = "?",
  complete = function()
    return { "auto", "always", "off" }
  end,
  desc = "colour this buffer from the server, or stop (toggle); ! auto|always|off sets the mode",
})

vim.api.nvim_create_user_command("DbtNode", function(opts)
  require("cogitator.commands").node(opts)
end, {
  nargs = "?",
  desc = "a node's card (dbt.nodeInfo): [kind:]name, e.g. macro:cents_to_dollars",
})

vim.api.nvim_create_user_command("DbtAstGrep", function(opts)
  require("cogitator.astgrep").command(opts)
end, {
  nargs = "*",
  desc = "ast-grep: [sql:]pattern [=> rewrite] — matches to quickfix, a rewrite after asking",
})

vim.api.nvim_create_user_command("DbtSyntaxTree", function()
  require("cogitator.syntax").toggle()
end, { desc = "the tree-sitter-dbt syntax tree of this buffer, in a split (toggle)" })

vim.api.nvim_create_user_command("DbtTreesitter", function(opts)
  require("cogitator.treesitter").command(opts.args)
end, {
  nargs = "?",
  complete = function()
    return { "install", "status", "start" }
  end,
  desc = "the tree-sitter-dbt grammar in Neovim: install, status (default), start",
})
