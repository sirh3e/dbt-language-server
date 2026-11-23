--- The picker's keys: vim motions over the levels, the preview's scroll,
--- the filter prompt, help and closing.

local prompt = require("cogitator.prompt")
local valid = require("cogitator.picker.window").valid

local M = {}

local function scroll_preview(lines)
  local win = require("cogitator.picker")._state.wins.preview
  if valid(win) then
    local key = vim.keycode(lines > 0 and "<C-e>" or "<C-y>")
    vim.api.nvim_win_call(win, function()
      vim.cmd.normal({ args = { math.abs(lines) .. key }, bang = true })
    end)
  end
end

local function map(buf, lhs, fn)
  vim.keymap.set("n", lhs, fn, { buffer = buf, nowait = true, silent = true })
end

--- The picker's keys on its list buffer.
function M.attach(buf)
  local P = require("cogitator.picker")
  local function counted(fn)
    return function()
      fn(vim.v.count1)
    end
  end
  for _, lhs in ipairs({ "j", "<Down>", "<C-n>" }) do
    map(buf, lhs, counted(P.move))
  end
  for _, lhs in ipairs({ "k", "<Up>", "<C-p>" }) do
    map(
      buf,
      lhs,
      counted(function(n)
        P.move(-n)
      end)
    )
  end
  for _, lhs in ipairs({ "l", "<Right>", "<CR>" }) do
    map(buf, lhs, P.enter)
  end
  for _, lhs in ipairs({ "h", "<Left>", "<BS>", "-" }) do
    map(buf, lhs, P.back)
  end
  map(buf, "gg", function()
    P.jump(vim.v.count > 0 and vim.v.count or 1)
  end)
  map(buf, "G", function()
    P.jump(vim.v.count > 0 and vim.v.count or -1)
  end)
  map(buf, "<C-d>", function()
    P.move(math.max(1, math.floor(P._state.geom.height / 2)))
  end)
  map(buf, "<C-u>", function()
    P.move(-math.max(1, math.floor(P._state.geom.height / 2)))
  end)
  map(
    buf,
    "J",
    counted(function(n)
      scroll_preview(5 * n)
    end)
  )
  map(
    buf,
    "K",
    counted(function(n)
      scroll_preview(-5 * n)
    end)
  )
  map(buf, "?", function()
    P._state.help = not P._state.help
    P._render()
  end)
  map(buf, "/", function()
    P.set_filter("")
    prompt.run("", function(text)
      if P._state then
        P.set_filter(text)
      end
    end)
  end)
  map(buf, "q", function()
    P.close()
  end)
  map(buf, "<Esc>", function()
    if P._state and P.level().filter ~= "" then
      P.set_filter("")
    else
      P.close()
    end
  end)
end

return M
