--- workspace/executeCommand wrappers (spec §1.6, §5.1): every dbt.* command
--- takes one argument object { uri, position? }.

local M = {}

local function client_for(bufnr)
  return vim.lsp.get_clients({ bufnr = bufnr, name = "cogitator" })[1]
end

--- The single argument object of spec §1.6 (exposed for the test suite).
function M.build_argument(bufnr, with_position)
  local arg = { uri = vim.uri_from_bufnr(bufnr) }
  if with_position then
    local pos = vim.api.nvim_win_get_cursor(0)
    arg.position = { line = pos[1] - 1, character = pos[2] }
  end
  return arg
end

local function exec(command, with_position, handler)
  local bufnr = vim.api.nvim_get_current_buf()
  local client = client_for(bufnr)
  if not client then
    vim.notify("cogitator is not attached to this buffer", vim.log.levels.ERROR)
    return
  end
  client:exec_cmd({
    command = command,
    arguments = { M.build_argument(bufnr, with_position) },
  }, { bufnr = bufnr }, handler)
end

function M.run()
  exec("dbt.run", false)
end

function M.test()
  exec("dbt.test", false)
end

function M.compile()
  exec("dbt.compile", false) -- success opens the compiled SQL via showDocument
end

function M.parse()
  exec("dbt.parse", false) -- project-wide; success refreshes the server context
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

function M.lineage()
  exec("dbt.showLineage", false) -- result arrives as a window/showMessage
end

