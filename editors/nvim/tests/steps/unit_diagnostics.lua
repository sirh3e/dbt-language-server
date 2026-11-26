-- cogitator.diagnostics without a server: what a problem's inline text,
-- its fixes and :Dbt fix's decision are, from the payloads the server
-- sends.

return function(ctx)
  local H = ctx.H
  local diag = require("cogitator.diagnostics")

  local replace = {
    message = "materialization 'incrementl' does not exist — did you mean 'incremental'? dbt fails to build the model.",
    user_data = {
      lsp = {
        code = "invalid-materialization",
        data = { fix = "replace_token", suggestions = { "incremental", "ephemeral" } },
      },
    },
  }
  local rewrite = {
    message = "model 'orders' refs itself. dbt refuses a model that depends on itself (a cycle).",
    user_data = {
      lsp = {
        code = "ref-self",
        data = {
          fix = {
            multi_edit = { fixes = { { title = "Use {{ this }} for the model's own relation", edits = {} } } },
          },
          suggestions = {},
        },
      },
    },
  }
  local plain =
    { message = "the model has no SQL. dbt builds a model from its SELECT.", user_data = { lsp = {} } }

  H.check(
    vim.deep_equal(diag.fixes(replace), { "'incremental'", "'ephemeral'" }),
    "diagnostics: a did-you-mean's fixes are its suggestions",
    vim.inspect(diag.fixes(replace))
  )
  H.check(
    vim.deep_equal(diag.fixes(rewrite), { "Use {{ this }} for the model's own relation" }),
    "diagnostics: a multi-edit's fixes are its titles"
  )
  H.check(#diag.fixes(plain) == 0, "diagnostics: a problem without data has no fix")
  H.check(
    diag.headline(replace.message)
      == "materialization 'incrementl' does not exist — did you mean 'incremental'?",
    "diagnostics: the headline keeps the did-you-mean and drops the reason",
    diag.headline(replace.message)
  )
  H.check(
    diag.headline(rewrite.message) == "model 'orders' refs itself",
    "diagnostics: the headline is the first sentence",
    diag.headline(rewrite.message)
  )
  diag.FIX_KEY = "<leader>di"
  H.check(
    vim.endswith(diag.inline(replace), "✚ <leader>di") and not diag.inline(plain):find("✚", 1, true),
    "diagnostics: inline text names the fix key only when a fix exists",
    diag.inline(replace)
  )
  local float = diag.float(rewrite)
  H.check(
    float:find("Fix (<leader>di or :Dbt fix):", 1, true) ~= nil
      and float:find("✚ Use {{ this }}", 1, true) ~= nil,
    "diagnostics: the float lists the fixes under the message",
    float
  )
  H.check(
    diag.float(plain) == plain.message,
    "diagnostics: the float of a problem without a fix is its message"
  )

  local edit = { documentChanges = { { textDocument = { uri = "file:///m.sql" }, edits = {} } } }
  local create = { documentChanges = { { kind = "create", uri = "file:///n.sql" } } }
  local one = { { kind = "quickfix", title = "Change to 'incremental'", edit = edit } }
  local what, action = diag.decide(one)
  H.check(what == "apply" and action == one[1], ":Dbt fix: one fix is applied")
  local two = {
    { kind = "quickfix", title = "Change to 'a'", edit = edit },
    { kind = "quickfix", title = "Change to 'b'", edit = edit },
  }
  H.check(diag.decide(two) == "choose", ":Dbt fix: several fixes are chosen from")
  local preferred =
    { two[1], { kind = "quickfix", title = "Change to 'b'", edit = edit, isPreferred = true } }
  local _, chosen = diag.decide(preferred)
  H.check(chosen == preferred[2], ":Dbt fix: the preferred fix is applied")
  H.check(
    diag.decide({ { kind = "quickfix", title = "Create model 'x'", edit = create } }) == "choose",
    ":Dbt fix: a fix that creates a file is never applied unasked"
  )
  H.check(
    diag.decide({ { kind = "refactor", title = "Extract", edit = edit } }) == "none",
    ":Dbt fix: only quickfixes count"
  )

  -- :DbtTestSql's headers lead to the tests, in order, by label
  local views = require("cogitator.commands.views")
  local lines, headers = views.test_sql_lines({
    text = "-- The tests of orders\n\n-- ── unique on order_id · severity error\nselect 1\n\n"
      .. "-- ── not_null on order_id · severity warn\nselect 2\n",
    tests = { { label = "unique on order_id" }, { label = "not_null on order_id" } },
  })
  H.check(#lines == 7, ":DbtTestSql: the server's text, a line each", vim.inspect(lines))
  H.check(
    headers[3] and headers[3].label == "unique on order_id" and headers[6].label == "not_null on order_id",
    ":DbtTestSql: each header line knows its test",
    vim.inspect(headers)
  )
  H.check(
    vim.tbl_contains(views.STRATEGIES, "delete+insert"),
    ":DbtIncrementalSql: the strategies are completed"
  )
end
