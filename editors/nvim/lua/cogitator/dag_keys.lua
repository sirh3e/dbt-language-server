--- :DbtDag keymaps. Movement comes from cogitator.dag_nav; the view hands
--- over everything that touches its state.

local motion = require("cogitator.motion")
local nav = require("cogitator.dag_nav")

local M = {}

--- Install the view's keys on its buffer.
---@param buf integer
---@param view table the operations cogitator.dag exposes
function M.attach(buf, view)
  local function map(lhs, fn, desc)
    motion.map(buf, lhs, fn, desc)
  end
  --- A motion from the current selection, run once per [count].
  local function step(lhs, to, desc)
    map(
      lhs,
      motion.repeated(function()
        view.select(to(view.graph(), view.state().selected))
      end),
      desc
    )
  end
  local function repeated(lhs, fn, desc)
    map(lhs, motion.repeated(fn), desc)
  end

  step("h", function(g, id)
    return nav.neighbour(g, view.state().memory, id, "parent")
  end, "go to a parent")
  step("l", function(g, id)
    return nav.neighbour(g, view.state().memory, id, "child")
  end, "go to a child")
  step("j", function(g, id)
    return nav.in_layer(g, id, 1)
  end, "next node in this layer")
  step("k", function(g, id)
    return nav.in_layer(g, id, -1)
  end, "previous node in this layer")

  map("gg", function()
    view.select(nav.nth(view.graph(), vim.v.count1))
  end, "first node, or node [count]")
  map("G", function()
    local g = view.graph()
    view.select(vim.v.count > 0 and nav.nth(g, vim.v.count) or nav.last(g))
  end, "last node, or node [count]")
  map("{", function()
    view.select(nav.layer_end(view.graph(), view.state().selected, false, vim.v.count1))
  end, "first layer, or layer [count]")
  map("}", function()
    view.select(nav.layer_end(view.graph(), view.state().selected, true, vim.v.count1))
  end, "last layer, or layer [count] from the end")

  map("/", view.search, "search nodes by name")
  repeated("n", function()
    view.next_match(1)
  end, "next search match")
  repeated("N", function()
    view.next_match(-1)
  end, "previous search match")
  repeated("<C-o>", function()
    view.travel(-1)
  end, "older selection")
  repeated("<C-i>", function()
    view.travel(1)
  end, "newer selection")

  map("q", view.close, "close the DAG")
  map("<CR>", view.open_node, "open the model under the cursor")
  map("c", view.toggle_node, "toggle columns of this node")
  map("C", view.toggle_all, "toggle columns everywhere")
  map("L", view.lineage, "column lineage for this model")
  map("R", view.refresh, "refresh the DAG")
end

return M
