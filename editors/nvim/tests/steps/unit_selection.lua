-- dbt selections in a finder, without a server: what reads as a
-- selection and what stays fzf, the inline --exclude, completion from a
-- vocabulary, the counts line, and an answer as items.

return function(ctx)
  local H = ctx.H
  local selection = require("cogitator.pick.selection")

  for _, c in ipairs({
    { "tag:nightly", true },
    { "config.materialized:incremental", true },
    { "+dim_items", true },
    { "dim_items+", true },
    { "2+dim_items+1", true },
    { "@dim_items", true },
    { "stg_*", true },
    { "tag:a,resource_type:model", true },
    { "dim_items --exclude resource_type:test", true },
    { "tag:", true },
    { "dim_items", false },
    { "stg cust", false },
    { "'orders", false },
    { "^stg", false },
    { "orders$", false },
    { "!stg ^cust", false },
    { "pay | ^cust", false },
    { "notamethod:x", true },
    { "12:30", false },
    { "", false },
  }) do
    H.check(
      selection.is_selection(c[1]) == c[2],
      ("selection: %q %s"):format(c[1], c[2] and "is a dbt selection" or "stays fzf words")
    )
  end

  for _, c in ipairs({
    { "tag:a +b --exclude resource_type:test", "tag:a +b", "resource_type:test" },
    { "tag:a", "tag:a", nil },
    { "tag:a --exclude ", "tag:a", nil },
  }) do
    local select, exclude = selection.split(c[1])
    H.check(
      select == c[2] and exclude == c[3],
      ("selection: %q splits off its exclusion"):format(c[1]),
      vim.inspect({ select, exclude })
    )
  end

  local vocab = {
    methods = { "tag", "path", "package", "resource_type", "config.materialized" },
    values = {
      tag = { "hourly", "history", "nightly" },
      resource_type = { "model", "seed", "test" },
      ["config.materialized"] = { "incremental", "table" },
      fqn = { "dim_items", "dim_states", "hub_item" },
    },
  }
  for _, c in ipairs({
    -- query, cursor, completed query, candidates
    { "ta", 2, "tag:", { "tag:" } },
    { "tag:n", 5, "tag:nightly ", { "tag:nightly" } },
    { "tag:h", 5, "tag:h", { "tag:history", "tag:hourly" } },
    { "+dim_i", 6, "+dim_items ", { "dim_items" } },
    { "tag:nightly,resource_type:s", 27, "tag:nightly,resource_type:seed ", { "resource_type:seed" } },
    {
      "config.materialized:inc",
      23,
      "config.materialized:incremental ",
      { "config.materialized:incremental" },
    },
    { "zz", 2, "zz", {} },
    { "ta +x", 2, "tag: +x", { "tag:" } },
  }) do
    local done = selection.complete(c[1], c[2], vocab)
    H.check(
      done.query == c[3] and vim.deep_equal(done.candidates, c[4]),
      ("selection: <Tab> after %q completes to %q"):format(c[1]:sub(1, c[2]), c[3]),
      vim.inspect(done)
    )
  end
  H.check(
    vim.deep_equal(selection.candidates("2+dim_i", vocab), { "2+dim_items" }),
    "selection: the command line's candidates keep the graph operators"
  )

  H.check(
    selection.summary({ model = 3, test = 2, seed = 1, unit_test = 1 })
      == "7 · 3 models, 1 seed, 2 tests, 1 unit test",
    "selection: the counts line reads kind by kind",
    selection.summary({ model = 3, test = 2, seed = 1, unit_test = 1 })
  )
  H.check(selection.summary({}) == "0", "selection: nothing selected says 0")

  local select_source = require("cogitator.pick.select_source")
  local answer = select_source.answer({
    nodes = {
      {
        uniqueId = "model.vault_lake.bv_item_transitions.v1",
        resourceType = "model",
        name = "bv_item_transitions",
        version = "1",
        tags = { "hourly" },
        graphId = "model:bv_item_transitions",
        uri = "file:///p/models/bv.sql",
      },
      {
        uniqueId = "source.vault_lake.landing.items",
        resourceType = "source",
        name = "items",
        tags = {},
        graphId = "source:landing",
        uri = "file:///p/models/_sources.yml",
        range = { start = { line = 4, character = 8 }, ["end"] = { line = 4, character = 8 } },
      },
      {
        uniqueId = "test.vault_lake.not_null_dim_items_item_id",
        resourceType = "test",
        name = "not_null_dim_items_item_id",
        tags = {},
        uri = "file:///p/models/dim/_dim__models.yml",
      },
    },
    counts = { model = 1, source = 1, test = 1 },
  })
  local texts = vim.tbl_map(function(i)
    return i.text
  end, answer.items)
  H.check(
    vim.deep_equal(texts, {
      "bv_item_transitions.v1  [model]  #hourly",
      "landing.items  [source]",
      "not_null_dim_items_item_id  [test]",
    }),
    "selection: an answer lists dbt's names with their kind and tags",
    vim.inspect(texts)
  )
  H.check(
    answer.items[1].name == "bv_item_transitions"
      and answer.items[2].name == "landing"
      and answer.items[2].selector == "source:landing.items"
      and answer.items[2].lnum == 5
      and answer.items[3].value.in_graph == false,
    "selection: an item focuses its graph node, selects itself alone and lands on its line",
    vim.inspect(answer.items)
  )
  H.check(
    answer.status == "3 · 1 model, 1 source, 1 test",
    "selection: the answer's counts line",
    answer.status
  )
  local failed = select_source.answer({
    nodes = {},
    counts = {},
    error = { message = "no method", start = 0, ["end"] = 4 },
  })
  H.check(failed.error and failed.error.start == 0, "selection: an error keeps its span for the prompt")
end
