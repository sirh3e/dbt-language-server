-- :DbtDag navigation and colour with real keypresses. Runs inside
-- 05_profiles with the DAG already open.

return function(ctx, dag)
  local H = ctx.H
  local nav = require("cogitator.dag_nav")
  local s = dag._state
  local g = function()
    return { boxes = s.boxes, index = s.index, adjacency = s.adjacency }
  end
  local function node_named(name)
    for id, box in pairs(s.boxes) do
      if box.node.name == name then
        return id
      end
    end
  end
  local function select_named(name)
    s.selected = nil
    local box = s.boxes[node_named(name)]
    local row = box.y + 1
    local line = vim.api.nvim_buf_get_lines(s.buf, row - 1, row, false)[1]
    vim.api.nvim_win_set_cursor(s.win, { row, vim.fn.byteidx(line, box.x + 1) })
    vim.api.nvim_exec_autocmds("CursorMoved", { buffer = s.buf })
    H.check(s.selected == box.node.id, "dag motions: selection follows the cursor to " .. name, s.selected)
  end
  local function cursor_on_selection(label)
    local box = s.boxes[s.selected]
    local row = vim.api.nvim_win_get_cursor(s.win)[1]
    H.check(row == box.y + 1, label, ("row %d, box y %d"):format(row, box.y))
  end

  H.check(s.selected ~= nil, "dag motions: a node starts selected")

  -- orders has several parents and customer_orders as a child
  select_named("orders")
  local orders = s.selected
  H.feed("l")
  H.check(s.boxes[s.selected].node.name == "customer_orders", "dag motions: l follows an edge to a child", s.selected)
  cursor_on_selection("dag motions: the cursor sits on the selected box")
  H.feed("h")
  H.check(s.selected == orders, "dag motions: h returns along the path it came", s.selected)
  local parents = s.adjacency.parents[orders]
  H.feed("h")
  H.check(vim.tbl_contains(parents, s.selected), "dag motions: h follows an edge to a parent", s.selected)

  -- within a layer, with counts
  local layer = s.index.layers[1]
  select_named(layer[1].node.name)
  H.feed("j")
  H.check(s.selected == layer[2].node.id, "dag motions: j moves down the layer", s.selected)
  H.feed("k")
  H.check(s.selected == layer[1].node.id, "dag motions: k moves up the layer", s.selected)
  H.feed("2j")
  H.check(s.selected == layer[3].node.id, "dag motions: counts repeat j", s.selected)
  H.feed("k")

  -- ends
  H.feed("G")
  H.check(s.selected == nav.last(g()), "dag motions: G goes to the last node", s.selected)
  H.feed("gg")
  H.check(s.selected == nav.nth(g(), 1), "dag motions: gg goes to the first node", s.selected)
  H.feed("3gg")
  H.check(s.selected == nav.nth(g(), 3), "dag motions: [count]gg goes to that node", s.selected)
  H.feed("}")
  local last_layer = table.maxn(s.index.layers)
  H.check(s.boxes[s.selected].layer == last_layer, "dag motions: } goes to the last layer", s.selected)
  H.feed("{")
  H.check(s.boxes[s.selected].layer == 1, "dag motions: { goes to the first layer", s.selected)

  -- search, and history
  H.feed("gg")
  local start = s.selected
  H.feed("/stg_pay<CR>")
  H.check(s.boxes[s.selected].node.name == "stg_payments", "dag motions: / finds a node by name", s.selected)
  H.feed("<C-o>")
  H.check(s.selected == start, "dag motions: <C-o> goes back", s.selected)
  H.feed("<C-i>")
  H.check(s.boxes[s.selected].node.name == "stg_payments", "dag motions: <C-i> goes forward", s.selected)
  H.feed("/stg_<CR>")
  local first_match = s.selected
  H.feed("n")
  H.check(s.selected ~= first_match, "dag motions: n steps to another match", s.selected)
  H.feed("N")
  H.check(s.selected == first_match, "dag motions: N steps back", s.selected)
  local before = s.selected
  H.feed("/nothing-matches-this<Esc>")
  H.check(s.selected == before, "dag motions: <Esc> returns to where the search began", s.selected)

  -- re-layout keeps the selection under the cursor
  select_named("orders")
  H.feed("c")
  H.check(s.selected == orders, "dag motions: c keeps the selection", s.selected)
  cursor_on_selection("dag motions: c re-anchors the cursor")
  H.feed("c")

  -- colours
  local paint = require("cogitator.dag_paint")
  local groups = {}
  for _, m in ipairs(vim.api.nvim_buf_get_extmarks(s.buf, paint.STATIC_NS, 0, -1, { details = true })) do
    groups[m[4].hl_group] = true
  end
  H.check(groups.DbtDagModel and groups.DbtDagEdge, "dag motions: models and edges are coloured", vim.inspect(groups))
  local picked = {}
  for _, m in ipairs(vim.api.nvim_buf_get_extmarks(s.buf, paint.SELECTION_NS, 0, -1, { details = true })) do
    picked[m[4].hl_group] = true
  end
  H.check(picked.DbtDagSelected and picked.DbtDagSelectedEdge, "dag motions: the selection is coloured", vim.inspect(picked))
  H.check(vim.api.nvim_get_hl(0, { name = "DbtDagModel" }).bg == 0x0094b3, "dag motions: the model colour is dbt-docs'")
  vim.cmd.colorscheme("default")
  H.check(vim.api.nvim_get_hl(0, { name = "DbtDagSource" }).bg == 0x5fb825, "dag motions: colours survive :colorscheme")
end
