--- The sources by name: their order, what each lists, and building one.

local M = {}

--- The sources a :DbtFind with no argument offers, in this order.
M.ORDER = {
  "models",
  "sources",
  "all",
  "definitions",
  "macros",
  "columns",
  "upstream",
  "downstream",
  "lineage",
  "column_lineage",
  "refs",
  "tests",
  "tags",
  "selectors",
  "select",
  "astgrep",
  "unresolved",
}

M.DESCRIPTIONS = {
  models = "every model",
  sources = "every source",
  all = "every node: models, sources, exposures, metrics…",
  definitions = "models, sources, macros and selectors — jump to the definition",
  macros = "every macro",
  columns = "every documented column, as model.column",
  upstream = "everything upstream of this model, nearest first",
  downstream = "everything downstream of this model, nearest first",
  lineage = "upstream and downstream of this model",
  column_lineage = "where each column of this model comes from (<leader>dl)",
  refs = "the ref(), source(), CTEs and macros of this file",
  tests = "the tests of this model",
  tags = "tags, then the nodes carrying one",
  selectors = "selectors.yml entries",
  select = "dbt node selection: tag:nightly, +dim_items, resource_type:test, a,b, … --exclude x",
  unresolved = "every unresolved ref, source, doc, macro and var of the project",
  astgrep = "ast-grep: a structural pattern ({{ ref($M) }}, sql:select $$$C from $T), its matches",
}

--- Build the named source for the current buffer.
---@param name string
---@param ctx cogitator.pick.Ctx
---@return table
function M.build(name, ctx)
  local make = require("cogitator.pick.sources")[name]
  if name == "lineage" then
    return require("cogitator.pick.sources").lineage(ctx, "lineage")
  end
  return make(ctx)
end

return M
