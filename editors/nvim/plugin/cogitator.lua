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
cmd("DbtSchema", "schema", "jump to the model's properties-YAML entry")
cmd("DbtLineage", "lineage", "show column lineage for the current model")
