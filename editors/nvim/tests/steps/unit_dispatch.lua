-- :Dbt's routing table and completion — no server involved.

return function(ctx)
  local H = ctx.H
  local dispatch = require("cogitator.dispatch")
  local commands = require("cogitator.commands")

  local missing = vim.tbl_filter(function(name)
    return type(commands[name]) ~= "function"
  end, dispatch._routes)
  H.check(#missing == 0, "dispatch: every routed command exists", vim.inspect(missing))

  H.check(
    vim.deep_equal(dispatch.complete("c", "Dbt c"), { "cache", "cancel", "columns", "compile", "ctes", "ctesql" }),
    "dispatch: completes subcommands by prefix",
    vim.inspect(dispatch.complete("c", "Dbt c"))
  )
  H.check(
    vim.deep_equal(dispatch.complete("", "Dbt cache "), { "clear" }),
    "dispatch: completes a subcommand's argument",
    vim.inspect(dispatch.complete("", "Dbt cache "))
  )
  H.check(#dispatch.complete("", "Dbt run ") == 0, "dispatch: no argument completion where none applies")
  H.check(vim.fn.exists(":Dbt") == 2, "dispatch: :Dbt is defined")
  H.check(
    vim.fn.exists(":DbtDeps") == 2
      and vim.fn.exists(":DbtReloadConfig") == 2
      and vim.fn.exists(":DbtColumnLineage") == 2,
    "dispatch: :DbtDeps, :DbtReloadConfig and :DbtColumnLineage are defined"
  )
end
