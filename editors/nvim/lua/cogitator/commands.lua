--- workspace/executeCommand wrappers (spec §1.6, §5.1): every dbt.* command
--- takes one argument object { uri, position? }.

local actions = require("cogitator.actions")

local M = {}

local client_for = actions.client_for

--- The single argument object of spec §1.6 (exposed for the test suite).
---@param bufnr integer
---@param with_position boolean include the cursor as a 0-based position
---@return {uri: string, position?: {line: integer, character: integer}}
function M.build_argument(bufnr, with_position)
  local arg = { uri = vim.uri_from_bufnr(bufnr) }
  if with_position then
    local pos = vim.api.nvim_win_get_cursor(0)
    arg.position = { line = pos[1] - 1, character = pos[2] }
  end
  return arg
end

local function exec(command, with_position, handler)
  local bufnr = vim.api.nvim_get_current_buf()
  local client = client_for(bufnr)
  if not client then
    return
  end
  client:exec_cmd({
    command = command,
    arguments = { M.build_argument(bufnr, with_position) },
  }, { bufnr = bufnr }, handler)
end

function M.run()
  exec("dbt.run", false)
end

function M.test()
  exec("dbt.test", false)
end

function M.compile()
  exec("dbt.compile", false) -- success opens the compiled SQL via showDocument
end

function M.parse()
  exec("dbt.parse", false) -- project-wide; success refreshes the server context
end

function M.source_freshness()
  exec("dbt.sourceFreshness", false) -- project-wide; sources.json feeds the lenses
end

--- :DbtPreview [limit] — `dbt show` for the current model; the result
--- table opens via showDocument when the run succeeds.
function M.preview(opts)
  local bufnr = vim.api.nvim_get_current_buf()
  local client = client_for(bufnr)
  if not client then
    return
  end
  local arg = M.build_argument(bufnr, false)
  local limit = tonumber(opts and opts.args or "")
  if limit then
    arg.limit = limit
  end
  client:exec_cmd({ command = "dbt.preview", arguments = { arg } }, { bufnr = bufnr })
end

