--- The picker's floats: where the five panes go, and opening one.

local M = {}

--- Is `win` a window that still exists?
function M.valid(win)
  return win and vim.api.nvim_win_is_valid(win)
end

--- The panes' sizes and place for a tree of items.
function M.geometry(opts)
  local width = math.min(vim.o.columns - 4, 150)
  local count = #opts.items
  for _, it in ipairs(opts.items) do
    count = math.max(count, #(it.children or {}))
  end
  local height = math.max(8, math.min(vim.o.lines - 8, math.max(count, 14)))
  local parent_w = math.max(14, math.floor(width * 0.2))
  local current_w = math.max(24, math.floor(width * 0.36))
  local preview_w = width - parent_w - current_w - 4
  local row = math.max(0, math.floor((vim.o.lines - height - 5) / 2) - 1)
  local col = math.max(0, math.floor((vim.o.columns - width - 2) / 2))
  return {
    width = width,
    height = height,
    parent_w = parent_w,
    current_w = current_w,
    preview_w = preview_w,
    row = row,
    col = col,
  }
end

--- Open `buf` in a float of the picker's look.
function M.float(buf, cfg, focus)
  local win = vim.api.nvim_open_win(
    buf,
    focus,
    vim.tbl_extend("force", {
      relative = "editor",
      style = "minimal",
      zindex = 60,
    }, cfg)
  )
  vim.wo[win].winhighlight =
    "NormalFloat:DbtPickerNormal,FloatBorder:DbtPickerBorder,CursorLine:DbtPickerSelected"
  vim.wo[win].wrap = false
  return win
end

return M
