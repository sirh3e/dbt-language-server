--- Sources from LSP symbols: definitions, macros and selectors
--- (workspace/symbol), and this file's refs, CTEs and macros.

local actions = require("cogitator.actions")
local paint = require("cogitator.pick.paint")
local preview = require("cogitator.pick.preview")

local M = {}

local SYMBOL_KINDS = { [5] = "model", [11] = "source", [12] = "macro", [20] = "selector", [24] = "exposure" }

--- workspace/symbol, all of it, matched here (the server matches substrings).
---@param ctx cogitator.pick.Ctx
---@param title string
---@param keep fun(symbol: table): boolean
local function symbols(ctx, title, keep, query)
  return {
    title = title,
    empty = "no " .. title .. " in this project",
    items = function(done)
      actions.request(
        ctx.client,
        ctx.bufnr,
        "workspace/symbol",
        { query = query or "" },
        { timeout_ms = 15000 },
        function(err, result)
          local items = {}
          for _, sym in ipairs((not err and type(result) == "table") and result or {}) do
            if keep(sym) then
              local loc = sym.location or {}
              local start = loc.range and loc.range.start
              local stop = loc.range and loc.range["end"]
              local kind = SYMBOL_KINDS[sym.kind] or "symbol"
              items[#items + 1] = {
                name = sym.name,
                uri = loc.uri,
                lnum = start and (start.line + 1) or nil,
                -- a macro's whole block, marked in the preview
                end_lnum = stop and (stop.line + 1) or nil,
                col = start and start.character or nil,
                value = sym,
              }
              paint.paint(items[#items], paint.node_parts(sym.name, kind, nil, loc.uri))
              local file = loc.uri and vim.fn.fnamemodify(vim.uri_to_fname(loc.uri), ":~:.")
              if file then
                local item = items[#items]
                local more_text, more = paint.line({ { "  " }, { file, "DbtPickPath" } })
                for _, span in ipairs(more) do
                  item.hls[#item.hls + 1] = { span[1] + #item.text, span[2] + #item.text, span[3] }
                end
                item.text = item.text .. more_text
              end
            end
          end
          done(items)
        end
      )
    end,
    preview = function(item, done)
      preview.file(item, nil, done)
    end,
  }
end

--- Every model, source, macro and selector: jump to its definition.
function M.definitions(ctx)
  return symbols(ctx, "definitions", function()
    return true
  end)
end

function M.macros(ctx)
  return symbols(ctx, "macros", function(sym)
    return sym.kind == 12
  end)
end

function M.selectors(ctx)
  return symbols(ctx, "selectors", function()
    return true
  end, "selector:")
end

--- The ref()/source() calls and the CTEs and macros of the buffer: go to
--- what each one names.
function M.refs(ctx)
  local bufnr = ctx.bufnr
  return {
    title = "references in this file",
    empty = "no ref(), source() or CTE in this file",
    items = function(done)
      local items = {}
      local lines = vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)
      local patterns = {
        { "ref%(%s*['\"]([^'\"]+)['\"]", "ref" },
        { "source%(%s*['\"]([^'\"]+)['\"]%s*,%s*['\"]([^'\"]+)['\"]", "source" },
      }
      for lnum, line in ipairs(lines) do
        for _, p in ipairs(patterns) do
          local init = 1
          while true do
            local s, _, a, b = line:find(p[1], init)
            if not s then
              break
            end
            local name = b and (a .. "." .. b) or a
            items[#items + 1] = {
              text = ("%s(%s)  :%d"):format(p[2], name, lnum),
              name = b or a,
              lnum = lnum,
              col = (line:find(a, s, true) or s) - 1,
              value = { kind = p[2] },
            }
            init = s + 1
          end
        end
      end
      actions.request(ctx.client, bufnr, "textDocument/documentSymbol", {
        textDocument = { uri = vim.uri_from_bufnr(bufnr) },
      }, { timeout_ms = 5000 }, function(err, result)
        for _, sym in ipairs((not err and type(result) == "table") and result or {}) do
          local range = (sym.location or sym).range
          items[#items + 1] = {
            text = ("%s  [%s]  :%d"):format(
              sym.name,
              sym.kind == 12 and "macro" or "cte",
              range.start.line + 1
            ),
            name = sym.name,
            lnum = range.start.line + 1,
            col = range.start.character,
            value = { kind = "local" },
          }
        end
        table.sort(items, function(a, b)
          return a.lnum < b.lnum
        end)
        done(items)
      end)
    end,
    preview = function(item, done)
      local lines = vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)
      local sql = vim.bo[bufnr].filetype == "sql"
      done({
        lines = lines,
        filetype = vim.bo[bufnr].filetype,
        lnum = item.lnum,
        code = sql and require("cogitator.pick.preview").code_of(vim.api.nvim_buf_get_name(bufnr)) or nil,
      })
    end,
    choose = function(items, action)
      local item = items[1]
      local win = vim.fn.bufwinid(bufnr)
      if win == -1 then
        return
      end
      vim.api.nvim_set_current_win(win)
      pcall(vim.api.nvim_win_set_cursor, win, { item.lnum, item.col or 0 })
      if item.value.kind == "local" then
        return
      end
      if action == "default" then
        vim.lsp.buf.definition()
      elseif action == "split" or action == "vsplit" or action == "tab" then
        vim.cmd(({ split = "split", vsplit = "vsplit", tab = "tab split" })[action])
        vim.lsp.buf.definition()
      elseif action == "nav" then
        require("cogitator.explorer").open({ focus = item.name })
      end
    end,
  }
end

return M
