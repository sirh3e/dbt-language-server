-- Pure UI units: DAG layout and drawing, highlight choice, prompt editing.

return function(ctx)
  local H = ctx.H
  local layout = require("cogitator.layout")

  local function node(id, layer, name)
    return { id = id, name = name or id, kind = "model", layer = layer, tags = {}, columns = {} }
  end
  local function drawn(data)
    local boxes, width, height = layout.layout(data, {}, false)
    local lines, decor = layout.draw(boxes, data.edges, width + 2, height + 1)
    return boxes, lines, decor
  end

  -- three layers with a stacked middle: the a→z edge skips a layer and
  -- runs into m's box, which it must not corrupt
  local data = {
    nodes = { node("a", 0), node("m", 1), node("n", 1), node("z", 2) },
    edges = { { from = "a", to = "m" }, { from = "a", to = "z" }, { from = "m", to = "z" } },
  }
  local boxes, lines, decor = drawn(data)
  local adjacency = layout.adjacency(data.edges)
  H.check(vim.deep_equal(adjacency.parents.z, { "a", "m" }), "layout: parents", vim.inspect(adjacency))
  H.check(vim.deep_equal(adjacency.children.a, { "m", "z" }), "layout: children", vim.inspect(adjacency))

  local m = boxes.m
  local top = vim.fn.split(lines[m.y], "\\zs")
  H.check(
    top[m.x] == "╭" and top[m.x + m.w - 1] == "╮",
    "canvas: edges leave box corners intact",
    lines[m.y]
  )
  local border_ok = true
  for x = m.x + 1, m.x + m.w - 2 do
    border_ok = border_ok and top[x] == "─"
  end
  H.check(border_ok, "canvas: edges leave box borders intact", lines[m.y])

  -- every recorded edge range covers only edge glyphs (bytes, not cells)
  local edge_glyphs = { ["─"] = 1, ["│"] = 1, ["┼"] = 1, ["▸"] = 1, ["╮"] = 1, ["╯"] = 1, ["╭"] = 1, ["╰"] = 1 }
  local only_edges = true
  for _, ranges in pairs(decor.edges) do
    for _, r in ipairs(ranges) do
      for _, ch in ipairs(vim.fn.split(lines[r[1] + 1]:sub(r[2] + 1, r[3]), "\\zs")) do
        only_edges = only_edges and edge_glyphs[ch] ~= nil
      end
    end
  end
  H.check(only_edges, "canvas: edge ranges are byte-accurate over drawn cells", vim.inspect(decor.edges))

  -- a box's range starts on its ╭ even after multi-byte glyphs on the row
  local r = decor.boxes.z[1]
  H.check(lines[r[1] + 1]:sub(r[2] + 1, r[2] + 3) == "╭", "canvas: box range is byte-accurate", vim.inspect(r))

  local index = layout.index(boxes)
  H.check(layout.box_at(index, m.y, m.x) == m, "layout: box_at hits a box corner")
  H.check(layout.box_at(index, m.y, m.x - 1) == nil, "layout: box_at misses beside a box")
  H.check(#index.layers[2] == 2 and index.layers[2][1].y < index.layers[2][2].y, "layout: layers sorted by row")

  local nav = require("cogitator.dag_nav")
  local history = nav.new_history()
  nav.visit(history, "a")
  nav.visit(history, "b")
  nav.visit(history, "c")
  H.check(nav.travel(history, -1) == "b", "nav: history goes back")
  nav.visit(history, "d")
  H.check(nav.travel(history, 1) == nil, "nav: a new visit drops the forward history")
  H.check(nav.travel(history, -1) == "b" and nav.travel(history, -1) == "a", "nav: history keeps the older visits")
  H.check(nav.travel(history, -1) == nil, "nav: history stops at the oldest")

  local g = { boxes = boxes, index = index, adjacency = adjacency }
  local memory = nav.new_memory()
  H.check(nav.neighbour(g, memory, "a", "parent") == "a", "nav: no parent stays put")
  local child = nav.neighbour(g, memory, "z", "parent")
  H.check(nav.neighbour(g, memory, child, "child") == "z", "nav: child returns where the parent move came from")
  H.check(vim.deep_equal(nav.ordered(g), { "a", "m", "n", "z" }), "nav: reading order", vim.inspect(nav.ordered(g)))
  H.check(nav.next_match(g, { "a", "n" }, "n", 1) == "a", "nav: search wraps forward")
  H.check(nav.next_match(g, { "a", "n" }, "a", -1) == "n", "nav: search wraps back")

  local highlight = require("cogitator.highlight")
  H.check(highlight.group_for({ kind = "source" }) == "DbtDagSource", "highlight: source")
  H.check(highlight.group_for({ kind = "test" }) == "DbtDagModel", "highlight: test uses the default colour")
  H.check(
    highlight.group_for({ kind = "model", uri = "file:///m.py" }) == "DbtDagPython",
    "highlight: python models"
  )
  H.check(highlight.group_for({ kind = "model", uri = vim.NIL }) == "DbtDagModel", "highlight: missing uri")

  local prompt = require("cogitator.prompt")
  H.check(select(1, prompt.step("a", "ä")) == "aä", "prompt: multibyte input appends")
  H.check(select(1, prompt.step("aä", vim.keycode("<BS>"))) == "a", "prompt: backspace removes one character")
  H.check(select(2, prompt.step("a", vim.keycode("<Left>"))) == "leave", "prompt: special keys leave")
  H.check(select(1, prompt.step("a", "\27")) == "", "prompt: <Esc> clears")
end
