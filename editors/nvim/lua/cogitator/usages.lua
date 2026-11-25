--- Where a model, macro or source is used, in the yazi-style picker:
--- :DbtReferences lists every call site of what the cursor names (`!`:
--- the file's own model) grouped by file, each with its line and a
--- preview around it; :DbtMacros lists every macro and, chosen, where it
--- is called. `/` filters both with fzf's syntax, through the files and
--- their lines at once. A dialog's first entry renames what it lists.
---
--- `items`, `preview_lines` and `name_position` are pure, for the suite.

local actions = require("cogitator.actions")

local M = {}

--- Lines either side of a reference in its preview.
M.CONTEXT = { before = 6, after = 14 }

local relative = require("cogitator.paths").relative

local function syntax_of(path)
  if path:match("%.ya?ml$") then
    return "yaml"
  end
  return path:match("%.sql$") and "sql" or nil
end

--- The preview of one use: the lines around it, the used line marked.
---@param lines string[] the file
---@param lnum integer 1-based line of the use
---@return string[]
function M.preview_lines(lines, lnum)
  local first = math.max(1, lnum - M.CONTEXT.before)
  local last = math.min(#lines, lnum + M.CONTEXT.after)
  local out = {}
  for i = first, last do
    out[#out + 1] = (i == lnum and "▶ " or "  ") .. lines[i]
  end
  return out
end

local byte_col = require("cogitator.positions").byte_col

--- Picker items of LSP locations: a group per file (in path order), a
--- line per use in it. `read(path)` gives a file's lines (nil when it
--- cannot be read).
---@param locations lsp.Location[]
---@param opts {root?: string, encoding?: string, read: fun(path: string): string[]?}
---@return cogitator.picker.Item[]
function M.items(locations, opts)
  local by_file, order = {}, {}
  for _, loc in ipairs(locations or {}) do
    local path = vim.uri_to_fname(loc.uri)
    if not by_file[path] then
      by_file[path] = {}
      order[#order + 1] = path
    end
    table.insert(by_file[path], loc.range.start)
  end
  table.sort(order)
  local items = {}
  for _, path in ipairs(order) do
    local lines = opts.read(path) or {}
    local children = {}
    local starts = by_file[path]
    table.sort(starts, function(a, b)
      return a.line < b.line or (a.line == b.line and a.character < b.character)
    end)
    for _, start in ipairs(starts) do
      local lnum = start.line + 1
      local text = lines[lnum]
      children[#children + 1] = {
        label = ("%d: %s"):format(lnum, text and vim.trim(text) or "(the whole file)"),
        value = {
          path = path,
          lnum = lnum,
          col = byte_col(text, start.character, opts.encoding),
        },
        preview = function()
          return { lines = M.preview_lines(lines, lnum), syntax = syntax_of(path) }
        end,
      }
    end
    local name = relative(path, opts.root)
    items[#items + 1] = {
      label = name,
      icon = path:match("%.ya?ml$") and "≡" or "◆",
      icon_hl = path:match("%.ya?ml$") and "DbtPickerSource" or "DbtPickerGroup",
      detail = #children == 1 and "1 use" or (#children .. " uses"),
      children = children,
    }
  end
  return items
end

--- Where `name` is written after `macro` on its header line, as an LSP
--- position (0-based), or nil.
---@param line string
---@param name string
---@return integer? character
function M.name_position(line, name)
  local s = line and line:find("macro%s+" .. vim.pesc(name) .. "%f[^%w_]")
  if not s then
    return nil
  end
  return line:find(name, s, true) - 1
end

local function read_file(path)
  local ok, lines = pcall(vim.fn.readfile, path)
  return ok and lines or nil
end

--- A reader that reads each file once per dialog.
local function cached_reader()
  local cache = {}
  return function(path)
    if cache[path] == nil then
      cache[path] = read_file(path) or false
    end
    return cache[path] or nil
  end
end

--- The subject of a request: what the cursor names, or (`whole`) the
--- file's own model, asked past its last line where nothing local is.
---@return {uri: string, position: table, name: string}
function M.subject(bufnr, whole, encoding)
  local uri = vim.uri_from_bufnr(bufnr)
  if whole then
    local stem = vim.fn.fnamemodify(vim.api.nvim_buf_get_name(bufnr), ":t:r")
    return { uri = uri, position = { line = vim.api.nvim_buf_line_count(bufnr), character = 0 }, name = stem }
  end
  local params = vim.lsp.util.make_position_params(0, encoding or "utf-16")
  local word = vim.fn.expand("<cword>")
  return { uri = uri, position = params.position, name = word ~= "" and word or "this" }
end

--- The references dialog of a subject.
---@param client vim.lsp.Client
---@param bufnr integer
---@param subject {uri: string, position: table, name: string}
function M.show(client, bufnr, subject)
  actions.request(client, bufnr, "textDocument/references", {
    textDocument = { uri = subject.uri },
    position = subject.position,
    context = { includeDeclaration = false },
  }, { timeout_ms = 15000 }, function(err, result, reason)
    if err or reason then
      vim.notify("references: " .. (reason or (err and err.message) or "failed"), vim.log.levels.ERROR)
      return
    end
    if type(result) ~= "table" or #result == 0 then
      vim.notify(("nothing references %s"):format(subject.name), vim.log.levels.INFO)
      return
    end
    local items = M.items(result, {
      root = client.config.root_dir,
      encoding = client.offset_encoding,
      read = cached_reader(),
    })
    table.insert(items, 1, {
      label = ("rename %s…"):format(subject.name),
      icon = "✎",
      value = { rename = true },
      preview = function()
        return {
          "  rename " .. subject.name .. " and every use listed here",
          "",
          "  asks for the new name, then shows the edit before applying it",
        }
      end,
    })
    require("cogitator.picker").open({
      title = ("%s · %d %s"):format(subject.name, #result, #result == 1 and "use" or "uses"),
      items = items,
      on_choose = function(item)
        if item.value and item.value.rename then
          require("cogitator.rename").run(client, bufnr, subject)
          return
        end
        require("cogitator.pick.actions").open({ item.value }, "default")
      end,
    })
  end)
end

--- :DbtReferences[!]: what the cursor names, or with `!` the file's model.
---@param opts {bang?: boolean}?
function M.references(opts)
  local bufnr = vim.api.nvim_get_current_buf()
  local client = actions.client_for(bufnr)
  if not client then
    return
  end
  M.show(client, bufnr, M.subject(bufnr, opts and opts.bang, client.offset_encoding))
end

--- The macro items of workspace/symbol's answer (kind 12), by name.
---@param symbols table[]
---@param root string?
---@return cogitator.picker.Item[]
function M.macro_items(symbols, root)
  local items = {}
  for _, sym in ipairs(symbols or {}) do
    if sym.kind == 12 and sym.location and sym.location.uri then
      local path = vim.uri_to_fname(sym.location.uri)
      items[#items + 1] = {
        label = sym.name,
        icon = "ƒ",
        icon_hl = "DbtPickerRefactor",
        detail = relative(path, root),
        value = sym,
      }
    end
  end
  table.sort(items, function(a, b)
    return a.label < b.label or (a.label == b.label and a.detail < b.detail)
  end)
  return items
end

--- :DbtMacros: every macro (fzf-filtered), then where the chosen one is called.
function M.macros()
  local bufnr = vim.api.nvim_get_current_buf()
  local client = actions.client_for(bufnr)
  if not client then
    return
  end
  local read = cached_reader()
  actions.request(
    client,
    bufnr,
    "workspace/symbol",
    { query = "" },
    { timeout_ms = 15000 },
    function(err, result)
      local items =
        M.macro_items((not err and type(result) == "table") and result or {}, client.config.root_dir)
      if #items == 0 then
        vim.notify("no macros in this project — is the project loaded?", vim.log.levels.INFO)
        return
      end
      require("cogitator.picker").open({
        title = ("macros · %d"):format(#items),
        items = items,
        preview = function(item)
          local sym = item.value
          local path = vim.uri_to_fname(sym.location.uri)
          local lnum = sym.location.range.start.line + 1
          return { lines = M.preview_lines(read(path) or {}, lnum), syntax = syntax_of(path) }
        end,
        on_choose = function(item)
          local sym = item.value
          local path = vim.uri_to_fname(sym.location.uri)
          local start = sym.location.range.start
          local line = (read(path) or {})[start.line + 1]
          local character = M.name_position(line, sym.name) or start.character
          M.show(client, bufnr, {
            uri = sym.location.uri,
            position = { line = start.line, character = character + math.min(1, #sym.name - 1) },
            name = sym.name,
          })
        end,
      })
    end
  )
end

return M
