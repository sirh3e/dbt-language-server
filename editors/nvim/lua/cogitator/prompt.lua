--- A live single-line prompt read with getcharstr: the caller sees the
--- text change on every keystroke.

local M = {}

local BS = vim.keycode("<BS>")

--- Apply one key to the prompt text. Special keys arrive with a 0x80 lead
--- byte; UTF-8 characters never start with one.
---@param text string
---@param ch string
---@return string text
---@return "edit"|"accept"|"cancel"|"leave" action
function M.step(text, ch)
  if ch == "\27" then
    return "", "cancel"
  elseif ch == "\r" or ch == "\n" then
    return text, "accept"
  elseif ch == BS then
    return vim.fn.strcharpart(text, 0, vim.fn.strchars(text) - 1), "edit"
  elseif ch:byte(1) >= 0x20 and ch:byte(1) ~= 0x7f and ch:byte(1) ~= 0x80 then
    return text .. ch, "edit"
  end
  return text, "leave"
end

--- Run the prompt until the user accepts, cancels or presses another key.
---@param text string the initial text
---@param on_change fun(text: string)
function M.run(text, on_change)
  while true do
    vim.cmd.redraw()
    local ok, ch = pcall(vim.fn.getcharstr)
    local next_text, action = M.step(text, ok and ch or "\27")
    if next_text ~= text then
      text = next_text
      on_change(text)
    end
    if action ~= "edit" then
      return
    end
  end
end

return M
