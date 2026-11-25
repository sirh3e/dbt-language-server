--- dbt's node-selection syntax in a finder (spec §5.6): telling a
--- selection (`tag:nightly`, `+dim_items`, `a,b`, `stg_*`, `… --exclude x`)
--- from fzf's own syntax (`'x`, `^x`, `x$`, `!x`, `a | b`), splitting off
--- an inline `--exclude`, and completing the word under the cursor from
--- the server's vocabulary (dbt.select's `vocabulary`). Pure: the server
--- evaluates; this only reads the text.

local M = {}

--- The methods dbt has, as a word before `:` names them.
M.METHODS = {
  "tag",
  "path",
  "file",
  "fqn",
  "package",
  "resource_type",
  "config",
  "access",
  "group",
  "source",
  "exposure",
  "metric",
  "semantic_model",
  "test_type",
  "test_name",
  "selector",
  "unit_test",
  "version",
  "state",
  "result",
  "source_status",
  "saved_query",
}

--- Is one whitespace-separated word dbt syntax?
---@param word string
---@return boolean
local function selection_word(word)
  if word == "--exclude" then
    return true
  end
  -- `method:value`, a method dbt has or not (the server then says
  -- which it has); no dbt name holds a `:`
  if word:match("^@?%d*%+?[%a_][%w_]*[%.%w_]*:") then
    return true
  end
  -- graph operators: @x, +x, 2+x, x+, x+1
  if word:match("^@.") or word:match("^%d*%+.") or word:match(".%+%d*$") then
    return true
  end
  return word:find(",", 1, true) ~= nil or word:find("*", 1, true) ~= nil
end

--- Is `query` a dbt selection rather than fzf words? fzf's own operators
--- (`'exact`, `^start`, `end$`, `!not`, `a | b`) keep it fzf.
---@param query string
---@return boolean
function M.is_selection(query)
  local any = false
  for word in query:gmatch("%S+") do
    if word == "|" or word:match("^[!'%^]") or word:match("%$$") then
      return false
    end
    any = any or selection_word(word)
  end
  return any
end

--- The selection and the `--exclude` after it.
---@param query string
---@return string select, string? exclude
function M.split(query)
  local at = query:find("--exclude", 1, true)
  if not at then
    return vim.trim(query), nil
  end
  local exclude = vim.trim(query:sub(at + #"--exclude"))
  return vim.trim(query:sub(1, at - 1)), exclude ~= "" and exclude or nil
end

local function common_prefix(words)
  local prefix = words[1] or ""
  for i = 2, #words do
    local w = words[i]
    local n = 0
    while n < #prefix and n < #w and prefix:byte(n + 1) == w:byte(n + 1) do
      n = n + 1
    end
    prefix = prefix:sub(1, n)
  end
  return prefix
end

--- What the word ending at `cursor` (a 0-based byte column) can become.
--- `vocab` is dbt.select's `vocabulary`: `{methods, values: {method: [..]}}`.
--- A word with a `:` completes its method's values; one without, the
--- methods (as `method:`) and the models' names. Graph operators in
--- front of the word stay.
---@param query string
---@param cursor integer
---@param vocab {methods: string[], values: table<string, string[]>}?
---@return {query: string, cursor: integer, candidates: string[]}
function M.complete(query, cursor, vocab)
  vocab = vocab or { methods = {}, values = {} }
  local before = query:sub(1, cursor)
  local start = (before:match("^.*()[%s,]") or 0) + 1
  local word = before:sub(start)
  local ops = word:match("^@?%d*%+?") or ""
  local typed = word:sub(#ops + 1)
  local method, prefix = typed:match("^([%w_%.]+):(.*)$")
  local offer = {}
  if method then
    for _, v in ipairs((vocab.values or {})[method] or {}) do
      if vim.startswith(v, prefix) then
        offer[#offer + 1] = method .. ":" .. v
      end
    end
  else
    for _, m in ipairs(vocab.methods or {}) do
      if vim.startswith(m, typed) then
        offer[#offer + 1] = m .. ":"
      end
    end
    for _, name in ipairs((vocab.values or {}).fqn or {}) do
      if typed ~= "" and vim.startswith(name, typed) then
        offer[#offer + 1] = name
      end
    end
  end
  table.sort(offer)
  if #offer == 0 then
    return { query = query, cursor = cursor, candidates = {} }
  end
  local done = common_prefix(offer)
  if #offer == 1 and not vim.endswith(done, ":") then
    done = done .. " "
  end
  if #done < #typed then
    done = typed
  end
  local new = query:sub(1, start - 1) .. ops .. done
  return { query = new .. query:sub(cursor + 1), cursor = #new, candidates = offer }
end

--- The words `word` can become, graph operators kept in front (for the
--- command line's completion).
---@param word string
---@param vocab table?
---@return string[]
function M.candidates(word, vocab)
  local ops = word:match("^@?%d*%+?") or ""
  local done = M.complete(word, #word, vocab)
  local out = {}
  for i, c in ipairs(done.candidates) do
    out[i] = ops .. c
  end
  return out
end

--- "7 · 3 models, 2 tests, 2 seeds" from dbt.select's `counts`.
---@param counts table<string, integer>?
---@return string
function M.summary(counts)
  local order = {
    "model",
    "seed",
    "snapshot",
    "source",
    "test",
    "unit_test",
    "analysis",
    "exposure",
    "metric",
    "semantic_model",
  }
  local total, parts = 0, {}
  for _, kind in ipairs(order) do
    local n = (counts or {})[kind]
    if n and n > 0 then
      total = total + n
      local word = kind:gsub("_", " ")
      parts[#parts + 1] = ("%d %s%s"):format(n, word, n == 1 and "" or (word:match("s$") and "es" or "s"))
    end
  end
  if total == 0 then
    return "0"
  end
  return ("%d · %s"):format(total, table.concat(parts, ", "))
end

return M
