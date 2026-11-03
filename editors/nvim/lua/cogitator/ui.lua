--- Shared float/window plumbing for the graph, explorer and DAG views.
--- Pure UI mechanics only — no LSP traffic and no dbt knowledge here.

local M = {}

---@class cogitator.ui.FloatOpts
---@field row integer 0-based editor row of the float's top edge
---@field col integer 0-based editor column of the float's left edge
---@field width integer inner width in cells
---@field height integer inner height in rows
---@field title string rendered centered in the border
---@field cursorline boolean? highlight the cursor line (default false)

--- Open a minimal, rounded, editor-relative float over a fresh scratch
--- buffer (bufhidden=wipe).
---@param opts cogitator.ui.FloatOpts
---@return integer buf the scratch buffer
---@return integer win the floating window
function M.float(opts)
  local buf = vim.api.nvim_create_buf(false, true)
  vim.bo[buf].bufhidden = "wipe"
  local win = vim.api.nvim_open_win(buf, false, {
    relative = "editor",
    row = opts.row,
    col = opts.col,
    width = opts.width,
    height = opts.height,
    style = "minimal",
    border = "rounded",
    title = " " .. opts.title .. " ",
    title_pos = "center",
  })
  vim.wo[win].cursorline = opts.cursorline or false
  return buf, win
end

--- Replace a read-only scratch buffer's content (toggles 'modifiable').
---@param buf integer
---@param lines string[]
function M.set_lines(buf, lines)
  vim.bo[buf].modifiable = true
  vim.api.nvim_buf_set_lines(buf, 0, -1, false, lines)
  vim.bo[buf].modifiable = false
end

--- Retitle a floating window in place.
---@param win integer
---@param title string
function M.retitle(win, title)
  if vim.api.nvim_win_is_valid(win) then
    vim.api.nvim_win_set_config(win, { title = title, title_pos = "center" })
  end
end

---@class cogitator.ui.ViewState
---@field wins table<string, integer> pane name → window
---@field bufs table<string, integer> pane name → buffer
---@field timer uv.uv_timer_t? debounce timer, stopped and closed on close

--- Tear a float set down: timer, windows, buffers — tolerant of panes
--- that already vanished.
---@param state cogitator.ui.ViewState
function M.close_state(state)
  if state.timer then
    state.timer:stop()
    state.timer:close()
  end
  for _, win in pairs(state.wins) do
    if vim.api.nvim_win_is_valid(win) then
      vim.api.nvim_win_close(win, true)
    end
  end
  for _, buf in pairs(state.bufs) do
    if vim.api.nvim_buf_is_valid(buf) then
      vim.api.nvim_buf_delete(buf, { force = true })
    end
  end
end

return M
