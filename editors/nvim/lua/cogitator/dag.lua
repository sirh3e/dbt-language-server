--- :DbtDag — the whole project as a dbt-docs-style DAG (Neovim only).
--- Layers flow left→right (sources/roots first); each node is a box with
--- its tags and, when expanded, its documented columns (docglow-style).
--- Keys: <CR> open model, c/C toggle columns, L column lineage, R refresh,
--- q close. The pure layout engine lives in cogitator.layout.

local actions = require("cogitator.actions")
local layout_lib = require("cogitator.layout")

local M = {}

---@class cogitator.dag.State
---@field data table the dbt.projectGraph payload
---@field expanded table<string, boolean> per-node column toggle
---@field all_expanded boolean
---@field boxes table<string, cogitator.layout.Box>
---@field buf integer the cogitator://dag scratch buffer
---@field src_buf integer
---@field client vim.lsp.Client
---@type cogitator.dag.State?
M._state = nil

--- The node whose box the cursor is inside (char-cell hit test).
---@return cogitator.layout.Node?
local function node_at_cursor()
  local s = M._state
  local row, byte_col = unpack(vim.api.nvim_win_get_cursor(0))
  local line = vim.api.nvim_get_current_line()
  local col = vim.fn.charidx(line, math.min(byte_col, #line)) + 1
  for _, box in pairs(s.boxes) do
    if row >= box.y and row <= box.y + box.h - 1 and col >= box.x and col <= box.x + box.w - 1 then
      return box.node
    end
  end
  return nil
end

--- Re-run layout + draw into the scratch buffer.
local function render()
  local s = M._state
  local boxes, width, height = layout_lib.layout(s.data, s.expanded, s.all_expanded)
  s.boxes = boxes
  local lines = layout_lib.draw(boxes, s.data.edges, width + 2, height + 1)
  require("cogitator.ui").set_lines(s.buf, lines)
end

--- <CR>: close the DAG tab and edit the node's file.
local function open_node()
  local node = node_at_cursor()
  if not node then
    return
  end
  if not node.uri or node.uri == vim.NIL then
    vim.notify("no file for " .. node.name, vim.log.levels.WARN)
    return
  end
  vim.cmd.tabclose()
  M._state = nil
  vim.cmd.edit(vim.uri_to_fname(node.uri))
end

--- c: toggle the columns of the node under the cursor.
local function toggle_node()
  local node = node_at_cursor()
  if node then
    M._state.expanded[node.id] = not M._state.expanded[node.id]
    render()
  end
end

--- L: column lineage (message-rendered) for the model under the cursor.
local function lineage_for_node()
  local s = M._state
  local node = node_at_cursor()
  if not (node and node.kind == "model" and node.uri ~= vim.NIL and node.uri) then
    return
  end
  actions.exec_with(s.client, s.src_buf, "dbt.showLineage", { uri = node.uri })
end

function M.close()
  if M._state then
    pcall(vim.cmd.tabclose)
    M._state = nil
  end
end

--- Fetch dbt.projectGraph and open the DAG in a new tab.
function M.open()
  local src_buf = vim.api.nvim_get_current_buf()
  local client = actions.client_for(src_buf)
  if not client then
    return
  end
  actions.exec_with(client, src_buf, "dbt.projectGraph", { uri = vim.uri_from_bufnr(src_buf) }, function(err, data)
    if err or not data or data == vim.NIL or #data.nodes == 0 then
      vim.notify(
        "dbt.projectGraph has nothing to show"
          .. (err and (": " .. err.message) or " (is the project context loaded yet?)"),
        vim.log.levels.WARN
      )
      return
    end
    vim.cmd.tabnew()
    local buf = vim.api.nvim_get_current_buf()
    vim.bo[buf].buftype = "nofile"
    vim.bo[buf].bufhidden = "wipe"
    vim.bo[buf].swapfile = false
    vim.api.nvim_buf_set_name(buf, "cogitator://dag")
    vim.wo.wrap = false

    M._state = {
      data = data,
      expanded = {},
      all_expanded = false,
      boxes = {},
      buf = buf,
      src_buf = src_buf,
      client = client,
    }
    render()

    local function map(lhs, fn, desc)
      vim.keymap.set("n", lhs, fn, { buffer = buf, nowait = true, desc = desc })
    end
    map("q", M.close, "close the DAG")
    map("<CR>", open_node, "open the model under the cursor")
    map("c", toggle_node, "toggle columns of this node")
    map("C", function()
      M._state.all_expanded = not M._state.all_expanded
      render()
    end, "toggle columns everywhere")
    map("L", lineage_for_node, "column lineage for this model")
    map("R", function()
      M.close()
      vim.schedule(M.open)
    end, "refresh the DAG")
  end)
end

return M
