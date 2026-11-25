-- The <leader>d… keys' parts without a server: the pickers' colours,
-- ast-grep's lines and examples, the card's colours and files, the forms
-- dR yanks, and which showDocument a compile asks about.

return function(ctx)
  local H = ctx.H
  local paint = require("cogitator.pick.paint")

  -- a line of parts: its text and the spans that colour it
  local text, hls = paint.line({ { "orders", "DbtDagModelFg" }, { "  " }, { "[model]", "DbtDagModelFg" } })
  H.check(
    text == "orders  [model]"
      and vim.deep_equal(hls, { { 0, 6, "DbtDagModelFg" }, { 8, 15, "DbtDagModelFg" } }),
    "paint: a line's text and its spans come from one list of parts",
    vim.inspect({ text, hls })
  )
  H.check(
    vim.deep_equal(paint.segments(text, hls), {
      { "orders", "DbtDagModelFg" },
      { "  ", nil },
      { "[model]", "DbtDagModelFg" },
    }),
    "paint: spans cut back into segments for snacks and fzf-lua",
    vim.inspect(paint.segments(text, hls))
  )
  for _, case in ipairs({
    { "model", "file:///p/a.sql", "DbtDagModelFg" },
    { "model", "file:///p/a.py", "DbtDagPythonFg" },
    { "source", nil, "DbtDagSourceFg" },
    { "exposure", nil, "DbtDagExposureFg" },
    { "test", nil, "DbtKindTestFg" },
    { "macro", nil, "DbtKindMacroFg" },
    { "selector", nil, "DbtKindSelectorFg" },
    { "seed", nil, "DbtKindSeedFg" },
  }) do
    H.check(
      paint.kind_group(case[1], case[2]) == case[3],
      "paint: a " .. case[1] .. (case[2] and (" " .. case[2]) or "") .. " is " .. case[3],
      paint.kind_group(case[1], case[2])
    )
  end
  require("cogitator.highlight").setup()
  for _, group in ipairs({ "DbtKindTestFg", "DbtKindMacroFg", "DbtPickTag", "DbtCardRelation" }) do
    H.check(
      next(vim.api.nvim_get_hl(0, { name = group })) ~= nil,
      "paint: the group " .. group .. " is defined"
    )
  end

  -- a node's item: the text the suite has always matched, now coloured
  local item = require("cogitator.pick.sources.common").node_item({
    name = "a",
    kind = "model",
    tags = { "x" },
    uri = "file:///p/a.sql",
  }, nil, { { "↑1 ", "DbtPickUpstream" } })
  local spans = {}
  for _, span in ipairs(item.hls) do
    spans[#spans + 1] = item.text:sub(span[1] + 1, span[2]) .. "=" .. span[3]
  end
  H.check(
    item.text == "↑1 a  [model]  #x"
      and vim.deep_equal(
        spans,
        { "↑1 =DbtPickUpstream", "a=DbtDagModelFg", "[model]=DbtDagModelFg", "#x=DbtPickTag" }
      ),
    "paint: an upstream node is its arrow, its name and kind in its colour, its tags",
    vim.inspect({ item.text, spans })
  )

  -- ast-grep: a match as a coloured line, with its rewrite
  local astgrep = require("cogitator.astgrep")
  local line = astgrep.picker_item({
    uri = "file:///p/models/a.sql",
    range = { start = { line = 4, character = 2 }, ["end"] = { line = 4, character = 20 } },
    text = "{{ ref('b') }}",
    replacement = "{{ ref('p', 'b') }}",
  })
  local by_group = {}
  for _, span in ipairs(line.hls) do
    by_group[span[3]] = line.text:sub(span[1] + 1, span[2])
  end
  H.check(
    by_group.DbtPickLine == "5"
      and by_group.DbtPickMatchText == "{{ ref('b') }}"
      and by_group.DbtPickReplacement == "{{ ref('p', 'b') }}"
      and by_group.DbtPickPath ~= nil
      and line.lnum == 5,
    "astgrep: a picker line colours its file, line, match and rewrite",
    vim.inspect({ line.text, by_group })
  )
  local parsed = vim.tbl_map(function(e)
    return astgrep.parse(e.pattern) ~= nil
  end, astgrep.EXAMPLES)
  H.check(
    #astgrep.EXAMPLES >= 10 and not vim.tbl_contains(parsed, false),
    "astgrep: every example parses as [sql:]pattern [=> rewrite]",
    vim.inspect(parsed)
  )

  -- the explorer's card: kind, tags and relation coloured; the two files
  local card = require("cogitator.explorer.card")
  local s = {
    selected = "customers",
    node_of = {
      customers = { name = "customers", kind = "model", uri = "file:///p/models/customers.sql", tags = {} },
    },
    marked = {},
    info = { model = "customers", uri = "file:///p/models/customers.sql", tags = { "mart" }, tests = {} },
    relation = { relation = "db.main.customers", origin = "defaults" },
    compiled = { content = "select 1", uri = "file:///p/target/compiled/p/models/customers.sql" },
  }
  local view = card.view(s, 60)
  local marked = {}
  for _, m in ipairs(view.marks) do
    marked[m[4]] = marked[m[4]] or {}
    table.insert(marked[m[4]], view.lines[m[1]]:sub(m[2] + 1, m[3]))
  end
  H.check(
    vim.tbl_contains(marked.DbtDagModelFg or {}, "customers")
      and vim.tbl_contains(marked.DbtCardTag or {}, "#mart")
      and vim.tbl_contains(marked.DbtCardRelation or {}, "db.main.customers")
      and (marked.DbtCardWarn or {})[1] ~= nil,
    "explorer: the card colours the model by its kind, its tags and its relation",
    vim.inspect(marked)
  )
  local files = {}
  for row, jump in pairs(view.jumps) do
    files[#files + 1] = view.lines[row]:match("^%s*(%S+ ?%S*)") .. " → " .. jump.uri
  end
  table.sort(files)
  H.check(
    vim.deep_equal(files, {
      "compiled SQL → file:///p/target/compiled/p/models/customers.sql",
      "dbt SQL → file:///p/models/customers.sql",
    }),
    "explorer: the card's files lines open the dbt SQL and the compiled SQL",
    vim.inspect(files)
  )
  s.compiled = false
  local text_now = table.concat(card.view(s, 60).lines, "\n")
  H.check(
    text_now:find("not compiled yet", 1, true) ~= nil and card.compiled_uri(s) == nil,
    "explorer: before dbt compiled it, the card says so and offers nothing to open",
    text_now
  )

  -- dR: the forms of a relation
  local nav = require("cogitator.commands")
  local forms = vim.tbl_map(
    function(f)
      return f.text
    end,
    nav.relation_forms({
      kind = "model",
      name = "customers",
      database = "db",
      schema = "main",
      identifier = "customers",
      relation = "db.main.customers",
    }, 10)
  )
  H.check(
    vim.deep_equal(forms, {
      "db.main.customers",
      '"db"."main"."customers"',
      "{{ ref('customers') }}",
      "select * from {{ ref('customers') }}",
      "select * from db.main.customers limit 10",
    }),
    "relation: dR offers the relation, quoted, its ref() and two queries",
    vim.inspect(forms)
  )
  local source_forms = nav.relation_forms({
    kind = "source",
    name = "orders",
    source = "raw",
    schema = "Raw",
    identifier = 'we"ird',
    relation = 'Raw.we"ird',
  }, 10)
  H.check(
    source_forms[2].text == '"Raw"."we""ird"' and source_forms[3].text == "{{ source('raw', 'orders') }}",
    "relation: a source's forms quote each part and name it with source()",
    vim.inspect(source_forms)
  )

  -- dc: only the compiled SQL is asked about
  local sd = require("cogitator.show_document")
  local compiled = "file:///p/target/compiled/p/models/a.sql"
  for _, case in ipairs({
    { { uri = compiled }, nil, "ask" },
    { { uri = compiled }, "ask", "ask" },
    { { uri = compiled }, "open", "open" },
    { { uri = compiled }, "never", "skip" },
    { { uri = "file:///p/target/run/p/models/a.sql" }, nil, "ask" },
    { { uri = "file:///p/models/schema.yml" }, nil, "open" },
    { { uri = "file:///p/models/schema.yml" }, "never", "open" },
    { { uri = compiled, external = true }, nil, "open" },
  }) do
    H.check(
      sd.decide(case[1], case[2]) == case[3],
      ("show_document: %s with %s → %s"):format(case[1].uri, tostring(case[2]), case[3]),
      sd.decide(case[1], case[2])
    )
  end
  local labels = vim.tbl_map(function(c)
    return c.value
  end, sd.choices(compiled))
  H.check(
    vim.deep_equal(labels, { "edit", "split", "vsplit", "tabedit", "skip" }),
    "show_document: the question offers here, split, vsplit, tab and not now",
    vim.inspect(labels)
  )

  -- the dependencies dialog: a node's build on one line, and its card
  local deps = require("cogitator.deps")
  local incremental = {
    materialized = "incremental",
    incrementalStrategy = "merge",
    uniqueKey = { "id", "day" },
    partitionBy = { "day" },
    clusterBy = {},
  }
  H.check(
    deps.summary(incremental) == "incremental · merge · unique_key id, day · partition day"
      and deps.summary({ materialized = "view", uniqueKey = {}, partitionBy = {} }) == "view"
      and deps.summary(nil) == "",
    "deps: a node's build on one line: materialized, strategy, unique key, partition",
    deps.summary(incremental)
  )
  local deps_card = table.concat(
    deps.card({ name = "dim", kind = "model", depth = 2, tags = { "nightly" }, config = incremental }).lines,
    "\n"
  )
  H.check(
    deps_card:find("unique_key%s+id, day") ~= nil
      and deps_card:find("partition_by%s+day") ~= nil
      and deps_card:find("incremental_strategy%s+merge") ~= nil
      and deps_card:find("2 hops away", 1, true) ~= nil,
    "deps: the card shows each part of the config",
    deps_card
  )
  local appends = table.concat(
    deps.card({ name = "e", kind = "model", config = { materialized = "incremental" } }).lines,
    "\n"
  )
  H.check(
    appends:find("no unique_key", 1, true) ~= nil,
    "deps: an incremental model without a unique key says it appends",
    appends
  )

  -- the hover's definition link
  local hj = require("cogitator.hover_jump")
  local lines = {
    "**model** `a`",
    "",
    "Definition: [a.sql:1](file:///p/models/a.sql#L1) · `gd` jumps there",
    "see [b.sql:4](file:///p/models/b.sql#L4)",
  }
  H.check(
    vim.deep_equal(hj.link_in(lines), { uri = "file:///p/models/a.sql", line = 1 })
      and vim.deep_equal(hj.link_in(lines, 4), { uri = "file:///p/models/b.sql", line = 4 })
      and hj.link_in({ "no link [x](https://example.com)" }) == nil,
    "hover: the definition link under the cursor, else the first one",
    vim.inspect(hj.link_in(lines, 4))
  )
end
