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

--- bufnr → { tick = changedtick, names = string[] }: the columns
--- dbt.columnLineage last listed for a buffer, which completion reads.
---@type table<integer, {tick: integer, names: string[]}>
M._columns = {}
--- bufnr → the cancel function of a column fetch in flight
---@type table<integer, fun()>
M._fetching = {}

--- Ask the server, in the background, for the columns of `bufnr`; the
--- answer lands in `M._columns`. A fetch in flight for an older text is
--- cancelled; nothing waits.
---@param bufnr integer
function M.fetch_columns(bufnr)
  local client = actions.client_for(bufnr, true)
  if not client then
    return
  end
  local tick = vim.api.nvim_buf_get_changedtick(bufnr)
  local cached = M._columns[bufnr]
  if cached and cached.tick == tick then
    return
  end
  if M._fetching[bufnr] then
    M._fetching[bufnr]()
  end
  M._fetching[bufnr] = actions.request(client, bufnr, "workspace/executeCommand", {
    command = "dbt.columnLineage",
    arguments = { { uri = vim.uri_from_bufnr(bufnr) } },
  }, {
    timeout_ms = 10000,
    stale = function()
      return not vim.api.nvim_buf_is_valid(bufnr)
    end,
  }, function(err, result)
    M._fetching[bufnr] = nil
    if err or type(result) ~= "table" or type(result.columns) ~= "table" then
      return
    end
    M._columns[bufnr] = {
      tick = tick,
      names = vim.tbl_map(function(c)
        return c.name
      end, result.columns),
    }
  end)
end

--- The column names of the current buffer, for completion. Completion
--- has to answer at once, so this is what was fetched last (none before
--- the first answer); a missing or older list starts a fetch, so the
--- next <Tab> has it.
---@return string[]
function M.column_names()
  local bufnr = vim.api.nvim_get_current_buf()
  local cached = M._columns[bufnr]
  if not cached or cached.tick ~= vim.api.nvim_buf_get_changedtick(bufnr) then
    M.fetch_columns(bufnr)
  end
  return cached and cached.names or {}
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
  actions = { run = module_call("cogitator.code_actions", "open"), desc = "code actions at the cursor" },
  expand = { run = plain("expand_jinja"), desc = "render this model's Jinja" },
  plan = { run = plain("plan"), desc = "this model's query plan" },
  doctor = { run = plain("doctor"), desc = "the server's health report" },
  dead = { run = plain("dead_models"), desc = "models nothing uses" },
  unresolved = { run = with_args("unresolved"), desc = "every unresolved ref, source, doc, macro, var" },
  split = { run = plain("split"), desc = "split this table model at its incrementality frontier" },
  incrementality = { run = plain("incrementality"), desc = "table models that could be incremental" },
  impact = {
    run = with_args("impact"),
    complete = function()
      return M.column_names()
    end,
    desc = "downstream impact [column]",
  },
  breaking = { run = plain("breaking_changes"), desc = "breaking changes against the last build" },
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
  syntax = { run = module_call("cogitator.syntax", "toggle"), desc = "the syntax tree of this buffer" },
  highlight = {
    run = module_call("cogitator.tshl", "toggle"),
    desc = "colour this buffer with tree-sitter-dbt (server)",
  },
  node = { run = with_args("node"), desc = "a node's card: [kind:]name (dbt.nodeInfo)" },
  astgrep = {
    run = function(args)
      require("cogitator.astgrep").command({ args = args })
    end,
    desc = "ast-grep: [sql:]pattern [=> rewrite]",
  },
  treesitter = {
    run = module_call("cogitator.treesitter", "command"),
    complete = function()
      return { "install", "status", "start" }
    end,
    desc = "the tree-sitter-dbt grammar: install, status, start",
  },
  manage = { run = module_call("cogitator.manager", "manage"), desc = "manage a known project" },
  find = {
    run = function(args)
      require("cogitator.pick").find(args)
    end,
    complete = function()
      return require("cogitator.pick.sources").ORDER
    end,
    desc = "fuzzy-find models, definitions, columns, lineage… [source]",
  },
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

--- The subcommands as groups, in menu order: the `:Dbt` menu and the
--- code-action menu (cogitator.code_actions) are both built from this.
--- `model` groups need a model's .sql buffer. Every subcommand is in
--- exactly one group (checked by the suite).
M.GROUPS = {
  {
    name = "Model",
    icon = "◇",
    model = true,
    subs = {
      "run",
      "test",
      "compile",
      "preview",
      "expand",
      "plan",
      "split",
      "impact",
      "breaking",
      "lineage",
      "schema",
      "tests",
      "extract",
      "relation",
      "query",
    },
  },
  {
    name = "Views",
    icon = "◈",
    subs = { "find", "graph", "dag", "columns", "explorer", "node", "syntax", "highlight" },
  },
  {
    name = "Project",
    icon = "▣",
    subs = {
      "parse",
      "deps",
      "freshness",
      "selection",
      "cancel",
      "dead",
      "unresolved",
      "incrementality",
      "tags",
      "selectors",
      "astgrep",
    },
  },
  {
    name = "Session",
    icon = "⚙",
    subs = { "status", "doctor", "target", "profiles", "projects", "manage", "reload", "cache", "treesitter" },
  },
}

--- Subcommands that are no menu entry themselves.
M.UNGROUPED = { actions = true }

--- Is `bufnr` a model's SQL buffer (what the Model group needs)?
---@param bufnr integer
---@return boolean
function M.is_model_buffer(bufnr)
  return vim.bo[bufnr].filetype == "sql" and vim.api.nvim_buf_get_name(bufnr) ~= ""
end

--- Picker groups of the subcommands. `skip` names subcommands to leave
--- out (the code-action menu drops those the server already offers).
---@param bufnr integer
---@param skip table<string, boolean>?
---@return cogitator.picker.Item[]
function M.menu_items(bufnr, skip)
  skip = skip or {}
  local model = M.is_model_buffer(bufnr)
  local groups = {}
  for _, group in ipairs(M.GROUPS) do
    local children = {}
    for _, name in ipairs(group.subs) do
      local sub = M.SUBCOMMANDS[name]
      if sub and not skip[name] then
        children[#children + 1] = {
          label = sub.desc,
          detail = ":Dbt " .. name,
          icon = "›",
          icon_hl = "DbtPickerCommand",
          disabled = (group.model and not model) and "needs a model's .sql buffer" or nil,
          value = { subcommand = name },
          preview = function()
            return {
              "  " .. sub.desc,
              "",
              "  :Dbt " .. name .. (sub.complete and " [argument]" or ""),
              "",
              "  run by the plugin; its answer opens in its own view",
            }
          end,
        }
      end
    end
    if #children > 0 then
      groups[#groups + 1] = {
        label = group.name,
        icon = group.icon,
        detail = tostring(#children),
        children = children,
      }
    end
  end
  return groups
end

--- Run `:Dbt <line>`.
---@param line string the command's arguments
function M.run(line)
  local name, rest = vim.trim(line):match("^(%S*)%s*(.-)$")
  if name == "" then
    local config = package.loaded["cogitator"] and require("cogitator").config
    if config and config.ui and config.ui.picker == false then
      local names = vim.tbl_keys(M.SUBCOMMANDS)
      table.sort(names)
      require("cogitator.pick").select(names, {
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
    require("cogitator.picker").open({
      title = "dbt",
      items = M.menu_items(vim.api.nvim_get_current_buf()),
      on_choose = function(item)
        M.SUBCOMMANDS[item.value.subcommand].run("")
      end,
    })
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
