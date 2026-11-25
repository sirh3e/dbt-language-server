--- The built-in picker's live mode: while the query is something the
--- source evaluates itself (`source.is_live(query)`, a dbt selection),
--- the list is the source's answer to it (`source.live(query, done)`),
--- asked 120 ms after the last key, a stale answer dropped, instead of
--- the fuzzy ranking of its items. Leaving it brings the items back.

local M = {}

M.DEBOUNCE_MS = 120

---@param s cogitator.pick.BuiltinState
---@return boolean
function M.wanted(s)
  local src = s.source
  return src.live ~= nil and src.is_live ~= nil and src.is_live(s.query)
end

--- After the query changed: enter, stay in or leave live mode. Answers
--- true while live (the caller then does not rank).
---@param s cogitator.pick.BuiltinState
---@param render fun(s: cogitator.pick.BuiltinState)
---@return boolean
function M.update(s, render)
  if not M.wanted(s) then
    if s.base then
      s.items, s.texts = s.base.items, s.base.texts
      s.base, s.live_status, s.live_error = nil, nil, nil
      s.marked = {}
    end
    return false
  end
  if not s.base then
    s.base = { items = s.items, texts = s.texts }
    s.marked = {}
  end
  s.live_gen = (s.live_gen or 0) + 1
  local gen = s.live_gen
  s.live_status = "…"
  s.live_timer = s.live_timer or vim.uv.new_timer()
  s.live_timer:stop()
  s.live_timer:start(
    M.DEBOUNCE_MS,
    0,
    vim.schedule_wrap(function()
      if s.closing or s.live_gen ~= gen then
        return
      end
      local asked = s.query
      s.source.live(asked, function(answer)
        if s.closing or s.live_gen ~= gen then
          return
        end
        s.items = answer.items or {}
        s.texts = {}
        s.matches = {}
        for i, item in ipairs(s.items) do
          s.texts[i] = item.text
          s.matches[i] = { index = i, positions = {} }
        end
        s.live_status, s.live_error = answer.status, answer.error
        s.marked = {}
        s.cursor = 1
        render(s)
      end)
    end)
  )
  return true
end

--- The prompt's title while live.
---@param s cogitator.pick.BuiltinState
---@return string?
function M.title(s)
  if not s.base then
    return nil
  end
  local marked = vim.tbl_count(s.marked)
  local mark = marked > 0 and (" · %d marked"):format(marked) or ""
  if s.live_error then
    return ("> %s · ✗ %s"):format(s.source.live_title or "dbt select", s.live_error.message)
  end
  return ("> %s · %s%s"):format(s.source.live_title or "dbt select", s.live_status or "…", mark)
end

local NS = vim.api.nvim_create_namespace("cogitator.pick.live")

--- Underline what an error points at in the prompt.
---@param s cogitator.pick.BuiltinState
function M.mark_error(s)
  local buf = s.bufs.prompt
  if not (buf and vim.api.nvim_buf_is_valid(buf)) then
    return
  end
  vim.api.nvim_buf_clear_namespace(buf, NS, 0, -1)
  local err = s.base and s.live_error
  if err and err.start and err["end"] and err["end"] > err.start then
    vim.api.nvim_buf_set_extmark(buf, NS, 0, err.start, {
      end_col = err["end"],
      hl_group = "DiagnosticUnderlineError",
      strict = false,
    })
  end
end

--- <Tab> while live: complete the word under the cursor.
---@param s cogitator.pick.BuiltinState
---@param set_query fun(text: string)
---@return boolean handled
function M.complete(s, set_query)
  if not (s.base and s.source.complete) then
    return false
  end
  local win = s.wins.prompt
  local col = win and vim.api.nvim_win_is_valid(win) and vim.api.nvim_win_get_cursor(win)[2] or #s.query
  s.source.complete(s.query, col, function(done)
    if s.closing or not done or done.query == s.query then
      if done and #(done.candidates or {}) > 1 then
        vim.notify(table.concat(done.candidates, "  "), vim.log.levels.INFO)
      end
      return
    end
    set_query(done.query)
    if win and vim.api.nvim_win_is_valid(win) then
      pcall(vim.api.nvim_win_set_cursor, win, { 1, done.cursor })
    end
  end)
  return true
end

function M.close(s)
  if s.live_timer then
    s.live_timer:stop()
    s.live_timer:close()
    s.live_timer = nil
  end
end

return M
