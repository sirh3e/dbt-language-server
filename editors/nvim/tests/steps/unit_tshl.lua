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
  local marks = vim.api.nvim_buf_get_extmarks(buf, tshl.ns, 0, -1, { details = true })
  H.check(painted == 2 and #marks == 2, "tshl: spans past the text are dropped", vim.inspect(marks))
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
  H.check(#vim.api.nvim_buf_get_extmarks(buf, tshl.ns, 0, -1, {}) == 0, "tshl: clear takes the colours off")
  vim.api.nvim_buf_delete(buf, { force = true })

  local preview = require("cogitator.pick.preview")
  H.check(preview.code_of("/p/models/a.sql") == "dbt", "preview: a model is dbt")
  H.check(preview.code_of("/p/target/compiled/p/models/a.sql") == "sql", "preview: compiled SQL is SQL")

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
