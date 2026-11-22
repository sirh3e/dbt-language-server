--- Structural search with ast-grep, through the server (dbt.astGrep):
--- patterns over tree-sitter-dbt — `{{ ref($M) }}`, `{{ config($$$A) }}` —
--- or, prefixed `sql:`, over the compiled SQL with tree-sitter-sequel.
---
---   :DbtAstGrep {{ ref($M) }}                       matches → quickfix
---   :DbtAstGrep {{ ref($M) }} => {{ ref('x', $M) }} rewrite, after asking
---   :DbtFind astgrep  (<leader>da)                  matches in the picker
---
--- The SQL between Jinja tags is text to tree-sitter-dbt, so a dbt pattern
--- matches Jinja; a `sql:` pattern is a whole statement.

local actions = require("cogitator.actions")

local M = {}

M.DEFAULT_PATTERN = "{{ ref($M) }}"

--- `[sql:]pattern [=> rewrite]` → its parts.
---@param args string
---@return {pattern: string, rewrite: string?, language: "dbt"|"sql"}?
function M.parse(args)
  args = vim.trim(args or "")
  local language = "dbt"
  local rest = args:match("^sql:%s*(.*)$")
  if rest then
    language, args = "sql", rest
  else
    args = args:match("^dbt:%s*(.*)$") or args
  end
  local pattern, rewrite = args:match("^(.-)%s*=>%s*(.*)$")
  pattern = vim.trim(pattern or args)
  if pattern == "" then
    return nil
  end
  if rewrite then
    rewrite = vim.trim(rewrite)
  end
  return { pattern = pattern, rewrite = rewrite, language = language }
end

--- A match as a quickfix item / picker item.
---@param m table one of dbt.astGrep's matches
---@return table
function M.item(m)
  local start = m.range.start
  local text = (m.text or ""):gsub("%s*\n%s*", " ")
  if m.replacement and m.replacement ~= vim.NIL then
    text = text .. "  →  " .. m.replacement:gsub("%s*\n%s*", " ")
  end
  local fname = vim.uri_to_fname(m.uri)
  return {
    filename = fname,
    path = fname,
    uri = m.uri,
    lnum = start.line + 1,
    col = start.character + 1,
    text = text,
  }
end

--- Ask the server; `done(result)` only on an answer.
---@param client vim.lsp.Client
---@param bufnr integer
---@param query {pattern: string, rewrite: string?, language: string}
---@param opts {uri?: string, apply?: boolean}?
---@param done fun(result: table)
function M.request(client, bufnr, query, opts, done)
  opts = opts or {}
  actions.exec_with(client, bufnr, "dbt.astGrep", {
    uri = opts.uri or "",
    pattern = query.pattern,
    rewrite = query.rewrite,
    language = query.language,
    apply = opts.apply or nil,
  }, function(err, result)
    if err or type(result) ~= "table" then
      vim.notify("dbt.astGrep: " .. (err and err.message or "no answer"), vim.log.levels.ERROR)
      return
    end
    done(result)
  end)
end

local function summary(result, query)
  local n = #result.matches
  return ("%d match%s of %s in %d %s file%s%s"):format(
    n,
    n == 1 and "" or "es",
    query.pattern,
    result.files or 0,
    query.language,
    (result.files or 0) == 1 and "" or "s",
    result.truncated and " (the first 500)" or ""
  )
end

--- :DbtAstGrep {args}
---@param opts {args: string}
function M.command(opts)
  local query = M.parse(opts.args)
  if not query then
    vim.notify(
      "usage: :DbtAstGrep [sql:]pattern [=> rewrite]   e.g. :DbtAstGrep " .. M.DEFAULT_PATTERN,
      vim.log.levels.INFO
    )
    return
  end
  local bufnr = vim.api.nvim_get_current_buf()
  local client = actions.client_for(bufnr)
  if not client then
    return
  end
  M.request(client, bufnr, query, nil, function(result)
    local items = vim.tbl_map(M.item, result.matches)
    vim.fn.setqflist({}, " ", { title = "ast-grep: " .. query.pattern, items = items })
    if #items == 0 then
      vim.notify("ast-grep: " .. summary(result, query), vim.log.levels.INFO)
      return
    end
    vim.cmd.copen()
    if not query.rewrite then
      return
    end
    require("cogitator.pick").select({ "apply", "cancel" }, {
      prompt = ("rewrite %d matches into %s?"):format(#items, query.rewrite),
      format_item = function(choice)
        return choice == "apply" and ("apply the %d rewrites (the quickfix list shows them)"):format(#items)
          or "cancel"
      end,
    }, function(choice)
      if choice == "apply" then
        M.request(client, bufnr, query, { apply = true }, function(applied)
          vim.notify("ast-grep: rewrote " .. #applied.matches .. " matches", vim.log.levels.INFO)
        end)
      end
    end)
  end)
end

--- The picker source of the matches of `query` (ctx.query), the list then
--- narrowed by the fuzzy query like any other.
---@param ctx {client: vim.lsp.Client, bufnr: integer, query: table?}
---@return table
function M.source(ctx)
  local query = ctx.query or { pattern = M.DEFAULT_PATTERN, language = "dbt" }
  return {
    title = "ast-grep " .. query.pattern,
    empty = "no match of " .. query.pattern,
    items = function(done)
      M.request(ctx.client, ctx.bufnr, query, nil, function(result)
        done(vim.tbl_map(function(m)
          local item = M.item(m)
          item.text = ("%s:%d  %s"):format(vim.fn.fnamemodify(item.filename, ":~:."), item.lnum, item.text)
          return item
        end, result.matches))
      end)
    end,
    preview = function(item, done)
      require("cogitator.pick.preview").file(item, nil, done)
    end,
  }
end

--- Ask for a pattern (`[sql:]pattern`), then `on_query(query)`.
---@param on_query fun(query: table)
function M.prompt(on_query)
  require("cogitator.picker").input(
    { prompt = "ast-grep pattern ([sql:]pattern): ", default = M.DEFAULT_PATTERN },
    function(text)
      local query = text and M.parse(text)
      if query then
        on_query(query)
      end
    end
  )
end

return M
