--- A model's CTEs (one picker, four ways to look at one):
---
---   :DbtPreviewCte[!] [limit]  the rows of the CTE under the cursor (or of
---                              the final select) via dbt.previewCte: the
---                              model run up to that CTE through
---                              `dbt show --inline`; in a split, or with `!`
---                              (or `preview.cte = "float"`) in a float at
---                              the cursor, <leader>dC
---   :DbtCteSql [cte]           the query that selects it, without the
---                              warehouse (dbt.cteSql), in a SQL split
---   :DbtCtes                   every CTE of the model in the picker, its
---                              columns and lineage (the hover card) in the
---                              preview; choosing one offers the rows (float
---                              or split), its SQL and a jump to it,
---                              <leader>dK
---
--- The hover on a CTE's name shows the same card anywhere (K).

local actions = require("cogitator.actions")

local M = {}

--- The split's lines for a dbt.previewCte answer (exposed for the tests):
--- the server's markdown table, which already names the model, the CTE,
--- the row count and the query.
---@param result table
---@return string[]
function M.lines(result)
  local text = type(result.table) == "string" and result.table or ""
  return vim.split(text:gsub("\n$", ""), "\n")
end

--- The float's size for `lines` within the editor (exposed for the tests):
--- as wide as the widest line, at most 90% of the columns, and as tall as
--- the lines, at most 70% of the rows.
---@param lines string[]
---@return integer width, integer height
function M.float_size(lines)
  local widest = 1
  for _, line in ipairs(lines) do
    widest = math.max(widest, vim.fn.strdisplaywidth(line))
  end
  local width = math.min(widest, math.max(20, math.floor(vim.o.columns * 0.9)))
  local height = math.min(#lines, math.max(3, math.floor(vim.o.lines * 0.7)))
  return width, height
end

--- How a preview is shown: a split, or a float at the cursor.
---@param result table dbt.previewCte's answer
---@param how "split"|"float"
function M.show(result, how)
  local lines = M.lines(result)
  if how == "float" then
    local width, height = M.float_size(lines)
    local buf, win = vim.lsp.util.open_floating_preview(lines, "markdown", {
      border = "rounded",
      focus_id = "cogitator_cte_preview",
      width = width,
      height = height,
      max_width = width,
      max_height = height,
    })
    for _, key in ipairs({ "q", "<Esc>" }) do
      vim.keymap.set("n", key, function()
        pcall(vim.api.nvim_win_close, win, true)
      end, { buffer = buf, nowait = true })
    end
    return buf, win
  end
  vim.cmd.split()
  local scratch = vim.api.nvim_create_buf(false, true)
  vim.api.nvim_buf_set_lines(scratch, 0, -1, false, lines)
  vim.bo[scratch].filetype = "markdown"
  vim.bo[scratch].modifiable = false
  vim.api.nvim_win_set_buf(0, scratch)
  vim.keymap.set("n", "q", vim.cmd.close, { buffer = scratch, nowait = true })
  return scratch, vim.api.nvim_get_current_win()
end

--- The argument of a request about a CTE: the one named, else the
--- cursor's (`cte = false`: the final select).
local function argument(bufnr, cte)
  local arg = { uri = vim.uri_from_bufnr(bufnr) }
  if type(cte) == "string" then
    arg.cte = cte
  elseif cte == nil then
    local pos = vim.api.nvim_win_get_cursor(0)
    arg.position = { line = pos[1] - 1, character = pos[2] }
  end
  return arg
end

--- The default way to show rows (`preview.cte` of the setup).
local function default_how()
  local config = require("cogitator").config
  return (config and config.preview and config.preview.cte) or "split"
end

--- Preview the rows of a CTE.
---@param opts? {args?: string, bang?: boolean, cte?: string|false, how?: "split"|"float"}
function M.preview_cte(opts)
  opts = opts or {}
  local bufnr = vim.api.nvim_get_current_buf()
  local client = actions.client_for(bufnr)
  if not client then
    return
  end
  local arg = argument(bufnr, opts.cte)
  local limit = tonumber(opts.args or "")
  if limit then
    arg.limit = limit
  end
  local how = opts.how or (opts.bang and "float") or default_how()
  client:exec_cmd(
    { command = "dbt.previewCte", arguments = { arg } },
    { bufnr = bufnr },
    function(err, result)
      if err or type(result) ~= "table" then
        vim.notify("dbt.previewCte failed" .. (err and (": " .. err.message) or ""), vim.log.levels.ERROR)
        return
      end
      M.show(result, how)
    end
  )
end

--- The rows of the CTE under the cursor in a float (<leader>dC).
function M.float()
  M.preview_cte({ how = "float" })
end

--- :DbtCteSql [cte] — the query that selects the CTE (the cursor's, or
--- the one named), Jinja resolved and without the warehouse.
---@param opts? {args?: string, cte?: string|false}
function M.cte_sql(opts)
  opts = opts or {}
  local bufnr = vim.api.nvim_get_current_buf()
  local client = actions.client_for(bufnr)
  if not client then
    return
  end
  local named = vim.trim(opts.args or "")
  local cte = opts.cte
  if cte == nil and named ~= "" then
    cte = named
  end
  client:exec_cmd(
    { command = "dbt.cteSql", arguments = { argument(bufnr, cte) } },
    { bufnr = bufnr },
    function(err, result)
      if err or type(result) ~= "table" then
        vim.notify("dbt.cteSql failed" .. (err and (": " .. err.message) or ""), vim.log.levels.ERROR)
        return
      end
      vim.cmd.vsplit()
      local scratch = vim.api.nvim_create_buf(false, true)
      vim.api.nvim_buf_set_lines(scratch, 0, -1, false, vim.split((result.text or ""):gsub("\n$", ""), "\n"))
      vim.bo[scratch].filetype = "sql"
      vim.bo[scratch].modifiable = false
      vim.api.nvim_win_set_buf(0, scratch)
      vim.keymap.set("n", "q", vim.cmd.close, { buffer = scratch, nowait = true })
    end
  )
end

--- The CTEs of a documentSymbol answer, in document order (exposed for
--- the tests): the `Struct` symbols of this file, then the final select.
---@param symbols table[] SymbolInformation[]
---@param uri string
---@param defined? string[] the CTE names the text defines (M.names_in):
---  a `Struct` symbol outside it is no CTE (`unnest(…) with ordinality`)
---@return {name: string|false, line: integer, character: integer}[]
function M.ctes_of(symbols, uri, defined)
  local out = {}
  for _, s in ipairs(symbols or {}) do
    local loc = s.location
    local is_cte = not defined or vim.tbl_contains(defined, s.name)
    if s.kind == vim.lsp.protocol.SymbolKind.Struct and loc and loc.uri == uri and is_cte then
      out[#out + 1] = { name = s.name, line = loc.range.start.line, character = loc.range.start.character }
    end
  end
  table.sort(out, function(a, b)
    return a.line < b.line or (a.line == b.line and a.character < b.character)
  end)
  out[#out + 1] = { name = false, line = -1, character = 0 }
  return out
end

--- A hover answer's text as preview lines (exposed for the tests).
---@param result table? the hover result
---@param name string|false the CTE
---@return {lines: string[], syntax: string}
function M.card_lines(result, name)
  if not name then
    return { lines = { "the final select of the model" }, syntax = "markdown" }
  end
  local text = result and result.contents
  text = type(text) == "table" and text.value or text
  local lines = type(text) == "string" and text ~= "" and vim.split(text, "\n")
    or { "(no card for " .. name .. ")" }
  return { lines = lines, syntax = "markdown" }
end

--- Every CTE's card, asked at once and never waited for: `on_done(cards)`
--- once each has answered or timed out, cards by CTE name.
local function fetch_cards(client, bufnr, ctes, on_done)
  local cards, pending = {}, 0
  local function settle()
    pending = pending - 1
    if pending == 0 then
      on_done(cards)
    end
  end
  for _, cte in ipairs(ctes) do
    if cte.name then
      pending = pending + 1
    end
  end
  if pending == 0 then
    return on_done(cards)
  end
  for _, cte in ipairs(ctes) do
    if cte.name then
      actions.request(client, bufnr, "textDocument/hover", {
        textDocument = { uri = vim.uri_from_bufnr(bufnr) },
        position = { line = cte.line, character = cte.character },
      }, { timeout_ms = 3000 }, function(_, result)
        cards[cte.name] = M.card_lines(result, cte.name)
        settle()
      end)
    end
  end
end

--- The CTE names a buffer's text defines (exposed for the tests): what
--- `:DbtCteSql` completes, read from the text so completion never waits.
---@param lines string[]
---@return string[]
function M.names_in(lines)
  local names, seen = {}, {}
  local text = table.concat(lines, "\n"):gsub("{#.-#}", ""):gsub("%-%-[^\n]*", "")
  for before, name in text:gmatch("([%w_,%s]-)([%a_][%w_]*)%s+[aA][sS]%s*%(") do
    local keyword = before:match("([%a_]+)%s*$")
    local lead = before:match("[,]%s*$") or (keyword and keyword:lower() == "with")
    if lead and not seen[name] and name:lower() ~= "with" then
      seen[name] = true
      names[#names + 1] = name
    end
  end
  return names
end

--- The picker's items for the CTEs (exposed for the tests): one group a
--- CTE, its actions inside.
---@param ctes table[] M.ctes_of's answer
---@param preview fun(item: table): table
---@return cogitator.picker.Item[]
function M.items(ctes, preview)
  local items = {}
  for _, cte in ipairs(ctes) do
    local name = cte.name
    local function act(label, icon, what)
      return { label = label, icon = icon, value = { what = what, cte = cte } }
    end
    items[#items + 1] = {
      label = name or "final select",
      icon = name and "◇" or "▣",
      detail = name and ("line " .. (cte.line + 1)) or nil,
      preview = preview,
      value = { cte = cte },
      children = {
        act("Preview rows in a float", "▤", "float"),
        act("Preview rows in a split", "▥", "split"),
        act("Show its SQL", "≡", "sql"),
        name and act("Jump to it", "→", "jump") or nil,
      },
    }
  end
  return items
end

--- :DbtCtes — every CTE of this model in the picker (<leader>dK).
function M.pick()
  local bufnr = vim.api.nvim_get_current_buf()
  local client = actions.client_for(bufnr)
  if not client then
    return
  end
  local uri = vim.uri_from_bufnr(bufnr)
  client:request("textDocument/documentSymbol", { textDocument = { uri = uri } }, function(err, symbols)
    if err then
      vim.notify("textDocument/documentSymbol failed: " .. err.message, vim.log.levels.ERROR)
      return
    end
    local ctes = M.ctes_of(symbols, uri, M.names_in(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)))
    local win = vim.api.nvim_get_current_win()
    fetch_cards(client, bufnr, ctes, function(cards)
      local items = M.items(ctes, function(item)
        local name = item.value.cte.name
        return name and cards[name] or M.card_lines(nil, name)
      end)
      require("cogitator.picker").open({
        title = "CTEs · " .. vim.fn.fnamemodify(vim.api.nvim_buf_get_name(bufnr), ":t"),
        items = items,
        on_choose = function(item)
          local v = item.value or {}
          if not v.what then
            return
          end
          local cte = v.cte
          if vim.api.nvim_win_is_valid(win) then
            vim.api.nvim_set_current_win(win)
          end
          if v.what == "jump" then
            vim.api.nvim_win_set_cursor(0, { cte.line + 1, cte.character })
          elseif v.what == "sql" then
            M.cte_sql({ cte = cte.name })
          else
            M.preview_cte({ cte = cte.name, how = v.what })
          end
        end,
      })
    end)
  end, bufnr)
end

return M