--- :DbtImpact [column] — downstream closure of this model, with
--- column-level hits when a column is given; quickfix list + summary.
function M.impact(opts)
  local bufnr = vim.api.nvim_get_current_buf()
  local client = client_for(bufnr)
  if not client then
    return
  end
  local column = opts and opts.args ~= "" and opts.args or nil
  client:exec_cmd({
    command = "dbt.impact",
    arguments = { { uri = vim.uri_from_bufnr(bufnr), column = column } },
  }, { bufnr = bufnr }, function(err, result)
    if err or type(result) ~= "table" then
      vim.notify("dbt.impact failed" .. (err and (": " .. err.message) or ""), vim.log.levels.ERROR)
      return
    end
    vim.notify(result.summary, vim.log.levels.INFO)
    local items = {}
    for _, node in ipairs(result.models) do
      items[#items + 1] = {
        filename = node.uri and node.uri ~= vim.NIL and vim.uri_to_fname(node.uri) or "",
        lnum = 1,
        col = 1,
        text = ("depth %d — %s"):format(node.depth, node.name),
      }
    end
    for _, hit in ipairs(result.columnHits or {}) do
      items[#items + 1] = { text = ("column hit: %s.%s"):format(hit.model, hit.column) }
    end
    if #items > 0 then
      vim.fn.setqflist({}, " ", { title = "dbt impact: " .. result.summary, items = items })
      vim.cmd.copen()
    end
  end)
end

--- :DbtExpandJinja — render this model's Jinja (stubbed subset) into
--- a scratch split.
function M.expand_jinja()
  local bufnr = vim.api.nvim_get_current_buf()
  local client = client_for(bufnr)
  if not client then
    return
  end
  client:exec_cmd({
    command = "dbt.expandJinja",
    arguments = { { uri = vim.uri_from_bufnr(bufnr) } },
  }, { bufnr = bufnr }, function(err, result)
    if err or type(result) ~= "table" or type(result.sql) ~= "string" then
      vim.notify("dbt.expandJinja failed" .. (err and (": " .. err.message) or ""), vim.log.levels.ERROR)
      return
    end
    vim.cmd.vsplit()
    local scratch = vim.api.nvim_create_buf(false, true)
    vim.api.nvim_buf_set_lines(scratch, 0, -1, false, vim.split(result.sql, "\n"))
    vim.bo[scratch].filetype = "sql"
    vim.bo[scratch].modifiable = false
    vim.api.nvim_win_set_buf(0, scratch)
  end)
end

--- The lines of a dbt.incrementalDiff answer (exposed for the test
--- suite): the unified diff, or a note and the SQL when the two renders
--- of `is_incremental()` are the same.
---@param result table
---@return string[]
function M.incremental_diff_lines(result)
  if result.identical then
    local lines = {
      ("-- %s: is_incremental() changes nothing in the rendered SQL"):format(tostring(result.model)),
      "",
    }
    vim.list_extend(lines, vim.split((result.fullRefresh or ""):gsub("\n$", ""), "\n"))
    return lines
  end
  return vim.split((result.diff or ""):gsub("\n$", ""), "\n")
end

--- :DbtIncrementalDiff — this model rendered with is_incremental() false
--- and true (dbt.incrementalDiff), as a unified diff in a split.
function M.incremental_diff()
  local bufnr = vim.api.nvim_get_current_buf()
  local client = client_for(bufnr)
  if not client then
    return
  end
  client:exec_cmd({
    command = "dbt.incrementalDiff",
    arguments = { { uri = vim.uri_from_bufnr(bufnr) } },
  }, { bufnr = bufnr }, function(err, result)
    if err or type(result) ~= "table" then
      vim.notify("dbt.incrementalDiff failed" .. (err and (": " .. err.message) or ""), vim.log.levels.ERROR)
      return
    end
    vim.cmd.vsplit()
    local scratch = vim.api.nvim_create_buf(false, true)
    vim.api.nvim_buf_set_lines(scratch, 0, -1, false, M.incremental_diff_lines(result))
    vim.bo[scratch].filetype = result.identical and "sql" or "diff"
    vim.bo[scratch].modifiable = false
    vim.api.nvim_win_set_buf(0, scratch)
    vim.keymap.set("n", "q", vim.cmd.close, { buffer = scratch, nowait = true })
  end)
end

--- The dbt.queryPlan payload as display lines (exposed for the test suite).
---@param plan table the server's result
---@param line_of fun(span: {start: integer}): integer byte span to 1-based line
---@return string[]
function M.plan_lines(plan, line_of)
  local lines = {
    ("dbt query plan: %s (%s, %s)"):format(tostring(plan.model), plan.status, plan.backend),
  }
  if type(plan.guidance) == "string" then
    vim.list_extend(lines, vim.split(plan.guidance, "\n"))
  end
  for _, name in ipairs({ "hir", "mir", "lir" }) do
    local level = plan.levels[name]
    table.insert(lines, "")
    if level.available then
      table.insert(lines, ("== %s (%s) =="):format(name:upper(), level.fidelity))
      vim.list_extend(lines, vim.split(level.text or "", "\n"))
    else
      table.insert(lines, ("== %s unavailable: %s"):format(name:upper(), level.unavailable or "?"))
    end
    for _, gap in ipairs(level.degradations or {}) do
      table.insert(lines, ("  degraded [%s]: %s"):format(gap.kind, gap.detail))
    end
    for _, bug in ipairs(level.violations or {}) do
      table.insert(lines, ("  VIOLATION [%s]: %s"):format(bug.invariant, bug.detail))
    end
  end
  table.insert(lines, "")
  table.insert(lines, "== Findings ==")
  for _, finding in ipairs(plan.findings or {}) do
    table.insert(
      lines,
      ("%d: %s [%s] %s"):format(line_of(finding.span), finding.id, finding.severity, finding.message)
    )
  end
  local incr = plan.incrementality
  if type(incr) == "table" and type(incr.sexp) == "string" then
    table.insert(lines, "")
    table.insert(lines, ("== Incrementality (%s) =="):format(incr.verdict))
    vim.list_extend(lines, vim.split(incr.sexp, "\n"))
    if type(incr.frontier) == "string" then
      table.insert(lines, "")
      vim.list_extend(lines, vim.split(incr.frontier, "\n"))
    end
  end
  table.insert(lines, "")
  table.insert(lines, ("== Rule trace%s =="):format(plan.trace.incomplete and " (incomplete)" or ""))
  for _, row in ipairs(plan.trace.entries or {}) do
    local detail = row.reason or (row.count and tostring(row.count)) or ""
    table.insert(lines, ("%-24s %-13s %-10s %s"):format(row.rule, row.phase, row.outcome, detail))
  end
  return lines
end

--- :DbtPlan — each IR level and the rule trace in a scratch split.
function M.plan()
  local bufnr = vim.api.nvim_get_current_buf()
  local client = client_for(bufnr)
  if not client then
    return
  end
  client:exec_cmd({
    command = "dbt.queryPlan",
    arguments = { { uri = vim.uri_from_bufnr(bufnr) } },
  }, { bufnr = bufnr }, function(err, plan)
    if err or type(plan) ~= "table" or type(plan.levels) ~= "table" then
      vim.notify("dbt.queryPlan failed" .. (err and (": " .. err.message) or ""), vim.log.levels.ERROR)
      return
    end
    local lines = M.plan_lines(plan, function(span)
      return vim.api.nvim_buf_call(bufnr, function()
        return vim.fn.byte2line(span.start + 1)
      end)
    end)
    vim.cmd.vsplit()
    local scratch = vim.api.nvim_create_buf(false, true)
    vim.api.nvim_buf_set_lines(scratch, 0, -1, false, lines)
    vim.bo[scratch].filetype = "cogitatorplan"
    vim.bo[scratch].modifiable = false
    vim.api.nvim_win_set_buf(0, scratch)
  end)
end

--- :DbtDoctor — the server's health report in a float.
function M.doctor()
  local bufnr = vim.api.nvim_get_current_buf()
  local client = client_for(bufnr)
  if not client then
    return
  end
  client:exec_cmd({
    command = "dbt.doctor",
    arguments = { { uri = vim.uri_from_bufnr(bufnr) } },
  }, { bufnr = bufnr }, function(err, report)
    if err or type(report) ~= "table" then
      vim.notify("dbt.doctor failed" .. (err and (": " .. err.message) or ""), vim.log.levels.ERROR)
      return
    end
    local lines = vim.split(vim.inspect(report), "\n")
    local ui = require("cogitator.ui")
    local width = math.min(90, vim.o.columns - 4)
    local height = math.min(#lines, vim.o.lines - 6)
    local buf, win = ui.float({
      row = math.floor((vim.o.lines - height) / 2),
      col = math.floor((vim.o.columns - width) / 2),
      width = width,
      height = height,
      title = "dbt doctor",
    })
    ui.set_lines(buf, lines)
    vim.api.nvim_set_current_win(win)
    vim.keymap.set("n", "q", vim.cmd.close, { buffer = buf, nowait = true })
  end)
end

--- `[kind:]name` → kind (or nil) and name.
---@param args string
---@return string? kind, string name
function M.parse_node(args)
  args = vim.trim(args or "")
  local kind, name = args:match("^([%w_]+):(.+)$")
  if kind then
    return kind, vim.trim(name)
  end
  return nil, args
end

--- The kinds :DbtNode tries, in order, when none is given.
M.NODE_KINDS =
  { "model", "macro", "test", "generic_test", "source", "exposure", "metric", "semantic_model", "analysis" }

--- Show a dbt.nodeInfo card in a float; <CR> on a line with a target
--- opens it, q / <Esc> close.
---@param info table
local function node_float(info)
  local ui = require("cogitator.ui")
  local width = math.min(90, math.max(50, vim.o.columns - 10))
  local card = require("cogitator.explorer.card").node(info, width - 2)
  local height = math.min(#card.lines, math.max(5, vim.o.lines - 8))
  local buf, win = ui.float({
    row = math.max(0, math.floor((vim.o.lines - height) / 2) - 1),
    col = math.max(0, math.floor((vim.o.columns - width) / 2)),
    width = width,
    height = height,
    title = (info.kind or "node") .. " · " .. (info.name or info.model or "?"),
    cursorline = true,
  })
  ui.set_lines(buf, card.lines)
  for _, line in ipairs(card.headings) do
    vim.api.nvim_buf_add_highlight(buf, -1, "Title", line - 1, 0, -1)
  end
  vim.api.nvim_set_current_win(win)
  local function close()
    if vim.api.nvim_win_is_valid(win) then
      vim.api.nvim_win_close(win, true)
    end
  end
  for _, lhs in ipairs({ "q", "<Esc>" }) do
    vim.keymap.set("n", lhs, close, { buffer = buf, nowait = true })
  end
  vim.keymap.set("n", "<CR>", function()
    local target = card.jumps[vim.api.nvim_win_get_cursor(win)[1]]
    if not target then
      target = info.uri and info.uri ~= vim.NIL and { uri = info.uri, range = info.range } or nil
    end
    if not target then
      return
    end
    close()
    if require("cogitator.actions").open_uri(target.uri) and target.range and target.range ~= vim.NIL then
      pcall(
        vim.api.nvim_win_set_cursor,
        0,
        { target.range.start.line + 1, target.range.start.character or 0 }
      )
    end
  end, { buffer = buf, nowait = true })
  return buf, win
end

--- :DbtNode [kind:]name — a node's card (dbt.nodeInfo) in a float: a
--- macro's signature, callers and calls; a test's refs; a generic test's
--- places in YAML; a model's neighbours. Without a kind, the kinds are
--- tried in order; without a name, the current buffer's node.
---@param opts {args: string}
function M.node(opts)
  local bufnr = vim.api.nvim_get_current_buf()
  local client = client_for(bufnr)
  if not client then
    return
  end
  local kind, name = M.parse_node(opts and opts.args or "")
  if name == "" then
    name = vim.fn.fnamemodify(vim.api.nvim_buf_get_name(bufnr), ":t:r")
  end
  if name == "" then
    vim.notify("usage: :DbtNode [kind:]name   e.g. :DbtNode macro:cents_to_dollars", vim.log.levels.INFO)
    return
  end
  local kinds = kind and { kind } or M.NODE_KINDS
  local function try(i)
    local k = kinds[i]
    if not k then
      vim.notify(("no node named %s (tried %s)"):format(name, table.concat(kinds, ", ")), vim.log.levels.WARN)
      return
    end
    client:exec_cmd({
      command = "dbt.nodeInfo",
      arguments = { { node = { kind = k, name = name }, quiet = #kinds > 1 } },
    }, { bufnr = bufnr }, function(err, result)
      if err then
        vim.notify("dbt.nodeInfo failed: " .. err.message, vim.log.levels.ERROR)
      elseif type(result) == "table" then
        node_float(result)
      else
        try(i + 1)
      end
    end)
  end
  try(1)
end

--- :DbtDeadModels — models nothing uses, as a quickfix list.
function M.dead_models()
  local bufnr = vim.api.nvim_get_current_buf()
  local client = client_for(bufnr)
  if not client then
    return
  end
  client:exec_cmd({
    command = "dbt.deadModels",
    arguments = { { uri = vim.uri_from_bufnr(bufnr) } },
  }, { bufnr = bufnr }, function(err, result)
    if err or type(result) ~= "table" then
      vim.notify("dbt.deadModels failed" .. (err and (": " .. err.message) or ""), vim.log.levels.ERROR)
      return
    end
    if #result.models == 0 then
      vim.notify("no dead models — everything is referenced", vim.log.levels.INFO)
      return
    end
    local items = {}
    for _, dead in ipairs(result.models) do
      items[#items + 1] = {
        filename = dead.uri and vim.uri_to_fname(dead.uri) or "",
        lnum = 1,
        col = 1,
        text = dead.name .. " — " .. table.concat(dead.reasons, "; "),
      }
    end
    vim.fn.setqflist({}, " ", { title = "dbt dead models", items = items })
    vim.cmd.copen()
  end)
end

--- The quickfix items of a dbt.unresolved answer (exposed for the test
--- suite and the picker): one per unresolved name, at its place.
---@param result table
---@return table[]
function M.unresolved_items(result)
  local items = {}
  for _, u in ipairs(result.items or {}) do
    local start = type(u.range) == "table" and u.range.start or {}
    items[#items + 1] = {
      filename = u.uri and vim.uri_to_fname(u.uri) or "",
      lnum = (start.line or 0) + 1,
      col = (start.character or 0) + 1,
      text = ("[%s] %s — %s"):format(u.kind, u.name, u.message or ""),
      type = "W",
      user_data = { kind = u.kind, name = u.name },
    }
  end
  return items
end

--- :DbtUnresolved [kind…] — every unresolved ref, source, doc, macro and
--- var of the project, as a quickfix list.
---@param opts {fargs?: string[], args?: string}?
function M.unresolved(opts)
  local bufnr = vim.api.nvim_get_current_buf()
  local client = client_for(bufnr)
  if not client then
    return
  end
  local words = (opts and opts.fargs) or vim.split((opts and opts.args) or "", "%s+", { trimempty = true })
  local kinds = #words > 0 and words or nil
  client:exec_cmd({
    command = "dbt.unresolved",
    arguments = { { uri = vim.uri_from_bufnr(bufnr), kinds = kinds } },
  }, { bufnr = bufnr }, function(err, result)
    if err or type(result) ~= "table" then
      vim.notify("dbt.unresolved failed" .. (err and (": " .. err.message) or ""), vim.log.levels.ERROR)
      return
    end
    if result.count == 0 then
      vim.notify("nothing unresolved — every ref, source, doc, macro and var resolves", vim.log.levels.INFO)
      return
    end
    vim.fn.setqflist({}, " ", {
      title = ("dbt unresolved (%d)"):format(result.count),
      items = M.unresolved_items(result),
    })
    vim.cmd.copen()
  end)
end

--- The lines of a dbt.splitPlan answer (exposed for the test suite): the
--- plan as an S-expression, then the new model's file when there is one.
---@param plan table
---@return string[]
function M.split_lines(plan)
  local lines = vim.split(plan.sexp or "", "\n")
  local part = type(plan.part) == "table" and plan.part or nil
  if part then
    table.insert(lines, "")
    table.insert(lines, (";; %s.sql — press a to split, q to close"):format(part.name))
    vim.list_extend(lines, vim.split((part.text or ""):gsub("\n$", ""), "\n"))
  end
  return lines
end

--- Applies the split code action in `bufnr`'s window.
---@param bufnr integer
function M.apply_split(bufnr)
  local win = vim.fn.bufwinid(bufnr)
  if win == -1 then
    vim.notify("the model's buffer is not shown in a window", vim.log.levels.WARN)
    return
  end
  vim.api.nvim_win_call(win, function()
    vim.lsp.buf.code_action({
      apply = true,
      context = { only = { "refactor.extract" }, diagnostics = {} },
      filter = function(action)
        return type(action.title) == "string" and action.title:find("^Split '") ~= nil
      end,
    })
  end)
end

--- :DbtSplit — this model split at its incrementality frontier
--- (dbt.splitPlan), as an S-expression and the new model's file; `a`
--- applies the split.
function M.split()
  local bufnr = vim.api.nvim_get_current_buf()
  local client = client_for(bufnr)
  if not client then
    return
  end
  client:exec_cmd({
    command = "dbt.splitPlan",
    arguments = { { uri = vim.uri_from_bufnr(bufnr) } },
  }, { bufnr = bufnr }, function(err, plan)
    if err then
      vim.notify("dbt.splitPlan failed: " .. err.message, vim.log.levels.ERROR)
      return
    end
    if type(plan) ~= "table" then
      vim.notify(
        "nothing to split: this model is not a table, or nothing in it keeps it one",
        vim.log.levels.INFO
      )
      return
    end
    vim.cmd.vsplit()
    local scratch = vim.api.nvim_create_buf(false, true)
    vim.api.nvim_buf_set_lines(scratch, 0, -1, false, M.split_lines(plan))
    vim.bo[scratch].filetype = "lisp"
    vim.bo[scratch].modifiable = false
    vim.api.nvim_win_set_buf(0, scratch)
    vim.keymap.set("n", "q", vim.cmd.close, { buffer = scratch, nowait = true })
    if type(plan.part) == "table" then
      vim.keymap.set("n", "a", function()
        vim.cmd.close()
        M.apply_split(bufnr)
      end, { buffer = scratch, nowait = true, desc = "apply the split" })
    end
  end)
end

--- The quickfix items of a dbt.incrementality report (exposed for the
--- test suite): every table model, as the server orders them.
---@param report table
---@return table[]
function M.incrementality_items(report)
  local items = {}
  for _, row in ipairs(report.models or {}) do
    local view = row.view or {}
    local why = view.summary
    if not why and type(view.reasons) == "table" and #view.reasons > 0 then
      why = table.concat(
        vim.tbl_map(function(r)
          return r.reason
        end, view.reasons),
        "; "
      )
    end
    local text = ("%s — %s%s"):format(
      row.model,
      view.verdict or "?",
      view.strategy and (" (" .. view.strategy .. ")") or ""
    )
    if type(row.lastRunSeconds) == "number" then
      text = text .. (" [last run %.1fs]"):format(row.lastRunSeconds)
    end
    if why then
      text = text .. ": " .. why
    end
    items[#items + 1] = {
      filename = row.uri and vim.uri_to_fname(row.uri) or "",
      lnum = 1,
      col = 1,
      text = text,
    }
  end
  return items
end

--- :DbtIncrementality — every table model and whether it could be
--- incremental, slowest last run first, as a quickfix list.
function M.incrementality()
  local bufnr = vim.api.nvim_get_current_buf()
  local client = client_for(bufnr)
  if not client then
    return
  end
  client:exec_cmd({
    command = "dbt.incrementality",
    arguments = { { uri = vim.uri_from_bufnr(bufnr) } },
  }, { bufnr = bufnr }, function(err, report)
    if err or type(report) ~= "table" then
      vim.notify("dbt.incrementality failed" .. (err and (": " .. err.message) or ""), vim.log.levels.ERROR)
      return
    end
    local items = M.incrementality_items(report)
    if #items == 0 then
      vim.notify("no table models in this project", vim.log.levels.INFO)
      return
    end
    vim.fn.setqflist({}, " ", {
      title = ("dbt incrementality: %d of %d could be incremental"):format(report.incremental or 0, #items),
      items = items,
    })
    vim.cmd.copen()
  end)
end

---@type table<integer, string> tokens of dbt jobs this session started
M._jobs = {}

--- :DbtRunSelection — run dbt with an arbitrary selector (tag:nightly,
--- state:modified+, …); the returned token feeds :DbtCancel.
function M.run_selection(opts)
  local bufnr = vim.api.nvim_get_current_buf()
  local client = client_for(bufnr)
  if not client then
    return
  end
  local function start(selector)
    if not selector or selector == "" then
      return
    end
    client:exec_cmd({
      command = "dbt.runSelection",
      arguments = { { uri = vim.uri_from_bufnr(bufnr), selector = selector } },
    }, { bufnr = bufnr }, function(err, result)
      if err then
        vim.notify(err.message, vim.log.levels.ERROR)
      elseif type(result) == "table" and result.token then
        M._jobs[result.token] = "dbt run: " .. selector
        vim.notify(("dbt job %d started (%s)"):format(result.token, selector), vim.log.levels.INFO)
      end
    end)
  end
  if opts and opts.args and opts.args ~= "" then
    start(opts.args)
  else
    vim.ui.input({ prompt = "dbt selector: " }, start)
  end
end

--- :DbtCancel — pick a running job this session started and cancel it.
function M.cancel()
  local bufnr = vim.api.nvim_get_current_buf()
  local client = client_for(bufnr)
  if not client then
    return
  end
  local tokens = vim.tbl_keys(M._jobs)
  if #tokens == 0 then
    vim.notify("no dbt jobs started from this session", vim.log.levels.INFO)
    return
  end
  table.sort(tokens)
  require("cogitator.pick").select(tokens, {
    prompt = "Cancel dbt job",
    format_item = function(token)
      return ("%d — %s"):format(token, M._jobs[token])
    end,
  }, function(token)
    if not token then
      return
    end
    client:exec_cmd({
      command = "dbt.cancel",
      arguments = { { uri = vim.uri_from_bufnr(bufnr), token = token } },
    }, { bufnr = bufnr }, function(err)
      if err then
        vim.notify(err.message, vim.log.levels.ERROR)
      else
        M._jobs[token] = nil
      end
    end)
  end)
end

--- :DbtCache [clear] — derived-cache counters, or clear + refresh.
function M.cache(action)
  local bufnr = vim.api.nvim_get_current_buf()
  local client = client_for(bufnr)
  if not client then
    return
  end
  local arg = M.build_argument(bufnr, false)
  arg.action = (action == "clear") and "clear" or "stats"
  client:exec_cmd({ command = "dbt.cache", arguments = { arg } }, { bufnr = bufnr }, function(err, r)
    if err or not r or r == vim.NIL then
      vim.notify("dbt.cache failed" .. (err and (": " .. err.message) or ""), vim.log.levels.ERROR)
      return
    end
    local function line(name, surface)
      return ("%s: %d entries, %d hits, %d misses"):format(
        name,
        surface.entries,
        surface.hits,
        surface.misses
      )
    end
    vim.notify(table.concat({
      "cogitator derived caches" .. (arg.action == "clear" and " (cleared)" or ""),
      line("profiles", r.profiles),
      line("lineage", r.lineage),
      line("results", r.results),
    }, "\n"))
  end)
end

--- :DbtQuery — bounded SELECT on the relation under the cursor: through
--- vim-dadbod when a connection is configured, else into a scratch sql
--- buffer ready for any client.
function M.query()
  local cfg = require("cogitator").config
  local limit = (cfg and cfg.query_limit) or 100
  exec("dbt.relation", true, function(err, result)
    if err then
      vim.notify("dbt.relation failed: " .. err.message, vim.log.levels.ERROR)
      return
    end
    if not result or result == vim.NIL then
      vim.notify("no relation here — not a dbt model, ref() or source()", vim.log.levels.WARN)
      return
    end
    local statement = ("select * from %s limit %d"):format(result.relation, limit)
    if vim.fn.exists(":DB") == 2 and (vim.b.db or vim.g.db) then
      vim.cmd.DB(statement)
      return
    end
    local buf = vim.api.nvim_create_buf(true, true)
    vim.api.nvim_buf_set_lines(buf, 0, -1, false, {
      ("-- %s %s — run with vim-dadbod (:DB) or your SQL client"):format(result.kind, result.name),
      statement,
    })
    vim.bo[buf].filetype = "sql"
    vim.api.nvim_set_current_buf(buf)
    vim.notify(("query for %s (limit %d)"):format(result.relation, limit))
  end)
end

--- dbt.relation: resolve database.schema.identifier of the source()/ref()
--- under the cursor (else the file's model), yank it for pasting into SQL.
function M.relation()
  exec("dbt.relation", true, function(err, result)
    if err then
      vim.notify("dbt.relation failed: " .. err.message, vim.log.levels.ERROR)
      return
    end
    if not result or result == vim.NIL then
      vim.notify("no relation here — not a dbt model, ref() or source()", vim.log.levels.WARN)
      return
    end
    vim.fn.setreg('"', result.relation)
    pcall(vim.fn.setreg, "+", result.relation)
    vim.notify(("%s %s → %s (yanked)"):format(result.kind, result.name, result.relation))
  end)
end

function M.lineage()
  exec("dbt.showLineage", false) -- result arrives as a window/showMessage
end

function M.schema()
  local bufnr = vim.api.nvim_get_current_buf()
  local client = client_for(bufnr)
  if not client then
    return
  end
  client:exec_cmd({
    command = "dbt.goToSchema",
    arguments = { M.build_argument(bufnr, true) },
  }, { bufnr = bufnr }, function(err, result)
    if err then
      vim.notify("dbt.goToSchema failed: " .. err.message, vim.log.levels.ERROR)
      return
    end
    if not result or result == vim.NIL or not result.uri then
      vim.notify("No schema entry found for this model.", vim.log.levels.INFO)
      return
    end
    vim.lsp.util.show_document(result, client.offset_encoding, { focus = true })
  end)
end

function M.extract()
  -- scaffold <model>.yml, or add the next version; the server applies the
  -- change via workspace/applyEdit and navigates to it
  exec("dbt.extractModelYaml", false)
end

--- Picker over every test defined for this model (and its versions).
function M.tests()
  require("cogitator.pick").find("tests")
end

--- :DbtTags — pick a tag, then a model carrying it, and open it.
function M.tags()
  require("cogitator.pick").find("tags")
end

--- :DbtSelectors — pick a selectors.yml entry and jump to it.
function M.selectors()
  require("cogitator.pick").find("selectors")
end

--- :DbtStatus — what the server knows and has been doing.
function M.status()
  local bufnr = vim.api.nvim_get_current_buf()
  local client = client_for(bufnr)
  if not client then
    return
  end
  client:exec_cmd({
    command = "dbt.status",
    arguments = { M.build_argument(bufnr, false) },
  }, { bufnr = bufnr }, function(err, s)
    if err or not s or s == vim.NIL then
      vim.notify("dbt.status failed" .. (err and (": " .. err.message) or ""), vim.log.levels.ERROR)
      return
    end
    local target = s.profiles.activeTarget
    local lines = {
      ("cogitator %s — backend %s, fusion %s"):format(s.server.version, s.server.backend, s.server.fusion),
      ("project %s (%s): %d models, %d sources, %d graph edges"):format(
        s.project.name ~= vim.NIL and s.project.name or "?",
        s.project.dialect,
        s.project.models,
        s.project.sources,
        s.project.graphEdges
      ),
      ("profiles: %s / %s"):format(
        s.profiles.activeProfile ~= "" and s.profiles.activeProfile or "?",
        (target ~= vim.NIL and target ~= "") and target or "(default target)"
      ),
      ("refresh #%d in %d ms (cache %d hits / %d misses)"):format(
        s.refresh.count,
        s.refresh.lastMs,
        s.refresh.cacheHits,
        s.refresh.cacheMisses
      ),
    }
    if s.project.root ~= vim.NIL then
      table.insert(lines, "root: " .. s.project.root)
    end
    if s.config then
      if s.config.error ~= nil and s.config.error ~= vim.NIL then
        table.insert(lines, "config: BROKEN — " .. tostring(s.config.error):gsub("\n.*", ""))
      elseif s.config.loaded then
        table.insert(lines, "config: " .. tostring(s.config.path))
      else
        table.insert(lines, "config: none (cogitator.yml at the project root is hot-loaded)")
      end
    end
    vim.notify(table.concat(lines, "\n"), vim.log.levels.INFO)
  end)
end

--- Lens-aware wrapper: macro call-site lenses carry command == "" and are
--- informational only (spec §5.2).
function M.run_lens()
  local bufnr = vim.api.nvim_get_current_buf()
  local row = vim.api.nvim_win_get_cursor(0)[1] - 1
  for _, lens in ipairs(vim.lsp.codelens.get(bufnr)) do
    if lens.range.start.line == row and lens.command and lens.command.command == "" then
      vim.notify(lens.command.title .. " (informational lens)", vim.log.levels.INFO)
      return
    end
  end
  vim.lsp.codelens.run()
end

return M
