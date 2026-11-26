--- Answers shown in a scratch split or a float: rendered Jinja, the
--- incremental diff, the query plan, the doctor's report, the cache
--- counters and the server's status.

local actions = require("cogitator.actions")
local shared = require("cogitator.commands.shared")

local client_for = actions.client_for

local M = {}

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

--- The incremental strategies dbt.incrementalSql takes.
M.STRATEGIES = { "append", "merge", "delete+insert", "insert_overwrite", "microbatch" }

--- The lines of a dbt.testSql answer and, by line, the test whose
--- header it is (exposed for the test suite): the server's `text`, each
--- `-- ── <label>` header matched to the test of that label, in order.
---@param result table
---@return string[] lines
---@return table<integer, table> tests by 1-based header line
function M.test_sql_lines(result)
  local lines = vim.split((result.text or ""):gsub("\n$", ""), "\n")
  local by_line, next_test = {}, 1
  local tests = result.tests or {}
  for i, line in ipairs(lines) do
    local test = tests[next_test]
    if test and vim.startswith(line, "-- ── " .. test.label) then
      by_line[i] = test
      next_test = next_test + 1
    end
  end
  return lines, by_line
end

--- A read-only SQL split of `lines`; `q` closes it.
local function sql_split(lines, name)
  vim.cmd.vsplit()
  local scratch = vim.api.nvim_create_buf(false, true)
  vim.api.nvim_buf_set_lines(scratch, 0, -1, false, lines)
  vim.bo[scratch].filetype = "sql"
  vim.bo[scratch].modifiable = false
  pcall(vim.api.nvim_buf_set_name, scratch, name)
  vim.api.nvim_win_set_buf(0, scratch)
  vim.keymap.set("n", "q", vim.cmd.close, { buffer = scratch, nowait = true })
  return scratch
end

--- :DbtTestSql — the SQL every test of this model runs (dbt.testSql),
--- each test's query in a SQL split; <CR> on a test's header opens
--- where it is declared, ]t / [t walk the tests, q closes.
function M.test_sql()
  local bufnr = vim.api.nvim_get_current_buf()
  local client = client_for(bufnr)
  if not client then
    return
  end
  client:exec_cmd({
    command = "dbt.testSql",
    arguments = { { uri = vim.uri_from_bufnr(bufnr) } },
  }, { bufnr = bufnr }, function(err, result)
    if err or type(result) ~= "table" then
      vim.notify("dbt.testSql failed" .. (err and (": " .. err.message) or ""), vim.log.levels.ERROR)
      return
    end
    local lines, by_line = M.test_sql_lines(result)
    local scratch = sql_split(lines, "cogitator://tests/" .. tostring(result.model))
    local headers = vim.tbl_keys(by_line)
    table.sort(headers)
    vim.keymap.set("n", "<CR>", function()
      local test = by_line[vim.api.nvim_win_get_cursor(0)[1]]
      if not test then
        return
      end
      vim.cmd.wincmd("p")
      vim.cmd.edit(vim.fn.fnameescape(vim.uri_to_fname(test.uri)))
      local pos = test.range and test.range.start or { line = 0, character = 0 }
      pcall(vim.api.nvim_win_set_cursor, 0, { pos.line + 1, pos.character })
    end, { buffer = scratch, desc = "open where the test is declared" })
    local function walk(step)
      return function()
        local row = vim.api.nvim_win_get_cursor(0)[1]
        local target
        for _, h in ipairs(headers) do
          if (step > 0 and h > row and not target) or (step < 0 and h < row) then
            target = h
          end
        end
        if target then
          vim.api.nvim_win_set_cursor(0, { target, 0 })
        end
      end
    end
    vim.keymap.set("n", "]t", walk(1), { buffer = scratch, desc = "next test" })
    vim.keymap.set("n", "[t", walk(-1), { buffer = scratch, desc = "previous test" })
  end)
end

--- :DbtIncrementalSql [strategy] — what an incremental run of this model
--- executes for its strategy, or the one named (dbt.incrementalSql).
---@param opts? {args?: string}
function M.incremental_sql(opts)
  local strategy = vim.trim((opts or {}).args or "")
  local bufnr = vim.api.nvim_get_current_buf()
  local client = client_for(bufnr)
  if not client then
    return
  end
  local argument = { uri = vim.uri_from_bufnr(bufnr) }
  if strategy ~= "" then
    argument.strategy = strategy
  end
  client:exec_cmd(
    { command = "dbt.incrementalSql", arguments = { argument } },
    { bufnr = bufnr },
    function(err, result)
      if err or type(result) ~= "table" then
        vim.notify("dbt.incrementalSql failed" .. (err and (": " .. err.message) or ""), vim.log.levels.ERROR)
        return
      end
      local lines = vim.split((result.text or ""):gsub("\n$", ""), "\n")
      sql_split(
        lines,
        ("cogitator://incremental/%s.%s"):format(tostring(result.model), tostring(result.strategy))
      )
    end
  )
end

--- :Dbt incremental — the strategy templates of this model (make it
--- incremental, or switch its strategy) in the code-action menu.
function M.incremental_templates()
  require("cogitator.code_actions").open({ only = { "refactor.rewrite" } })
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

--- :DbtCache [clear] — derived-cache counters, or clear + refresh.
function M.cache(action)
  local bufnr = vim.api.nvim_get_current_buf()
  local client = client_for(bufnr)
  if not client then
    return
  end
  local arg = shared.build_argument(bufnr, false)
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

--- :DbtStatus — what the server knows and has been doing.
function M.status()
  local bufnr = vim.api.nvim_get_current_buf()
  local client = client_for(bufnr)
  if not client then
    return
  end
  client:exec_cmd({
    command = "dbt.status",
    arguments = { shared.build_argument(bufnr, false) },
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

return M
