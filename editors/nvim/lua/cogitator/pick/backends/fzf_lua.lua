--- fzf-lua as the picker: each item is a line `<index>\t<text>` (fzf shows
--- the text, the index finds the item back), the preview comes from the
--- source through a previewer of fzf-lua's own kind, and the shared
--- actions are bound to fzf's keys.

local M = { name = "fzf-lua" }

function M.available()
  return (pcall(require, "fzf-lua"))
end

---@param entry string
---@param items table[]
---@return table?
local function item_of(entry, items)
  local index = tonumber((entry or ""):match("^(%d+)\t"))
  return index and items[index]
end

--- A previewer class that asks the source.
local function previewer(source, items)
  local base = require("fzf-lua.previewer.builtin").base
  local P = base:extend()
  function P:new(o, opts, fzf_win)
    P.super.new(self, o, opts, fzf_win)
    setmetatable(self, P)
    return self
  end
  function P.parse_entry(_, entry)
    return item_of(entry, items)
  end
  function P:populate_preview_buf(entry)
    local item = item_of(entry, items)
    if not item then
      return
    end
    local buf = self:get_tmp_buffer()
    self:set_preview_buf(buf)
    source.preview(item, function(p)
      if vim.api.nvim_buf_is_valid(buf) then
        vim.api.nvim_buf_set_lines(buf, 0, -1, false, p.lines or {})
        vim.bo[buf].filetype = p.filetype or ""
        require("cogitator.tshl").preview(buf, p)
      end
    end)
  end
  return P
end

--- An item's text with its colours as ANSI escapes (fzf's --ansi);
--- plain where fzf-lua cannot make them.
---@param item table
---@return string
function M.ansi(item)
  if not item.hls or #item.hls == 0 then
    return item.text
  end
  local ok, utils = pcall(require, "fzf-lua.utils")
  if not ok or type(utils.ansi_from_hl) ~= "function" then
    return item.text
  end
  local out = {}
  for _, seg in ipairs(require("cogitator.pick.paint").segments(item.text, item.hls)) do
    local piece = seg[1]
    if seg[2] then
      local colored = utils.ansi_from_hl(seg[2], piece)
      piece = type(colored) == "string" and colored or piece
    end
    out[#out + 1] = piece
  end
  return table.concat(out)
end

function M.open(source, items, run)
  local fzf = require("fzf-lua")
  local keys = require("cogitator.pick.actions").FZF_KEYS
  local entries = {}
  for i, item in ipairs(items) do
    entries[i] = ("%d\t%s"):format(i, M.ansi(item))
  end
  local fzf_actions = {}
  for action, key in pairs(keys) do
    fzf_actions[key] = function(selected)
      local chosen = {}
      for _, entry in ipairs(selected or {}) do
        chosen[#chosen + 1] = item_of(entry, items)
      end
      run(action, chosen)
    end
  end
  fzf.fzf_exec(entries, {
    prompt = source.title .. "> ",
    fzf_opts = { ["--delimiter"] = "\t", ["--with-nth"] = "2..", ["--multi"] = true, ["--ansi"] = true },
    previewer = source.preview and previewer(source, items) or nil,
    actions = fzf_actions,
  })
end

return M
