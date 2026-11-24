--- Going somewhere from the cursor: the relation and a bounded query on
--- it, column lineage, the schema entry, extracting YAML, tests, tags
--- and selectors.

local actions = require("cogitator.actions")
local shared = require("cogitator.commands.shared")

local client_for, exec = actions.client_for, shared.exec

local M = {}

--- :DbtQuery — bounded SELECT on the relation under the cursor: through
--- vim-dadbod when a connection is configured, else into a scratch sql
--- buffer ready for any client.
function M.query()
  local cfg = require("cogitator").config
  local limit = (cfg and cfg.query_limit) or 100
  exec("dbt.relation", true, function(err, result)
    if err then
      vim.notify("dbt.relation failed: " .. err.message, vim.log.levels.ERROR)
      return
    end
    if not result or result == vim.NIL then
      vim.notify("no relation here — not a dbt model, ref() or source()", vim.log.levels.WARN)
      return
    end
    local statement = ("select * from %s limit %d"):format(result.relation, limit)
    if vim.fn.exists(":DB") == 2 and (vim.b.db or vim.g.db) then
      vim.cmd.DB(statement)
      return
    end
    local buf = vim.api.nvim_create_buf(true, true)
    vim.api.nvim_buf_set_lines(buf, 0, -1, false, {
      ("-- %s %s — run with vim-dadbod (:DB) or your SQL client"):format(result.kind, result.name),
      statement,
    })
    vim.bo[buf].filetype = "sql"
    vim.api.nvim_set_current_buf(buf)
    vim.notify(("query for %s (limit %d)"):format(result.relation, limit))
  end)
end

--- dbt.relation: resolve database.schema.identifier of the source()/ref()
--- under the cursor (else the file's model), yank it for pasting into SQL.
function M.relation()
  exec("dbt.relation", true, function(err, result)
    if err then
      vim.notify("dbt.relation failed: " .. err.message, vim.log.levels.ERROR)
      return
    end
    if not result or result == vim.NIL then
      vim.notify("no relation here — not a dbt model, ref() or source()", vim.log.levels.WARN)
      return
    end
    vim.fn.setreg('"', result.relation)
    pcall(vim.fn.setreg, "+", result.relation)
    vim.notify(("%s %s → %s (yanked)"):format(result.kind, result.name, result.relation))
  end)
end

--- :DbtLineage — where each column of this model comes from, in the
--- picker (dbt.columnLineage, kept by the server per edit); when the
--- server has none, dbt.showLineage's message says why.
--- What can be yanked of a dbt.relation answer, most used first: the
--- relation, quoted, the Jinja that names it, a query of it (pure;
--- exposed for the suite).
---@param r {kind: string, name: string, source?: string, database?: string, schema?: string, identifier: string, relation: string}
---@param limit integer
---@return {label: string, detail: string, text: string}[]
function M.relation_forms(r, limit)
  local function present(v)
    return v ~= nil and v ~= vim.NIL and v ~= ""
  end
  local quoted = {}
  local parts = { r.database, r.schema, r.identifier }
  for i = 1, 3 do -- not ipairs: a source may have no database
    local part = parts[i]
    if present(part) then
      quoted[#quoted + 1] = '"' .. tostring(part):gsub('"', '""') .. '"'
    end
  end
  local jinja
  if r.kind == "source" and present(r.source) then
    jinja = ("{{ source('%s', '%s') }}"):format(r.source, r.name)
  else
    jinja = ("{{ ref('%s') }}"):format(r.name)
  end
  return {
    { label = r.relation, detail = "the relation, as the warehouse names it", text = r.relation },
    {
      label = table.concat(quoted, "."),
      detail = "quoted, for names with capitals or spaces",
      text = table.concat(quoted, "."),
    },
    {
      label = jinja,
      detail = r.kind == "source" and "the source() that names it" or "the ref() that names it",
      text = jinja,
    },
    {
      label = "select * from " .. jinja,
      detail = "a query of it, for a model or an analysis",
      text = "select * from " .. jinja,
    },
    {
      label = ("select * from %s limit %d"):format(r.relation, limit),
      detail = "a query of it, for the warehouse's own client",
      text = ("select * from %s limit %d"):format(r.relation, limit),
    },
  }
end

--- <leader>dR: the relation under the cursor (a ref(), a source(), else
--- the file's model) and the forms of it to yank, in the picker.
function M.relation_menu()
  local cfg = require("cogitator").config
  local limit = (cfg and cfg.query_limit) or 100
  exec("dbt.relation", true, function(err, result)
    if err then
      vim.notify("dbt.relation failed: " .. err.message, vim.log.levels.ERROR)
      return
    end
    if not result or result == vim.NIL then
      vim.notify("no relation here — not a dbt model, ref() or source()", vim.log.levels.WARN)
      return
    end
    local group = require("cogitator.pick.paint").kind_group(result.kind) or "DbtPickerSource"
    local items = {}
    for _, form in ipairs(M.relation_forms(result, limit)) do
      items[#items + 1] = {
        label = form.label,
        icon = "⎘",
        icon_hl = group,
        detail = form.detail,
        value = form,
        preview = function()
          return {
            "  " .. form.text,
            "",
            "  " .. form.detail,
            "",
            "  <CR> yanks it (also into the + register)",
          }
        end,
      }
    end
    require("cogitator.picker").open({
      title = ("yank · %s %s"):format(result.kind, result.name),
      items = items,
      on_choose = function(item)
        vim.fn.setreg('"', item.value.text)
        pcall(vim.fn.setreg, "+", item.value.text)
        vim.notify(item.value.text .. " (yanked)")
      end,
    })
  end)
end

function M.lineage()
  local bufnr = vim.api.nvim_get_current_buf()
  local client = client_for(bufnr)
  if not client then
    return
  end
  local pick = require("cogitator.pick")
  pick.open(require("cogitator.pick.sources").build("column_lineage", {
    client = client,
    bufnr = bufnr,
    on_none = function()
      exec("dbt.showLineage", false) -- the reason arrives as a window/showMessage
    end,
  }))
end

function M.schema()
  local bufnr = vim.api.nvim_get_current_buf()
  local client = client_for(bufnr)
  if not client then
    return
  end
  client:exec_cmd({
    command = "dbt.goToSchema",
    arguments = { shared.build_argument(bufnr, true) },
  }, { bufnr = bufnr }, function(err, result)
    if err then
      vim.notify("dbt.goToSchema failed: " .. err.message, vim.log.levels.ERROR)
      return
    end
    if not result or result == vim.NIL or not result.uri then
      vim.notify("No schema entry found for this model.", vim.log.levels.INFO)
      return
    end
    vim.lsp.util.show_document(result, client.offset_encoding, { focus = true })
  end)
end

function M.extract()
  -- scaffold <model>.yml, or add the next version; the server applies the
  -- change via workspace/applyEdit and navigates to it
  exec("dbt.extractModelYaml", false)
end

--- Picker over every test defined for this model (and its versions).
function M.tests()
  require("cogitator.pick").find("tests")
end

--- :DbtTags — pick a tag, then a model carrying it, and open it.
function M.tags()
  require("cogitator.pick").find("tags")
end

--- :DbtSelectors — pick a selectors.yml entry and jump to it.
function M.selectors()
  require("cogitator.pick").find("selectors")
end

return M
