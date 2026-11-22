--- telescope as the picker: a table finder over the items, the generic
--- sorter, a buffer previewer filled from the source, and the shared
--- actions mapped in both modes (multi-select with telescope's <Tab>).

local M = { name = "telescope" }

function M.available()
  return (pcall(require, "telescope.pickers"))
end

function M.open(source, items, run)
  local pickers = require("telescope.pickers")
  local finders = require("telescope.finders")
  local conf = require("telescope.config").values
  local previewers = require("telescope.previewers")
  local tactions = require("telescope.actions")
  local action_state = require("telescope.actions.state")
  local keys = require("cogitator.pick.actions").KEYS

  local function chosen(prompt_bufnr)
    local picker = action_state.get_current_picker(prompt_bufnr)
    local out = {}
    for _, entry in ipairs(picker:get_multi_selection()) do
      out[#out + 1] = entry.value
    end
    if #out == 0 then
      local entry = action_state.get_selected_entry()
      out[1] = entry and entry.value
    end
    return out
  end

  pickers
    .new({}, {
      prompt_title = source.title,
      finder = finders.new_table({
        results = items,
        entry_maker = function(item)
          return { value = item, display = item.text, ordinal = item.text }
        end,
      }),
      sorter = conf.generic_sorter({}),
      previewer = source.preview and previewers.new_buffer_previewer({
        title = "preview",
        define_preview = function(self, entry)
          local buf = self.state.bufnr
          source.preview(entry.value, function(p)
            if vim.api.nvim_buf_is_valid(buf) then
              vim.api.nvim_buf_set_lines(buf, 0, -1, false, p.lines or {})
              vim.bo[buf].filetype = p.filetype or ""
              require("cogitator.tshl").preview(buf, p)
            end
          end)
        end,
      }) or nil,
      attach_mappings = function(prompt_bufnr, map)
        local function bind(action)
          return function()
            local items_now = chosen(prompt_bufnr)
            tactions.close(prompt_bufnr)
            run(action, items_now)
          end
        end
        tactions.select_default:replace(bind("default"))
        for action, lhs in pairs(keys) do
          if action ~= "default" then
            map({ "i", "n" }, lhs, bind(action))
          end
        end
        return true
      end,
    })
    :find()
end

return M
