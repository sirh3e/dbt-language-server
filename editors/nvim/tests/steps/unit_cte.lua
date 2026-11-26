-- cogitator.preview_cte without a server: the float's size, the CTEs of
-- a documentSymbol answer and the picker's items.

return function(ctx)
  local H = ctx.H
  local cte = require("cogitator.preview_cte")

  local width, height = cte.float_size({ "| a | b |", "|---|---|", "| 1 | 22 |" })
  H.check(width == 10 and height == 3, "cte: the float fits its table", width .. "x" .. height)
  local long = {}
  for i = 1, 500 do
    long[i] = string.rep("x", 1000)
  end
  local w, h = cte.float_size(long)
  H.check(
    w <= math.floor(vim.o.columns * 0.9) and h <= math.floor(vim.o.lines * 0.7),
    "cte: a large table stays inside the editor",
    w .. "x" .. h
  )

  local struct, fn = vim.lsp.protocol.SymbolKind.Struct, vim.lsp.protocol.SymbolKind.Function
  local here, there = "file:///p/models/m.sql", "file:///p/models/other.sql"
  local function sym(name, kind, uri, line)
    return {
      name = name,
      kind = kind,
      location = { uri = uri, range = { start = { line = line, character = 5 } } },
    }
  end
  local ctes = cte.ctes_of({
    sym("renamed", struct, here, 9),
    sym("source", struct, here, 0),
    sym("a_macro", fn, here, 3),
    sym("elsewhere", struct, there, 1),
  }, here)
  H.check(
    #ctes == 3 and ctes[1].name == "source" and ctes[2].name == "renamed" and ctes[3].name == false,
    "cte: this file's CTEs in document order, then the final select",
    vim.inspect(ctes)
  )
  local only = cte.ctes_of(
    { sym("source", struct, here, 0), sym("ordinality", struct, here, 4) },
    here,
    { "source" }
  )
  H.check(
    #only == 2 and only[1].name == "source",
    "cte: a symbol the text defines no CTE of is left out",
    vim.inspect(only)
  )
  local items = cte.items(ctes, function()
    return { lines = {} }
  end)
  local labels = vim.tbl_map(function(i)
    return i.label
  end, items)
  H.check(
    vim.deep_equal(labels, { "source", "renamed", "final select" }),
    "cte: one picker group a CTE",
    vim.inspect(labels)
  )
  local whats = vim.tbl_map(function(i)
    return i.value.what
  end, items[1].children)
  H.check(
    vim.deep_equal(whats, { "float", "split", "sql", "jump" }),
    "cte: a CTE's four actions",
    vim.inspect(whats)
  )
  H.check(#items[3].children == 3, "cte: the final select has no jump")

  local buf, win = cte.show({ table = "| n |\n|---|\n| 1 |\n" }, "float")
  local float = win and vim.api.nvim_win_is_valid(win) and vim.api.nvim_win_get_config(win).relative ~= ""
  H.check(float, "cte: the rows open in a float at the cursor")
  H.check(
    buf and vim.api.nvim_buf_get_lines(buf, 0, -1, false)[1] == "| n |",
    "cte: the float holds the server's table"
  )
  if win and vim.api.nvim_win_is_valid(win) then
    vim.api.nvim_win_close(win, true)
  end

  H.check(
    vim.deep_equal(
      cte.names_in({
        "{{ config(materialized='table') }}",
        "with source as (",
        "  select * from {{ ref('raw') }}  -- not_a as (cte",
        "),",
        "renamed as (select id as (x) from source)",
        "select * from renamed",
      }),
      { "source", "renamed" }
    ),
    "cte: completion reads the CTE names from the text",
    vim.inspect(cte.names_in({ "with source as (", "), renamed as (" }))
  )
  H.check(
    cte.card_lines(nil, false).lines[1] == "the final select of the model",
    "cte: the final select's card"
  )
  H.check(
    cte.card_lines({ contents = "**CTE** `a` · 1 column" }, "a").lines[1] == "**CTE** `a` · 1 column",
    "cte: a card is the hover's text"
  )

  local config = require("cogitator.config")
  H.check(not pcall(config.build, { preview = { cte = "popup" } }), "config: preview.cte is split or float")
  H.check(pcall(config.build, { preview = { cte = "float" } }), "config: preview.cte = float is accepted")
end
