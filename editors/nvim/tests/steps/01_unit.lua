-- Unit half: config assembly, executeCommand argument shape and the
-- manager store — no server involved (spec §3, §1.2, §1.6).

return function(ctx)
  local H = ctx.H

  local config = require("cogitator.config")
  local built = config.build({ tree_sitter = false, debug = true, fusion = "/opt/dbt" })
  H.check(
    vim.deep_equal(built.cmd, { "cogitator", "--no-tree-sitter", "--debug", "--fusion", "/opt/dbt" }),
    "config: flags assembled from options",
    vim.inspect(built.cmd)
  )
  H.check(config.build({ fusion = true }).cmd[2] == "--fusion", "config: fusion = true adds bare --fusion")
  H.check(not pcall(config.build, { typo_key = 1 }), "config: unknown key is an error")
  H.check(not pcall(config.build, { lens = { typo = 1 } }), "config: unknown lens key is an error")

  -- executeCommand argument shape ({ uri, position }, 0-based)
  local probe = vim.fs.joinpath(H.tmp, "arg-shape.sql")
  local buf = vim.api.nvim_create_buf(false, true)
  vim.api.nvim_buf_set_name(buf, probe)
  vim.api.nvim_set_current_buf(buf)
  vim.api.nvim_buf_set_lines(buf, 0, -1, false, { "select 1" })
  vim.api.nvim_win_set_cursor(0, { 1, 3 })
  local arg_obj = require("cogitator.commands").build_argument(buf, true)
  H.check(
    arg_obj.uri:find("^file://") ~= nil and arg_obj.position.line == 0 and arg_obj.position.character == 3,
    "commands: argument object is { uri, position } (0-based)",
    vim.inspect(arg_obj)
  )

  -- :DbtPlan rendering: unavailable levels name their reason, the trace shows skips
  local plan_text = table.concat(
    require("cogitator.commands").plan_lines({
      model = "m",
      status = "declined",
      backend = "DuckDb",
      guidance = "No query plan for m.\nSolution: run dbt compile.",
      levels = {
        hir = {
          available = false,
          unavailable = "HIR declined: control flow",
          degradations = {},
          violations = {},
        },
        mir = { available = true, fidelity = "Complete", text = "Scan", degradations = {}, violations = {} },
        lir = { available = false, unavailable = "not lowered", degradations = {}, violations = {} },
      },
      findings = {
        {
          id = "join.cartesian",
          span = { start = 3, ["end"] = 4 },
          severity = "warning",
          message = "no key",
        },
      },
      trace = {
        incomplete = true,
        entries = {
          { rule = "filter.non_sargable", phase = "detectLir", outcome = "skipped", reason = "unmet:phys" },
        },
      },
    }, function(span)
      return span.start + 1
    end),
    "\n"
  )
  H.check(
    plan_text:find("HIR unavailable: HIR declined", 1, true) ~= nil
      and plan_text:find("run dbt compile", 1, true) ~= nil
      and plan_text:find("4: join.cartesian [warning]", 1, true) ~= nil
      and plan_text:find("unmet:phys", 1, true) ~= nil,
    "commands: plan_lines renders levels, findings and skip reasons",
    plan_text
  )

  -- :DbtSplit rendering: the S-expression, then the part's file
  local split_text = table.concat(
    require("cogitator.commands").split_lines({
      model = "orders_ranked",
      sexp = "(split orders_ranked\n  (model orders_ranked__by_user (materialized incremental)))",
      part = {
        name = "orders_ranked__by_user",
        cte = "by_user",
        text = "select 1\n{% if is_incremental() %}\n",
      },
    }),
    "\n"
  )
  H.check(
    split_text:find("(split orders_ranked", 1, true) == 1
      and split_text:find(";; orders_ranked__by_user.sql", 1, true) ~= nil
      and split_text:find("is_incremental()", 1, true) ~= nil,
    "commands: split_lines renders the plan and the new model",
    split_text
  )
  local refused =
    require("cogitator.commands").split_lines({ model = "m", sexp = "(split m (refused no-boundary))" })
  H.check(#refused == 1, "commands: a refused split is the S-expression alone", vim.inspect(refused))

  -- :DbtIncrementalDiff rendering: the diff, or a note when nothing differs
  local diff_lines = require("cogitator.commands").incremental_diff_lines({
    model = "events",
    identical = false,
    diff = "--- events (full refresh)\n+++ events (incremental)\n@@ -1 +1,2 @@\n select 1\n+where x > 1\n",
  })
  H.check(
    diff_lines[1] == "--- events (full refresh)" and diff_lines[#diff_lines] == "+where x > 1",
    "commands: incremental_diff_lines shows the unified diff",
    vim.inspect(diff_lines)
  )
  local same = require("cogitator.commands").incremental_diff_lines({
    model = "m",
    identical = true,
    fullRefresh = "select 1\n",
    diff = "",
  })
  H.check(
    same[1]:find("changes nothing", 1, true) ~= nil and same[#same] == "select 1",
    "commands: an unchanged model says so and shows its SQL",
    vim.inspect(same)
  )

  -- :DbtDiffAt: its words in either order, and the rendered lines
  local commands = require("cogitator.commands")
  local words = commands.diff_at_args("lineage main~2")
  H.check(words.rev == "main~2" and words.what == "lineage", "commands: diff_at_args", vim.inspect(words))
  local at_lines = commands.diff_at_lines({
    model = "orders",
    rev = "HEAD (abc)",
    what = "lineage",
    identical = false,
    columns = { added = { "status" }, removed = {}, changed = {} },
    diff = "--- a\n+++ b\n+status <- stg.status\n",
  })
  H.check(
    at_lines[1] == "-- + column status" and at_lines[#at_lines] == "+status <- stg.status",
    "commands: diff_at_lines lists columns, then the diff",
    vim.inspect(at_lines)
  )
  local unchanged = commands.diff_at_lines({ model = "m", rev = "HEAD", what = "sql", identical = true })
  H.check(unchanged[1]:find("no sql difference", 1, true) ~= nil, "commands: diff_at says nothing differs")
  local cost = commands.cost_report_lines({ text = "dbt cost report\n\n  1.0 s  orders\n" })
  H.check(#cost == 3 and cost[3]:find("orders", 1, true), "commands: cost_report_lines", vim.inspect(cost))
  local prof = commands.profile_lines({ hover = "**Profile of `x`**\n\ndistinct 3" })
  H.check(prof[3] == "distinct 3", "commands: profile_lines", vim.inspect(prof))

  -- :DbtSyncYaml: the argument of its words, and the preview lines
  local sync = require("cogitator.sync_yaml")
  local sync_arg = sync.arguments({ "%", "prune", "types", "stg_orders" }, "file:///p/models/a.sql")
  H.check(
    sync_arg.uri == "file:///p/models/a.sql"
      and sync_arg.prune == true
      and sync_arg.inheritTypes == true
      and sync_arg.models[1] == "stg_orders"
      and sync_arg.apply == nil,
    "sync_yaml: words become the dbt.syncYaml argument",
    vim.inspect(sync_arg)
  )
  local sync_lines = sync.lines({
    changes = 2,
    files = { { uri = "file:///p/models/a.yml", diff = "--- a/a.yml\n+++ b/a.yml\n+      - name: id\n" } },
    models = {
      {
        model = "a",
        createdEntry = false,
        added = { "id" },
        inherited = {
          { column = "id", from = { kind = "source", source = "raw", table = "t", column = "id" } },
        },
        typed = {},
        stale = { "old" },
        pruned = {},
        skipped = { { column = "x", reason = "written in flow style" } },
      },
      { model = "quiet", added = {}, inherited = {}, typed = {}, stale = {}, pruned = {}, skipped = {} },
    },
  })
  H.check(
    sync_lines[1] == "# dbt.syncYaml: 2 change(s) in 1 file(s)"
      and sync_lines[2] == "# a"
      and sync_lines[3] == "  + column id"
      and sync_lines[4] == "  ~ id description from source raw.t.id"
      and sync_lines[5] == "  ! stale column old (prune removes it)"
      and sync_lines[6] == "  ? x: written in flow style"
      and sync_lines[8] == "--- a/a.yml"
      and sync_lines[#sync_lines] == "+      - name: id",
    "sync_yaml: the preview lists each model's changes, then the diffs",
    vim.inspect(sync_lines)
  )

  -- :DbtPreviewCte rendering: the server's markdown table, line by line
  local preview = require("cogitator.preview_cte").lines({
    table = "# customers · CTE `orders`\n\n| id |\n| --- |\n| 1 |\n",
  })
  H.check(
    preview[1] == "# customers · CTE `orders`" and preview[#preview] == "| 1 |",
    "preview_cte: lines are the server's table",
    vim.inspect(preview)
  )

  -- :DbtIncrementality quickfix items
  local items = require("cogitator.commands").incrementality_items({
    incremental = 1,
    models = {
      {
        model = "big",
        uri = "file:///p/models/big.sql",
        lastRunSeconds = 12.5,
        view = { verdict = "append", strategy = "append", summary = "append by ts" },
      },
      {
        model = "ranked",
        uri = "file:///p/models/ranked.sql",
        view = { verdict = "keep", reasons = { { reason = "a LIMIT" } } },
      },
    },
  })
  H.check(
    #items == 2
      and items[1].text == "big — append (append) [last run 12.5s]: append by ts"
      and items[2].text == "ranked — keep: a LIMIT"
      and items[2].filename == "/p/models/ranked.sql",
    "commands: incrementality_items name verdict, strategy, run time and why",
    vim.inspect(items)
  )

  -- :DbtLineageSite message
  local site = require("cogitator.lineage_site")
  local text, level = site.message({
    written = { path = "/p/target/cogitator-lineage/index.html" },
    counts = { columnsTraced = 25, columns = 39, columnsUndetermined = 14, modelsDeclined = 1 },
  })
  local none_text, none_level = site.message(nil)
  local expected = "lineage page: /p/target/cogitator-lineage/index.html — "
    .. "25 of 39 columns traced, 14 undetermined, 1 models declined"
  H.check(
    text == expected
      and level == vim.log.levels.INFO
      and none_level == vim.log.levels.WARN
      and none_text:find("no dbt project", 1, true) ~= nil,
    "lineage_site: message says where the page is and what was traced",
    text
  )

  -- :DbtUnresolved quickfix items
  local unresolved = require("cogitator.commands").unresolved_items({
    count = 1,
    items = {
      {
        kind = "ref",
        name = "ghost",
        uri = "file:///p/models/a.sql",
        range = { start = { line = 2, character = 14 }, ["end"] = { line = 2, character = 19 } },
        message = "model 'ghost' not found in the project",
      },
    },
  })
  H.check(
    #unresolved == 1
      and unresolved[1].filename == "/p/models/a.sql"
      and unresolved[1].lnum == 3
      and unresolved[1].col == 15
      and unresolved[1].text == "[ref] ghost — model 'ghost' not found in the project",
    "commands: unresolved_items place each name at its line and column",
    vim.inspect(unresolved)
  )

  -- :DbtBreakingChanges quickfix items
  local breaking = require("cogitator.commands").breaking_items({
    diff = { removed = { { name = "customer_id", possiblyRenamedTo = "customer_key" } } },
    impacts = {
      {
        column = "customer_id",
        change = "removed",
        severity = "error",
        tests = { "unique" },
        downstream = { { model = "customers", column = "customer_id", contracted = true, public = false } },
      },
    },
  })
  H.check(
    #breaking == 2
      and breaking[1].type == "E"
      and breaking[1].text == "customer_id removed (possibly renamed to customer_key) — tested: unique"
      and breaking[2].text == "  read by customers.customer_id (contracted)",
    "commands: breaking_items name the change, the rename hint and the downstream reads",
    vim.inspect(breaking)
  )

  -- :DbtImpact quickfix items, with the column reach
  local impact = require("cogitator.commands").impact_items({
    models = { { name = "customers", uri = "file:///p/models/customers.sql", depth = 1 } },
    columnHits = { { model = "customers", column = "customer_id" } },
    reach = {
      tests = { { model = "customers", column = "customer_id", test = "unique" } },
      semanticItems = {
        {
          semanticModel = "sm",
          list = "entity",
          name = "customer",
          uri = "file:///p/models/sm.yml",
          range = { start = { line = 4, character = 8 } },
        },
      },
      exposures = { { name = "weekly", level = "node" } },
      dashboards = { { name = "Board", level = "column", url = "https://x" } },
    },
  })
  H.check(
    #impact == 6
      and impact[1].filename == "/p/models/customers.sql"
      and impact[4].text == "semantic model sm: entity customer"
      and impact[4].lnum == 5
      and impact[5].text == "exposure weekly (node level)"
      and impact[6].text == "dashboard Board (column level) https://x",
    "commands: impact_items list the models, hits and the column reach",
    vim.inspect(impact)
  )

  -- :DbtSearch summary
  local search = require("cogitator.search")
  local answer = {
    matches = { {}, {} },
    models = { "customers" },
    searched = 4,
    notSearched = { { model = "orders", why = "Jinja control blocks" } },
  }
  H.check(
    search.summary(answer) == "2 matches in 1 model (4 searched); 1 not searched"
      and search.not_searched_lines(answer)[1] == "orders: Jinja control blocks",
    "search: the summary and the models not searched",
    search.summary(answer)
  )

  -- manager store: merge, vim.NIL clearing, roots listing (pure file I/O)
  local proj = vim.fs.joinpath(H.tmp, "proj-a")
  local manager = require("cogitator.manager")
  manager.remember(proj, { profiles_dir = "/etc/dbt", target = "prod" })
  manager.remember(proj, { target = vim.NIL })
  local entry = manager.for_root(proj)
  H.check(
    entry.profiles_dir == "/etc/dbt" and entry.target == nil and entry.last_used ~= nil,
    "manager: store merge + vim.NIL clear roundtrip",
    vim.inspect(entry)
  )
  H.check(vim.tbl_contains(manager.roots(), proj), "manager: roots lists the project")

  dofile(vim.fs.joinpath(ctx.repo_root, "editors/nvim/tests/steps/unit_ui.lua"))(ctx)
  dofile(vim.fs.joinpath(ctx.repo_root, "editors/nvim/tests/steps/unit_graphs.lua"))(ctx)
  dofile(vim.fs.joinpath(ctx.repo_root, "editors/nvim/tests/steps/unit_column_dialog.lua"))(ctx)
  dofile(vim.fs.joinpath(ctx.repo_root, "editors/nvim/tests/steps/unit_dispatch.lua"))(ctx)
  dofile(vim.fs.joinpath(ctx.repo_root, "editors/nvim/tests/steps/unit_tools.lua"))(ctx)
  dofile(vim.fs.joinpath(ctx.repo_root, "editors/nvim/tests/steps/unit_async.lua"))(ctx)
  dofile(vim.fs.joinpath(ctx.repo_root, "editors/nvim/tests/steps/unit_diagnostics.lua"))(ctx)
  dofile(vim.fs.joinpath(ctx.repo_root, "editors/nvim/tests/steps/unit_cte.lua"))(ctx)
  dofile(vim.fs.joinpath(ctx.repo_root, "editors/nvim/tests/steps/unit_pick.lua"))(ctx)
  dofile(vim.fs.joinpath(ctx.repo_root, "editors/nvim/tests/steps/unit_selection.lua"))(ctx)
  dofile(vim.fs.joinpath(ctx.repo_root, "editors/nvim/tests/steps/unit_explorer.lua"))(ctx)
  dofile(vim.fs.joinpath(ctx.repo_root, "editors/nvim/tests/steps/unit_tshl.lua"))(ctx)
  dofile(vim.fs.joinpath(ctx.repo_root, "editors/nvim/tests/steps/unit_bootstrap.lua"))(ctx)
  dofile(vim.fs.joinpath(ctx.repo_root, "editors/nvim/tests/steps/unit_shortcuts.lua"))(ctx)
end
