--- fzf's search syntax, for the built-in picker and the navigator's `/`.
--- Pure: a query string in, ranked matches with their highlighted
--- character positions out.
---
---   foo bar   every term must match (AND)
---   fzy       fuzzy: the letters in order (vim.fn.matchfuzzypos)
---   'exact    an exact substring
---   ^prefix   starts with
---   suffix$   ends with
---   !term     must not match (combines: !^foo, !'bar)
---   a | b     either term (OR, within one AND group)
---
--- Exact, prefix and suffix terms are smart-case (a capital makes them
--- case-sensitive); fuzzy terms ignore case, as matchfuzzy does.

local M = {}

---@class cogitator.pick.Term
---@field kind "fuzzy"|"exact"|"prefix"|"suffix"
---@field text string
---@field negate boolean

--- One term of the query, or nil for an empty one.
---@param raw string
---@return cogitator.pick.Term?
local function term(raw)
  local negate = false
  if raw:sub(1, 1) == "!" then
    negate, raw = true, raw:sub(2)
  end
  local kind = "fuzzy"
  if raw:sub(1, 1) == "'" then
    kind, raw = "exact", raw:sub(2)
  elseif raw:sub(1, 1) == "^" then
    kind, raw = "prefix", raw:sub(2)
  elseif #raw > 1 and raw:sub(-1) == "$" then
    kind, raw = "suffix", raw:sub(1, -2)
  end
  if raw == "" then
    return nil
  end
  -- a negated fuzzy term would drop nearly everything: fzf treats it as exact
  if negate and kind == "fuzzy" then
    kind = "exact"
  end
  return { kind = kind, text = raw, negate = negate }
end

--- Parse a query into AND groups of OR'd terms.
---@param query string
---@return cogitator.pick.Term[][] groups
function M.parse(query)
  local groups = {}
  local words = vim.split(vim.trim(query or ""), "%s+", { trimempty = true })
  local i = 1
  while i <= #words do
    local group = {}
    local t = term(words[i])
    if t then
      group[1] = t
    end
    while words[i + 1] == "|" and words[i + 2] do
      local alt = term(words[i + 2])
      if alt then
        group[#group + 1] = alt
      end
      i = i + 2
    end
    if #group > 0 then
      groups[#groups + 1] = group
    end
    i = i + 1
  end
  return groups
end

--- Character (not byte) positions, 0-based, of a plain substring match.
---@return integer[]? positions
local function plain(text, t)
  local hay, needle = text, t.text
  if needle:lower() == needle then
    hay = hay:lower()
  end
  local from
  if t.kind == "prefix" then
    from = hay:sub(1, #needle) == needle and 1 or nil
  elseif t.kind == "suffix" then
    from = (#hay >= #needle and hay:sub(-#needle) == needle) and (#hay - #needle + 1) or nil
  else
    from = hay:find(needle, 1, true)
  end
  if not from then
    return nil
  end
  local first = vim.str_utfindex(text, "utf-32", from - 1)
  local count = vim.fn.strchars(needle)
  local out = {}
  for k = 0, count - 1 do
    out[#out + 1] = first + k
  end
  return out
end

--- Does `text` satisfy one term? Its score and positions when it does.
---@return integer? score, integer[]? positions
local function match_term(text, t)
  if t.kind == "fuzzy" then
    local res = vim.fn.matchfuzzypos({ text }, t.text)
    if #res[1] == 0 then
      return nil
    end
    return res[3][1], res[2][1]
  end
  local pos = plain(text, t)
  if not pos then
    return nil
  end
  -- an exact hit ranks above a scattered fuzzy one of the same length
  return 1000 + #t.text * 10 - (pos[1] or 0), pos
end

---@class cogitator.pick.Match
---@field index integer the item's index in the input list
---@field score integer
---@field positions integer[] 0-based character positions to highlight

--- Match one text against parsed groups: nil when it fails a group.
---@param groups cogitator.pick.Term[][]
---@param text string
---@return integer? score, integer[]? positions
function M.match(groups, text)
  local score, positions = 0, {}
  for _, group in ipairs(groups) do
    local best
    for _, t in ipairs(group) do
      local s, pos = match_term(text, t)
      if t.negate then
        if s == nil then
          best = best or { 0, {} }
        end
      elseif s and (not best or s > best[1]) then
        best = { s, pos }
      end
    end
    if not best then
      return nil
    end
    score = score + best[1]
    vim.list_extend(positions, best[2])
  end
  return score, positions
end

--- Rank `texts` by `query`: the matches, best first; ties keep the
--- shorter text first, then the input order. An empty query keeps every
--- item in input order.
---@param texts string[]
---@param query string
---@return cogitator.pick.Match[]
function M.rank(texts, query)
  local groups = M.parse(query)
  local out = {}
  if #groups == 0 then
    for i = 1, #texts do
      out[i] = { index = i, score = 0, positions = {} }
    end
    return out
  end
  -- the first positive fuzzy group narrows the list in C before the rest
  -- run per item: matchfuzzypos over the whole list is fast
  local candidates
  local first = groups[1]
  if #first == 1 and first[1].kind == "fuzzy" and not first[1].negate then
    local items = {}
    for i, text in ipairs(texts) do
      items[i] = { text = text, i = i }
    end
    local res = vim.fn.matchfuzzypos(items, first[1].text, { key = "text" })
    candidates = {}
    for k, item in ipairs(res[1]) do
      candidates[#candidates + 1] = { i = item.i, score = res[3][k], positions = res[2][k] }
    end
    groups = vim.list_slice(groups, 2)
  else
    candidates = {}
    for i = 1, #texts do
      candidates[i] = { i = i, score = 0, positions = {} }
    end
  end
  for _, c in ipairs(candidates) do
    local s, pos = M.match(groups, texts[c.i])
    if s then
      vim.list_extend(c.positions, pos)
      out[#out + 1] = { index = c.i, score = c.score + s, positions = c.positions }
    end
  end
  table.sort(out, function(a, b)
    if a.score ~= b.score then
      return a.score > b.score
    end
    local la, lb = #texts[a.index], #texts[b.index]
    if la ~= lb then
      return la < lb
    end
    return a.index < b.index
  end)
  return out
end

return M
