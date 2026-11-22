--- `:Dbt <subcommand> [args]` — one entry point over every :Dbt* command,
--- with completion of the subcommands and of their arguments — plus the
--- two project commands that had no command of their own (dbt.deps,
--- dbt.reloadConfig). The :Dbt* commands stay; this routes to the same
--- functions.

local actions = require("cogitator.actions")

local M = {}

local function commands()
  return require("cogitator.commands")
end

--- :DbtDeps — `dbt deps` for the project; the job can be cancelled.
function M.deps()
  local bufnr = vim.api.nvim_get_current_buf()
  actions.exec("dbt.deps", { uri = vim.uri_from_bufnr(bufnr) }, function(err, result)
    if err then
      vim.notify("dbt.deps failed: " .. err.message, vim.log.levels.ERROR)
    elseif type(result) == "table" and result.token then
      commands()._jobs[result.token] = "dbt deps"
    end
  end)
end

--- :DbtReloadConfig — read cogitator.yml again now (it also hot-reloads
--- on its own within ~2 s).
function M.reload_config()
  actions.exec("dbt.reloadConfig", { uri = vim.uri_from_bufnr(0) }, function(err)
    if err then
      vim.notify("dbt.reloadConfig failed: " .. err.message, vim.log.levels.ERROR)
    else
      vim.notify("cogitator.yml reload requested — :DbtStatus shows what was loaded")
    end
  end)
end

--- The column names dbt.columnLineage lists for the current buffer, for
--- completion (a short synchronous request; nothing when it is slow).
---@return string[]
function M.column_names()
  local bufnr = vim.api.nvim_get_current_buf()
  local client = actions.client_for(bufnr, true)
  if not client then
    return {}
  end
  local response = client:request_sync("workspace/executeCommand", {
    command = "dbt.columnLineage",
    arguments = { { uri = vim.uri_from_bufnr(bufnr) } },
  }, 800, bufnr)
  local result = response and response.result
  if type(result) ~= "table" or type(result.columns) ~= "table" then
    return {}
  end
  return vim.tbl_map(function(c)
    return c.name
  end, result.columns)
end

--- The commands.lua functions the table routes to (checked by the suite).
M._routes = {}

local function plain(name)
  M._routes[#M._routes + 1] = name
  return function()
    commands()[name]()
  end
end

local function with_args(name)
  M._routes[#M._routes + 1] = name
  return function(args)
    commands()[name]({ args = args })
  end
end

local function module_call(module, fn)
  return function(args)
    require(module)[fn](args)
  end
end

---@class cogitator.dispatch.Sub
---@field run fun(args: string)
---@field complete? fun(lead: string): string[]
---@field desc string

---@type table<string, cogitator.dispatch.Sub>
M.SUBCOMMANDS = {
  run = { run = plain("run"), desc = "dbt run this model" },
  test = { run = plain("test"), desc = "dbt test this model" },
  compile = { run = plain("compile"), desc = "dbt compile this model, open the compiled SQL" },
  parse = { run = plain("parse"), desc = "dbt parse the project" },
  deps = { run = M.deps, desc = "dbt deps for the project" },
  freshness = { run = plain("source_freshness"), desc = "dbt source freshness" },
  selection = { run = with_args("run_selection"), desc = "dbt run --select <selector>" },
  preview = { run = with_args("preview"), desc = "dbt show this model [limit]" },
  cancel = { run = plain("cancel"), desc = "cancel a dbt job" },
  expand = { run = plain("expand_jinja"), desc = "render this model's Jinja" },
  plan = { run = plain("plan"), desc = "this model's query plan" },
  doctor = { run = plain("doctor"), desc = "the server's health report" },
  dead = { run = plain("dead_models"), desc = "models nothing uses" },
  impact = {
    run = with_args("impact"),
    complete = function()
      return M.column_names()
    end,
    desc = "downstream impact [column]",
  },
  relation = { run = plain("relation"), desc = "yank the relation under the cursor" },
  query = { run = plain("query"), desc = "bounded SELECT on the relation under the cursor" },
  schema = { run = plain("schema"), desc = "jump to the properties-YAML entry" },
  tests = { run = plain("tests"), desc = "jump to a test of this model" },
  lineage = { run = plain("lineage"), desc = "column lineage as a message" },
  columns = {
    run = function(args)
      require("cogitator.dag").open({ columns = true, column = args ~= "" and args or nil })
    end,
    complete = function()
      return M.column_names()
    end,
    desc = "column-lineage graph [column]",
  },
  extract = { run = plain("extract"), desc = "extract the model into schema YAML" },
  status = { run = plain("status"), desc = "what the server knows" },
  tags = { run = plain("tags"), desc = "pick a tag, then a model" },
  selectors = { run = plain("selectors"), desc = "pick a selectors.yml entry" },
  cache = {
    run = function(args)
      commands().cache(args)
    end,
    complete = function()
      return { "clear" }
    end,
    desc = "derived-cache counters [clear]",
  },
  reload = { run = M.reload_config, desc = "reload cogitator.yml" },
  dag = { run = module_call("cogitator.dag", "open"), desc = "the project DAG" },
  graph = { run = module_call("cogitator.graph", "open"), desc = "the three-pane model graph" },
  explorer = { run = module_call("cogitator.explorer", "open"), desc = "the project explorer" },
  projects = { run = module_call("cogitator.manager", "pick_project"), desc = "the known projects" },
  target = { run = module_call("cogitator.profiles", "pick_target"), desc = "pick profile/target" },
  profiles = {
    run = function(args)
      vim.cmd("DbtProfiles" .. (args ~= "" and (" " .. vim.fn.fnameescape(args)) or ""))
    end,
    complete = function(lead)
      return vim.fn.getcompletion(lead, "dir")
    end,
    desc = "profiles resolution [dir]",
  },
}

--- Run `:Dbt <line>`.
---@param line string the command's arguments
function M.run(line)
  local name, rest = vim.trim(line):match("^(%S*)%s*(.-)$")
  if name == "" then
    local names = vim.tbl_keys(M.SUBCOMMANDS)
    table.sort(names)
    vim.ui.select(names, {
      prompt = "dbt",
      format_item = function(n)
        return ("%-10s %s"):format(n, M.SUBCOMMANDS[n].desc)
      end,
    }, function(choice)
      if choice then
        M.SUBCOMMANDS[choice].run("")
      end
    end)
    return
  end
  local sub = M.SUBCOMMANDS[name]
  if not sub then
    vim.notify(("unknown :Dbt subcommand %q — :Dbt lists them"):format(name), vim.log.levels.ERROR)
    return
  end
  sub.run(rest)
end

--- Completion for `:Dbt`: subcommands first, then the subcommand's own.
---@param lead string the word being completed
---@param cmdline string the whole command line
---@return string[]
function M.complete(lead, cmdline)
  local words = vim.split(vim.trim(cmdline:gsub("^%S+", "", 1)), "%s+", { trimempty = true })
  local finished = cmdline:match("%s$") ~= nil
  if #words == 0 or (#words == 1 and not finished) then
    local names = vim.tbl_filter(function(n)
      return vim.startswith(n, lead)
    end, vim.tbl_keys(M.SUBCOMMANDS))
    table.sort(names)
    return names
  end
  local sub = M.SUBCOMMANDS[words[1]]
  if not (sub and sub.complete) then
    return {}
  end
  return vim.tbl_filter(function(v)
    return vim.startswith(v, lead)
  end, sub.complete(lead))
end

return M
