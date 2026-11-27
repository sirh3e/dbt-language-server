--- The column-lineage dialog's two overlays: the `/` filter prompt and
--- the `?` help. Both are floats above the dialog that hand focus back to
--- the middle pane when they close.

local motion = require("cogitator.motion")

local M = {}

--- `/`: a one-line prompt over the middle pane's lower border; the list
--- filters as one types (fzf's syntax). <CR> keeps it, <Esc> drops it.
---@param dialog table
function M.prompt(dialog)
  local s = dialog._state
  local before = s.nav.filter
  local cfg = vim.api.nvim_win_get_config(s.wins.center)
  local buf = vim.api.nvim_create_buf(false, true)
  vim.bo[buf].bufhidden = "wipe"
  -- no completion menu or ghost text over a filter (blink.cmp, mini…)
  vim.b[buf].completion = false
  vim.b[buf].minicompletion_disable = true
  vim.api.nvim_buf_set_lines(buf, 0, -1, false, { before })
  local win = vim.api.nvim_open_win(buf, true, {
    relative = "editor",
    row = cfg.row + cfg.height + 1,
    col = cfg.col + 2,
    width = math.max(10, cfg.width - 4),
    height = 1,
    style = "minimal",
    zindex = 60,
  })
  s.prompt = win
  vim.api.nvim_buf_set_extmark(buf, vim.api.nvim_create_namespace("cogitator.column_dialog.prompt"), 0, 0, {
    virt_text = { { "/", "DbtPickerKey" } },
    virt_text_pos = "inline",
    right_gravity = false,
  })
  local function finish(keep)
    if not dialog._state then
      return
    end
    s.prompt = nil
    local typed = vim.api.nvim_buf_is_valid(buf) and (vim.api.nvim_buf_get_lines(buf, 0, 1, false)[1] or "")
      or ""
    if vim.api.nvim_win_is_valid(win) then
      vim.api.nvim_win_close(win, true)
    end
    vim.cmd.stopinsert()
    dialog.filter(keep and typed or before)
    if vim.api.nvim_win_is_valid(s.wins.center) then
      vim.api.nvim_set_current_win(s.wins.center)
    end
  end
  vim.api.nvim_create_autocmd({ "TextChangedI", "TextChanged" }, {
    buffer = buf,
    callback = function()
      if dialog._state == s then
        dialog.filter(vim.api.nvim_buf_get_lines(buf, 0, 1, false)[1] or "")
      end
    end,
  })
  local function imap(lhs, fn)
    vim.keymap.set({ "i", "n" }, lhs, fn, { buffer = buf, nowait = true })
  end
  imap("<CR>", function()
    finish(true)
  end)
  imap("<Esc>", function()
    finish(false)
  end)
  local function step(delta)
    return function()
      vim.api.nvim_win_call(s.wins.center, function()
        vim.cmd("normal! " .. (delta > 0 and "j" or "k"))
      end)
      dialog._sync()
    end
  end
  imap("<C-n>", step(1))
  imap("<Down>", step(1))
  imap("<C-p>", step(-1))
  imap("<Up>", step(-1))
  vim.cmd.startinsert({ bang = true })
end

--- `?`: the keys in a float of their own; any of q, <Esc>, ? closes it.
---@param dialog table
function M.help(dialog, keys)
  local s = dialog._state
  local lines = { " j/k, gg, G, <C-n>/<C-p> … — any motion moves the cursor; the panes follow", "" }
  for _, k in ipairs(keys) do
    lines[#lines + 1] = ("  %-8s %s"):format(k[1], k[2])
  end
  vim.list_extend(lines, {
    "",
    "  rows: ▦ model  ◆ source   ⋈ feeds a join  ƒ derived  = passed through   ↑in ↓out",
  })
  local width = 0
  for _, l in ipairs(lines) do
    width = math.max(width, vim.fn.strdisplaywidth(l) + 2)
  end
  local buf, win = require("cogitator.ui").float({
    row = math.max(0, math.floor((vim.o.lines - #lines) / 2) - 1),
    col = math.max(0, math.floor((vim.o.columns - width) / 2)),
    width = math.min(width, vim.o.columns - 4),
    height = #lines,
    title = "column lineage · keys",
  })
  require("cogitator.ui").set_lines(buf, lines)
  vim.api.nvim_win_set_config(win, { zindex = 70 })
  s.help = win
  vim.api.nvim_set_current_win(win)
  local function close()
    s.help = nil
    if vim.api.nvim_win_is_valid(win) then
      vim.api.nvim_win_close(win, true)
    end
    if dialog._state == s and vim.api.nvim_win_is_valid(s.wins.center) then
      vim.api.nvim_set_current_win(s.wins.center)
    end
  end
  for _, lhs in ipairs({ "q", "<Esc>", "?" }) do
    motion.map(buf, lhs, close, "close the help")
  end
end

return M
