-- The server-side highlighting's painter and the ast-grep command's parts,
-- without a server.

return function(ctx)
  local H = ctx.H
  local tshl = require("cogitator.tshl")

  -- a capture is its tree-sitter group; Neovim resolves an undefined
  -- dotted group to its parent, so a colourscheme's @keyword applies
  vim.api.nvim_set_hl(0, "@cogitator.test", { bold = true })
  local group = tshl.group("cogitator.test.deeper")
  H.check(group == "@cogitator.test.deeper", "tshl: a capture is its @ group", group)
  H.check(
    vim.api.nvim_get_hl(0, { name = group, link = false }).bold == true,
    "tshl: an undefined dotted group takes its parent's colours"
  )

  -- spans land at their UTF-16 columns, below the header
  local buf = vim.api.nvim_create_buf(false, true)
  vim.api.nvim_buf_set_lines(buf, 0, -1, false, { "header", "-- 🦀 x", "select 1" })
  local painted = tshl.apply(buf, {
    spans = {
      {
        range = { start = { line = 0, character = 0 }, ["end"] = { line = 0, character = 5 } },
        capture = "comment",
      },
      {
        range = { start = { line = 1, character = 0 }, ["end"] = { line = 1, character = 6 } },
        capture = "keyword",
      },
      {
        range = { start = { line = 9, character = 0 }, ["end"] = { line = 9, character = 1 } },
        capture = "keyword",
      },
    },
  }, 1)
  local marks = tshl.marks(buf)
  H.check(painted == 2 and #marks == 2, "tshl: spans past the text are dropped", vim.inspect(marks))
  H.check(marks[1][4].layer == "jinja", "tshl: a span without a layer is painted as jinja (v1 answers)")
  local comment = marks[1]
  -- "-- 🦀" is 5 UTF-16 units and 7 bytes: the crab is 4 bytes
  H.check(
    comment[2] == 1 and comment[3] == 0 and comment[4].end_col == 7 and comment[4].hl_group == "@comment",
    "tshl: a comment with a surrogate pair ends at its byte column",
    vim.inspect(comment)
  )
  H.check(
    marks[2][2] == 2 and marks[2][4].end_col == 6,
    "tshl: the second span on the next line",
    vim.inspect(marks[2])
  )
  tshl.clear(buf)
  H.check(#tshl.marks(buf) == 0, "tshl: clear takes the colours off")

  -- layers: each in its namespace and priority; one turned off, one skipped
  local function span(line, from, to, capture, layer)
    return {
      range = { start = { line = line, character = from }, ["end"] = { line = line, character = to } },
      capture = capture,
      layer = layer,
    }
  end
  vim.api.nvim_buf_set_lines(buf, 0, -1, false, { "name: customers", "x: '{{ var(1) }}'" })
  local layered = {
    spans = {
      span(0, 0, 4, "property", "yaml"),
      span(0, 6, 15, "type.dbt.model", "dbt"),
      span(1, 4, 6, "tag.delimiter", "jinja"),
      span(1, 7, 10, "keyword", "sql"),
    },
  }
  tshl.apply(buf, layered)
  local by_layer = {}
  for _, m in ipairs(tshl.marks(buf)) do
    by_layer[m[4].layer] = m[4]
  end
  H.check(
    by_layer.dbt
      and by_layer.dbt.hl_group == "@type.dbt.model"
      and by_layer.dbt.priority == 128
      and by_layer.yaml.priority == 120
      and by_layer.jinja.priority == 126
      and by_layer.sql.priority == 124,
    "tshl: each layer has its namespace and priority",
    vim.inspect(by_layer)
  )
  tshl.apply(buf, layered, 0, { skip = { yaml = true } })
  H.check(#vim.tbl_filter(function(m)
    return m[4].layer == "yaml"
  end, tshl.marks(buf)) == 0, "tshl: a skipped layer is not painted")
  local config = require("cogitator").config
  local saved = config and vim.deepcopy(config.highlight)
  if config then
    config.highlight.layers.dbt = false
    tshl.apply(buf, layered)
    H.check(#vim.tbl_filter(function(m)
      return m[4].layer == "dbt"
    end, tshl.marks(buf)) == 0, "tshl: a layer turned off in setup() is not painted")
    config.highlight = saved
  end
  tshl.mark_definition(buf, 1, 2)
  H.check(
    #vim.api.nvim_buf_get_extmarks(buf, tshl.def_ns, 0, -1, {}) == 2,
    "tshl: a definition's lines are marked"
  )
  tshl.mark_definition(buf, nil)
  H.check(#vim.api.nvim_buf_get_extmarks(buf, tshl.def_ns, 0, -1, {}) == 0, "tshl: and unmarked")

  -- what is painted on open, by mode
  vim.bo[buf].filetype = "yaml"
  H.check(tshl.decide(buf, false) == nil, "tshl: off paints nothing")
  local how = tshl.decide(buf, "auto")
  H.check(how and not how.skip, "tshl: auto paints a YAML buffer Neovim does not colour", vim.inspect(how))
  vim.bo[buf].filetype = "sql"
  H.check(tshl.decide(buf, "auto") ~= nil, "tshl: auto paints SQL without Neovim's tree-sitter")
  vim.bo[buf].filetype = "markdown"
  H.check(
    tshl.decide(buf, "auto") == nil and tshl.decide(buf, "always") ~= nil,
    "tshl: auto leaves other buffers, always paints them"
  )
  vim.api.nvim_buf_delete(buf, { force = true })

  for path, language in pairs({
    ["/p/models/a.sql"] = "dbt",
    ["/p/models/schema.yml"] = "yaml",
    ["/p/dbt_project.yaml"] = "yaml",
    ["/p/target/compiled/p/models/a.sql"] = "sql",
    ["C:\\p\\target\\run\\a.sql"] = "sql",
  }) do
    H.check(tshl.language_of(path) == language, "tshl: the language of " .. path, tshl.language_of(path))
  end

  local preview = require("cogitator.pick.preview")
  H.check(preview.code_of("/p/models/a.sql") == "dbt", "preview: a model is dbt")
  H.check(preview.code_of("/p/target/compiled/p/models/a.sql") == "sql", "preview: compiled SQL is SQL")
  H.check(preview.code_of("/p/models/schema.yml") == "yaml", "preview: a property file is YAML")
  H.check(preview.code_of("/p/README.md") == nil, "preview: other files are not the server's")

  -- the highlight options
  local build = require("cogitator.config").build
  H.check(
    pcall(build, { highlight = { server = "always", layers = { yaml = false } } }),
    "config: highlight.server and layers"
  )
  H.check(
    not pcall(build, { highlight = { server = "sometimes" } }),
    "config: an unknown highlight mode is an error"
  )
  H.check(
    not pcall(build, { highlight = { layers = { html = true } } }),
    "config: an unknown layer is an error"
  )

  -- the node card, without a server
  local card = require("cogitator.explorer.card")
  H.check(
    card.signature("f", { { name = "a" }, { name = "b", default = "'x'" } }) == "f(a, b='x')",
    "card: a signature with defaults"
  )
  local macro_card = card.node({
    kind = "macro",
    name = "cents",
    params = { { name = "col" } },
    package = "jaffle",
    uri = vim.uri_from_fname("/p/macros/m.sql"),
    callers = { { kind = "model", name = "orders", uri = vim.uri_from_fname("/p/models/orders.sql") } },
    calls = {},
  }, 60)
  local text = table.concat(macro_card.lines, "\n")
  H.check(
    text:find("cents(col)", 1, true)
      and text:find("called by (1)", 1, true)
      and text:find("orders  [model]", 1, true),
    "card: a macro's signature and callers",
    text
  )
  local generic = card.node({
    kind = "generic_test",
    name = "not_negative",
    params = { "model", "column_name" },
    appliedTo = { { model = "orders", column = "amount", uri = "file:///p/models/schema.yml", range = {} } },
  }, 60)
  local gtext = table.concat(generic.lines, "\n")
  local applied
  for i, l in ipairs(generic.lines) do
    if l:find("orders.amount", 1, true) then
      applied = i
    end
  end
  H.check(
    gtext:find("{% test not_negative(model, column_name) %}", 1, true) and applied and generic.jumps[applied],
    "card: a generic test's signature and where it is applied, which jump",
    gtext
  )
  local normalized = require("cogitator.explorer")._normalize({
    kind = "macro",
    callers = { { name = "a" } },
    calls = { { name = "b" } },
  })
  H.check(
    normalized.upstream[1].name == "b" and normalized.downstream[1].name == "a",
    "explorer: a macro's calls are upstream, its callers downstream"
  )
  local commands = require("cogitator.commands")
  for args, want in pairs({
    ["macro:times_five"] = { "macro", "times_five" },
    ["  orders "] = { nil, "orders" },
    ["generic_test:not_null"] = { "generic_test", "not_null" },
  }) do
    local kind, name = commands.parse_node(args)
    H.check(
      kind == want[1] and name == want[2],
      "node: parse " .. vim.inspect(args),
      vim.inspect({ kind, name })
    )
  end

  -- :DbtAstGrep's arguments
  local astgrep = require("cogitator.astgrep")
  local cases = {
    { "{{ ref($M) }}", { pattern = "{{ ref($M) }}", language = "dbt" } },
    {
      "{{ ref($M) }} => {{ ref('x', $M) }}",
      { pattern = "{{ ref($M) }}", rewrite = "{{ ref('x', $M) }}", language = "dbt" },
    },
    { "sql: select $$$C from $T", { pattern = "select $$$C from $T", language = "sql" } },
    { "dbt:{{ config($$$A) }}", { pattern = "{{ config($$$A) }}", language = "dbt" } },
    { "   ", nil },
  }
  for _, case in ipairs(cases) do
    H.check(
      vim.deep_equal(astgrep.parse(case[1]), case[2]),
      "astgrep: parse " .. vim.inspect(case[1]),
      vim.inspect(astgrep.parse(case[1]))
    )
  end
  local item = astgrep.item({
    uri = vim.uri_from_fname("/p/models/a.sql"),
    range = { start = { line = 2, character = 5 }, ["end"] = { line = 2, character = 20 } },
    text = "{{ ref('a') }}",
    replacement = "{{ ref('x', 'a') }}",
  })
  H.check(
    item.filename == "/p/models/a.sql"
      and item.lnum == 3
      and item.col == 6
      and item.text:find("→", 1, true) ~= nil,
    "astgrep: a match is a quickfix item with its rewrite",
    vim.inspect(item)
  )
end