function M.schema()
  local bufnr = vim.api.nvim_get_current_buf()
  local client = client_for(bufnr)
  if not client then
    vim.notify("cogitator is not attached to this buffer", vim.log.levels.ERROR)
    return
  end
  client:exec_cmd({
    command = "dbt.goToSchema",
    arguments = { M.build_argument(bufnr, true) },
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
  local bufnr = vim.api.nvim_get_current_buf()
  local client = vim.lsp.get_clients({ bufnr = bufnr, name = "cogitator" })[1]
  if not client then
    vim.notify("cogitator is not attached to this buffer", vim.log.levels.ERROR)
    return
  end
  client:exec_cmd({
    command = "dbt.modelTests",
    arguments = { M.build_argument(bufnr, false) },
  }, { bufnr = bufnr }, function(err, result)
    if err or not result or result == vim.NIL or not result.tests or #result.tests == 0 then
      vim.notify(
        "No tests defined for this model. Add data_tests entries in its "
          .. "schema YAML (:DbtExtract scaffolds the entry).",
        vim.log.levels.INFO
      )
      return
    end
    vim.ui.select(result.tests, {
      prompt = ("tests for %s"):format(result.model),
      format_item = function(t)
        local label = t.name
        if t.column and t.column ~= vim.NIL then
          label = label .. " on " .. t.column
        end
        if t.version and t.version ~= vim.NIL then
          label = label .. (" (v%d)"):format(t.version)
        end
        return label
      end,
    }, function(choice)
      if choice then
        vim.lsp.util.show_document(
          { uri = choice.uri, range = choice.range },
          client.offset_encoding,
          { focus = true }
        )
      end
    end)
  end)
end

--- :DbtTags — pick a tag, then a model carrying it, and open it.
function M.tags()
  local bufnr = vim.api.nvim_get_current_buf()
  local client = vim.lsp.get_clients({ bufnr = bufnr, name = "cogitator" })[1]
  if not client then
    vim.notify("cogitator is not attached to this buffer", vim.log.levels.ERROR)
    return
  end
  client:exec_cmd({
    command = "dbt.projectGraph",
    arguments = { M.build_argument(bufnr, false) },
  }, { bufnr = bufnr }, function(err, graph)
    if err or not graph or graph == vim.NIL then
      vim.notify("dbt.projectGraph failed", vim.log.levels.ERROR)
      return
    end
    local by_tag = {}
    for _, node in ipairs(graph.nodes) do
      for _, tag in ipairs(node.tags or {}) do
        by_tag[tag] = by_tag[tag] or {}
        table.insert(by_tag[tag], node)
      end
    end
    local tags = vim.tbl_keys(by_tag)
    table.sort(tags)
    if #tags == 0 then
      vim.notify("no tags in this project (tags: in YAML or {{ config(tags=…) }})", vim.log.levels.INFO)
      return
    end
    vim.ui.select(tags, {
      prompt = "dbt tag",
      format_item = function(tag)
        return ("#%s  (%d model%s)"):format(tag, #by_tag[tag], #by_tag[tag] == 1 and "" or "s")
      end,
    }, function(tag)
      if not tag then
        return
      end
      vim.ui.select(by_tag[tag], {
        prompt = "#" .. tag,
        format_item = function(node)
          return node.name
        end,
      }, function(node)
        if node and node.uri and node.uri ~= vim.NIL then
          vim.cmd.edit(vim.uri_to_fname(node.uri))
        end
      end)
    end)
  end)
end

--- :DbtSelectors — pick a selectors.yml entry and jump to it.
function M.selectors()
  local bufnr = vim.api.nvim_get_current_buf()
  local client = vim.lsp.get_clients({ bufnr = bufnr, name = "cogitator" })[1]
  if not client then
    vim.notify("cogitator is not attached to this buffer", vim.log.levels.ERROR)
    return
  end
  client:request("workspace/symbol", { query = "selector:" }, function(err, symbols)
    if err or not symbols or #symbols == 0 then
      vim.notify("no selectors.yml entries in this project", vim.log.levels.INFO)
      return
    end
    vim.ui.select(symbols, {
      prompt = "dbt selector",
      format_item = function(s)
        return s.name
      end,
    }, function(symbol)
      if symbol then
        vim.lsp.util.show_document(symbol.location, client.offset_encoding, { focus = true })
      end
    end)
  end, bufnr)
end

--- :DbtStatus — what the server knows and has been doing.
function M.status()
  local bufnr = vim.api.nvim_get_current_buf()
  local client = vim.lsp.get_clients({ bufnr = bufnr, name = "cogitator" })[1]
  if not client then
    vim.notify("cogitator is not attached to this buffer", vim.log.levels.ERROR)
    return
  end
  client:exec_cmd({
    command = "dbt.status",
    arguments = { M.build_argument(bufnr, false) },
  }, { bufnr = bufnr }, function(err, s)
    if err or not s or s == vim.NIL then
      vim.notify("dbt.status failed" .. (err and (": " .. err.message) or ""), vim.log.levels.ERROR)
      return
    end
    local target = s.profiles.activeTarget
    local lines = {
      ("cogitator %s — backend %s, fusion %s"):format(s.server.version, s.server.backend, s.server.fusion),
      ("project %s (%s): %d models, %d sources, %d graph edges"):format(
        s.project.name ~= vim.NIL and s.project.name or "?",
        s.project.dialect,
        s.project.models,
        s.project.sources,
        s.project.graphEdges
      ),
      ("profiles: %s / %s"):format(
        s.profiles.activeProfile ~= "" and s.profiles.activeProfile or "?",
        (target ~= vim.NIL and target ~= "") and target or "(default target)"
      ),
      ("refresh #%d in %d ms (cache %d hits / %d misses)"):format(
        s.refresh.count,
        s.refresh.lastMs,
        s.refresh.cacheHits,
        s.refresh.cacheMisses
      ),
    }
    if s.project.root ~= vim.NIL then
      table.insert(lines, "root: " .. s.project.root)
    end
    vim.notify(table.concat(lines, "\n"), vim.log.levels.INFO)
  end)
end

--- Lens-aware wrapper: macro call-site lenses carry command == "" and are
--- informational only (spec §5.2).
function M.run_lens()
  local bufnr = vim.api.nvim_get_current_buf()
  local row = vim.api.nvim_win_get_cursor(0)[1] - 1
  for _, lens in ipairs(vim.lsp.codelens.get(bufnr)) do
    if lens.range.start.line == row and lens.command and lens.command.command == "" then
      vim.notify(lens.command.title .. " (informational lens)", vim.log.levels.INFO)
      return
    end
  end
  vim.lsp.codelens.run()
end

return M
