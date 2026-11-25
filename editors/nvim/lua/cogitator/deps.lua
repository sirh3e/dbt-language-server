--- The dependencies dialog (`<leader>dp`, `:DbtDependencies [model]`): one
--- model, how it is built, what it uses and what uses it — directly, then
--- everything upstream and downstream, nearest first — from the server's
--- dbt.modelDependencies, in the yazi-style picker. Every node says how it
--- is built (materialized; an incremental model its strategy, unique key
--- and partition); <CR> opens it.

local actions = require("cogitator.actions")
local paint = require("cogitator.pick.paint")

local M = {}

local function present(v)
  return v ~= nil and v ~= vim.NIL and v ~= ""
end

local function list(v)
  return (type(v) == "table" and #v > 0) and v or nil
end

--- A node's config on one line: `incremental · merge · unique_key id, day
--- · partition day` (pure; exposed for the suite).
---@param config table? the answer's `config`
---@return string
function M.summary(config)
  if type(config) ~= "table" then
    return ""
  end
  local parts = {}
  if present(config.materialized) then
    parts[#parts + 1] = config.materialized
  end
  if present(config.incrementalStrategy) then
    parts[#parts + 1] = config.incrementalStrategy
  end
  if list(config.uniqueKey) then
    parts[#parts + 1] = "unique_key " .. table.concat(config.uniqueKey, ", ")
  end
  if list(config.partitionBy) then
    parts[#parts + 1] = "partition " .. table.concat(config.partitionBy, ", ")
  end
  if list(config.clusterBy) then
    parts[#parts + 1] = "cluster " .. table.concat(config.clusterBy, ", ")
  end
  return table.concat(parts, " · ")
end

--- The preview of a node: what it is, where, and every part of its config.
---@param node table
---@return {lines: string[]}
function M.card(node)
  local c = node.config or {}
  local file = present(node.uri) and vim.fn.fnamemodify(vim.uri_to_fname(node.uri), ":~:.") or "· no file ·"
  local lines = {
    "  " .. node.name,
    ("  %s%s  ·  %s"):format(node.kind, present(node.package) and (" of " .. node.package) or "", file),
    "",
  }
  local function row(label, value)
    if present(value) then
      lines[#lines + 1] = ("  %-20s %s"):format(label, value)
    end
  end
  row("materialized", c.materialized)
  row("incremental_strategy", c.incrementalStrategy)
  row("unique_key", list(c.uniqueKey) and table.concat(c.uniqueKey, ", ") or nil)
  row("partition_by", list(c.partitionBy) and table.concat(c.partitionBy, ", ") or nil)
  row("cluster_by", list(c.clusterBy) and table.concat(c.clusterBy, ", ") or nil)
  row("on_schema_change", c.onSchemaChange)
  if c.materialized == "incremental" and not list(c.uniqueKey) then
    lines[#lines + 1] = "  (no unique_key: every run appends)"
  end
  if list(node.tags) then
    lines[#lines + 1] = ""
    lines[#lines + 1] = "  #" .. table.concat(node.tags, " #")
  end
  if node.depth and node.depth > 0 then
    lines[#lines + 1] = ""
    lines[#lines + 1] = ("  %d hop%s away"):format(node.depth, node.depth == 1 and "" or "s")
  end
  lines[#lines + 1] = ""
  lines[#lines + 1] = "  <CR> opens it"
  return { lines = lines }
end

--- A node as a picker leaf; `arrow` marks the direction of a walk.
local function leaf(node, arrow)
  local group = paint.kind_group(node.kind, node.uri) or "DbtPickerSource"
  local summary = M.summary(node.config)
  local detail = node.kind .. (summary ~= "" and (" · " .. summary) or "")
  return {
    label = (arrow and (arrow .. node.depth .. " ") or "") .. node.name,
    icon = node.config and node.config.materialized == "incremental" and "◈" or "◆",
    icon_hl = group,
    hl = group,
    detail = detail,
    value = node,
    disabled = not present(node.uri) and "no file on disk" or nil,
    preview = function()
      return M.card(node)
    end,
  }
end

--- The dialog's items (pure; exposed for the suite): the model itself,
--- then a group per direction, direct before transitive.
---@param deps table dbt.modelDependencies' answer
---@return table[]
function M.items(deps)
  local items = { leaf(deps.model) }
  local function group(label, nodes, arrow, icon, hl)
    local children = {}
    for _, node in ipairs(nodes or {}) do
      children[#children + 1] = leaf(node, arrow)
    end
    if #children == 0 then
      children[1] = { label = "· nothing ·", icon = " ", disabled = "nothing here", value = {} }
    end
    items[#items + 1] = {
      label = label,
      icon = icon,
      icon_hl = hl,
      detail = tostring(#(nodes or {})),
      children = children,
    }
  end
  group("Uses directly", deps.parents, nil, "↑", "DbtPickUpstream")
  group("Used directly by", deps.children, nil, "↓", "DbtPickDownstream")
  group("Everything upstream", deps.upstream, "↑", "⇈", "DbtPickUpstream")
  group("Everything downstream", deps.downstream, "↓", "⇊", "DbtPickDownstream")
  return items
end

--- The dialog's title: the model and how it is built.
---@param deps table
---@return string
function M.title(deps)
  local summary = M.summary(deps.model.config)
  return "dependencies · " .. deps.model.name .. (summary ~= "" and (" · " .. summary) or "")
end

--- Open the dialog for `model` (a name), else the buffer's model.
---@param opts {model?: string}?
function M.open(opts)
  opts = type(opts) == "table" and opts or {}
  local bufnr = vim.api.nvim_get_current_buf()
  local client = actions.client_for(bufnr)
  if not client then
    return
  end
  local model = opts.model
  if model == "" then
    model = nil
  end
  actions.exec_with(client, bufnr, "dbt.modelDependencies", {
    uri = vim.uri_from_bufnr(bufnr),
    model = model,
  }, function(err, deps)
    if err then
      vim.notify("dbt.modelDependencies failed: " .. err.message, vim.log.levels.ERROR)
      return
    end
    if type(deps) ~= "table" or type(deps.model) ~= "table" then
      vim.notify(
        (model and ("%s is no model of the project"):format(model))
          or "not a model — open one, or :DbtDeps <model>",
        vim.log.levels.WARN
      )
      return
    end
    require("cogitator.picker").open({
      title = M.title(deps),
      items = M.items(deps),
      on_choose = function(item)
        local node = item.value
        if present(node.uri) and actions.open_uri(node.uri) then
          pcall(vim.api.nvim_win_set_cursor, 0, { (node.line or 0) + 1, 0 })
        end
      end,
    })
  end)
end

return M
