--- The picker as the plugin's dialogs: drop-ins for vim.ui.select (a
--- list) and vim.ui.input (a one-line prompt).

local window = require("cogitator.picker.window")

local M = {}

local function enabled()
  local config = package.loaded["cogitator"] and require("cogitator").config
  return not (config and config.ui and config.ui.picker == false)
end

--- A drop-in for vim.ui.select. Extra opts: `detail(item) -> string`,
--- `preview(item) -> lines` and `icon(item) -> string`.
---@param items any[]
---@param opts {prompt?: string, format_item?: fun(item: any): string, detail?: fun(item: any): string?, preview?: fun(item: any): any, icon?: fun(item: any): string?}
---@param on_choice fun(item: any?, idx: integer?)
function M.select(items, opts, on_choice)
  opts = opts or {}
  if not enabled() then
    return require("cogitator.pick").select(items, opts, on_choice)
  end
  if #items == 0 then
    on_choice(nil, nil)
    return
  end
  local format = opts.format_item or tostring
  local entries = {}
  for i, value in ipairs(items) do
    entries[i] = {
      label = format(value),
      detail = opts.detail and opts.detail(value) or nil,
      icon = opts.icon and opts.icon(value) or nil,
      value = value,
      index = i,
      preview = opts.preview and function()
        return opts.preview(value)
      end or nil,
    }
  end
  require("cogitator.picker").open({
    title = (opts.prompt or "select"):gsub("[:%s]+$", ""),
    items = entries,
    on_choose = function(item)
      on_choice(item.value, item.index)
    end,
    on_cancel = function()
      on_choice(nil, nil)
    end,
  })
end

--- A drop-in for vim.ui.input: a one-line float under a titled border.
--- <CR> accepts; <Esc> leaves insert mode, then <Esc> or q cancels;
--- <Tab> completes when `completion` is set (e.g. "dir").
---@param opts {prompt?: string, default?: string, completion?: string}
---@param on_confirm fun(text: string?)
function M.input(opts, on_confirm)
  opts = opts or {}
  if not enabled() then
    return vim.ui.input(opts, on_confirm)
  end
  local prev = vim.api.nvim_get_current_win()
  local buf = vim.api.nvim_create_buf(false, true)
  vim.bo[buf].bufhidden = "wipe"
  vim.bo[buf].filetype = "cogitator_input"
  vim.api.nvim_buf_set_lines(buf, 0, -1, false, { opts.default or "" })
  local title = (opts.prompt or "input"):gsub("[:%s]+$", "")
  local width = math.max(40, math.min(vim.o.columns - 8, math.max(60, #title + 8, #(opts.default or "") + 4)))
  local win = window.float(buf, {
    row = math.floor((vim.o.lines - 3) / 2) - 1,
    col = math.floor((vim.o.columns - width - 2) / 2),
    width = width,
    height = 1,
    border = "rounded",
    title = " " .. title .. " ",
    title_pos = "left",
    footer = " <CR> accept · <Esc> normal · q cancel" .. (opts.completion and " · <Tab> complete " or " "),
    footer_pos = "right",
  }, true)
  local done = false
  local function finish(text)
    if done then
      return
    end
    done = true
    vim.cmd.stopinsert()
    if vim.api.nvim_win_is_valid(win) then
      vim.api.nvim_win_close(win, true)
    end
    if vim.api.nvim_win_is_valid(prev) then
      vim.api.nvim_set_current_win(prev)
    end
    on_confirm(text)
  end
  local function accept()
    finish(vim.api.nvim_buf_get_lines(buf, 0, 1, false)[1] or "")
  end
  local o = { buffer = buf, nowait = true, silent = true }
  vim.keymap.set({ "n", "i" }, "<CR>", accept, o)
  vim.keymap.set("n", "<Esc>", function()
    finish(nil)
  end, o)
  vim.keymap.set("n", "q", function()
    finish(nil)
  end, o)
  vim.keymap.set("i", "<C-c>", function()
    finish(nil)
  end, o)
  if opts.completion then
    vim.keymap.set("i", "<Tab>", function()
      local line = vim.api.nvim_get_current_line()
      local matches = vim.fn.getcompletion(line, opts.completion)
      if #matches > 0 then
        vim.fn.complete(1, matches)
      end
    end, o)
  end
  vim.api.nvim_create_autocmd("WinLeave", {
    buffer = buf,
    once = true,
    callback = function()
      vim.schedule(function()
        finish(nil)
      end)
    end,
  })
  vim.cmd.startinsert({ bang = true })
end

return M
