--- What a picked item can be used for, the same whichever picker showed
--- it: open (in a split, vsplit or tab), show in the navigator, yank its
--- name, dbt run it. Sources whose items are not files bring their own
--- `choose`.

local M = {}

--- The keys every backend binds, by action.
M.KEYS = {
  default = "<CR>",
  split = "<C-x>",
  vsplit = "<C-v>",
  tab = "<C-t>",
  nav = "<C-g>",
  yank = "<C-y>",
  run = "<C-r>",
}

--- fzf's own names for the same keys (fzf-lua's actions table).
M.FZF_KEYS = {
  default = "default",
  split = "ctrl-x",
  vsplit = "ctrl-v",
  tab = "ctrl-t",
  nav = "ctrl-g",
  yank = "ctrl-y",
  run = "ctrl-r",
}

local OPEN = { default = "edit", split = "split", vsplit = "vsplit", tab = "tabedit" }

---@param item table
---@return string? path
local function path_of(item)
  if item.uri and item.uri ~= vim.NIL and item.uri ~= "" then
    return vim.uri_to_fname(item.uri)
  end
  return item.path
end

--- Open the first item the way `action` says; the others become listed
--- buffers, so a multi-pick opens them all.
---@param items table[]
---@param action string
function M.open(items, action)
  local first = items[1]
  local path = first and path_of(first)
  if not path then
    vim.notify("no file on disk for this entry", vim.log.levels.WARN)
    return
  end
  vim.cmd[OPEN[action] or "edit"](vim.fn.fnameescape(path))
  if first.lnum then
    pcall(vim.api.nvim_win_set_cursor, 0, { first.lnum, first.col or 0 })
    vim.cmd("normal! zz")
  end
  for i = 2, #items do
    local more = path_of(items[i])
    if more then
      vim.cmd.badd(vim.fn.fnameescape(more))
    end
  end
end

---@param items table[]
---@return string[]
local function names(items)
  local out = {}
  for _, item in ipairs(items) do
    out[#out + 1] = item.name or item.text
  end
  return out
end

--- Run `action` on the picked items (a source's own `choose` wins).
---@param source table
---@param action string
---@param items table[]
function M.run(source, action, items)
  if #items == 0 then
    return
  end
  if source.choose then
    source.choose(items, action)
    return
  end
  if OPEN[action] then
    M.open(items, action)
  elseif action == "yank" then
    local text = table.concat(names(items), " ")
    vim.fn.setreg('"', text)
    pcall(vim.fn.setreg, "+", text)
    vim.notify("yanked " .. text)
  elseif action == "run" then
    require("cogitator.commands").run_selection({ args = table.concat(names(items), " ") })
  elseif action == "nav" then
    require("cogitator.explorer").open({ focus = items[1].name })
  end
end

return M
