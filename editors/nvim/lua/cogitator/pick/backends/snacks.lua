--- snacks.picker as the picker: the items go in as they are (snacks
--- matches on `text`), the preview is filled from the source, and the
--- shared actions become snacks actions on the same keys.

local M = { name = "snacks" }

function M.available()
  local ok, snacks = pcall(require, "snacks")
  return ok and type(snacks) == "table" and snacks.picker ~= nil
end

function M.open(source, items, run)
  local snacks = require("snacks")
  local keys = require("cogitator.pick.actions").KEYS
  local entries = {}
  for i, item in ipairs(items) do
    entries[i] = { idx = i, text = item.text, item = item }
  end
  local function chosen(picker, current)
    local out = {}
    for _, entry in ipairs(picker:selected({ fallback = true })) do
      out[#out + 1] = entry.item
    end
    if #out == 0 and current then
      out[1] = current.item
    end
    return out
  end
  local snack_actions, input_keys = {}, {}
  for action, lhs in pairs(keys) do
    if action ~= "default" then
      local name = "cogitator_" .. action
      snack_actions[name] = function(picker, current)
        local items_now = chosen(picker, current)
        picker:close()
        run(action, items_now)
      end
      input_keys[lhs] = { name, mode = { "i", "n" } }
    end
  end
  snacks.picker.pick({
    title = source.title,
    items = entries,
    -- the item's colours as snacks' segments
    format = function(entry)
      return require("cogitator.pick.paint").segments(entry.text, entry.item.hls)
    end,
    preview = source.preview and function(ctx)
      source.preview(ctx.item.item, function(p)
        if ctx.preview then
          ctx.preview:set_lines(p.lines or {})
          if p.filetype then
            ctx.preview:highlight({ ft = p.filetype })
          end
        end
      end)
    end or "none",
    confirm = function(picker, current)
      local items_now = chosen(picker, current)
      picker:close()
      run("default", items_now)
    end,
    actions = snack_actions,
    win = { input = { keys = input_keys } },
  })
end

return M
