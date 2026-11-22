--- Highlighting from the server (dbt.highlight), painted as extmarks: the
--- Jinja by tree-sitter-dbt, the SQL between the tags by the SQL grammar it
--- injects, YAML by tree-sitter-yaml with the Jinja and SQL inside its
--- values, and the server's dbt-aware captures (models, columns, tests,
--- docs…). Nothing to install in Neovim (specs/highlighting/00-highlighting.md).
---
--- Every layer has a namespace and a priority of its own, so a layer can be
--- turned off and Neovim's own tree-sitter colours show through below the
--- server's. Captures become Neovim's tree-sitter groups (`@keyword`,
--- `@type.dbt.model` → `@type`), so every colourscheme applies.
---
--- Painted: the explorer's SQL panes, picker previews, `:DbtHighlight`, and
--- with `highlight.server = "auto"|"always"` every dbt buffer on open.

local M = {}

M.LAYERS = { "yaml", "sql", "jinja", "dbt" }

--- One namespace per layer.
M.ns = {}
for _, layer in ipairs(M.LAYERS) do
  M.ns[layer] = vim.api.nvim_create_namespace("cogitator.tshl." .. layer)
end

--- Priorities (§7.1): yaml below semantic tokens (125), the rest above.
M.PRIORITY = { yaml = 120, sql = 124, jinja = 126, dbt = 128 }

--- The definition marks of previews and the explorer.
M.def_ns = vim.api.nvim_create_namespace("cogitator.tshl.definition")

--- Above this many lines the visible window is painted first.
M.LARGE = 2000

--- The settings (`highlight` in setup()).
local function settings()
  local cogitator = package.loaded["cogitator"]
  local config = cogitator and cogitator.config or require("cogitator.config").build({})
  return config.highlight
end

--- The highlight group of a capture: Neovim's tree-sitter group of the
--- same name. An undefined `@type.dbt.model` resolves to `@type` by itself,
--- so colourschemes that only define the parent still apply.
---@param capture string
---@return string
function M.group(capture)
  return "@" .. capture
end

--- The grammar a file is coloured with, by its path: YAML for .yml/.yaml,
--- plain SQL under target/compiled or target/run, dbt anywhere else.
---@param path string
---@return "dbt"|"sql"|"yaml"
function M.language_of(path)
  if path:match("%.ya?ml$") then
    return "yaml"
  end
  if path:match("[/\\]target[/\\]compiled[/\\]") or path:match("[/\\]target[/\\]run[/\\]") then
    return "sql"
  end
  return "dbt"
end

--- UTF-16 column → byte column of `line`.
local function byte_col(line, character)
  local ok, col = pcall(vim.str_byteindex, line, "utf-16", character, false)
  return ok and col or math.min(character, #line)
end

--- Clear what apply() painted (every layer, or the given ones).
---@param buf integer
---@param layers string[]?
function M.clear(buf, layers)
  if not vim.api.nvim_buf_is_valid(buf) then
    return
  end
  for _, layer in ipairs(layers or M.LAYERS) do
    vim.api.nvim_buf_clear_namespace(buf, M.ns[layer], 0, -1)
  end
end

--- Every extmark the server's colours put in `buf`, for tests and health.
---@param buf integer
---@return table[] marks {id, row, col, details} with details.layer
function M.marks(buf)
  local out = {}
  for _, layer in ipairs(M.LAYERS) do
    for _, m in ipairs(vim.api.nvim_buf_get_extmarks(buf, M.ns[layer], 0, -1, { details = true })) do
      m[4].layer = layer
      out[#out + 1] = m
    end
  end
  table.sort(out, function(a, b)
    if a[2] == b[2] then
      return a[3] < b[3]
    end
    return a[2] < b[2]
  end)
  return out
end

--- Paint `result` (dbt.highlight's answer) into `buf`, its line 0 at
--- `first` (0-based; header lines above it stay unpainted). With
--- `opts.lines`, only those lines are cleared first (a range answer).
---@param buf integer
---@param result {spans: {range: table, capture: string, layer: string?}[]}
---@param first integer?
---@param opts {skip?: table<string, boolean>, lines?: {[1]: integer, [2]: integer}}?
---@return integer painted
function M.apply(buf, result, first, opts)
  if not vim.api.nvim_buf_is_valid(buf) then
    return 0
  end
  opts = opts or {}
  first = first or 0
  local enabled = (settings() or {}).layers or {}
  for _, layer in ipairs(M.LAYERS) do
    if opts.lines then
      vim.api.nvim_buf_clear_namespace(buf, M.ns[layer], opts.lines[1] + first, opts.lines[2] + first)
    else
      vim.api.nvim_buf_clear_namespace(buf, M.ns[layer], 0, -1)
    end
  end
  local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
  local painted = 0
  for _, span in ipairs(result.spans or {}) do
    local layer = span.layer or "jinja"
    if enabled[layer] ~= false and not (opts.skip or {})[layer] and M.ns[layer] then
      local s, e = span.range.start, span.range["end"]
      local srow, erow = s.line + first, e.line + first
      local sline, eline = lines[srow + 1], lines[erow + 1]
      if sline and eline then
        local ok = pcall(vim.api.nvim_buf_set_extmark, buf, M.ns[layer], srow, byte_col(sline, s.character), {
          end_row = erow,
          end_col = byte_col(eline, e.character),
          hl_group = M.group(span.capture),
          priority = M.PRIORITY[layer],
        })
        if ok then
          painted = painted + 1
        end
      end
    end
  end
  return painted
end

--- A cogitator client: the buffer's, else any running one.
---@param bufnr integer?
---@return vim.lsp.Client?
local function client_of(bufnr)
  local clients = vim.lsp.get_clients({ name = "cogitator", bufnr = bufnr })
  if #clients == 0 then
    clients = vim.lsp.get_clients({ name = "cogitator" })
  end
  return clients[1]
end

--- Ask dbt.highlight for `args` and paint the answer into `buf`. An answer
--- for an older request (the buffer was painted again meanwhile) is
--- dropped. The buffer's regex syntax is turned off once painted (unless
--- `keep_syntax`), so the two do not fight; without an answer it keeps
--- whatever colours it had.
---@param buf integer
---@param args table {uri} | {text, language?} | {node} | {model?, compiled = true}, + range?, version?
---@param opts {first?: integer, client?: vim.lsp.Client, skip?: table, keep_syntax?: boolean, lines?: table, on_done?: fun(painted: integer)}?
function M.paint(buf, args, opts)
  opts = opts or {}
  local client = opts.client or client_of(nil)
  if not client or client:is_stopped() or not vim.api.nvim_buf_is_valid(buf) then
    return
  end
  local gen = vim.b[buf].cogitator_tshl_gen or 0
  -- a range request does not outdate the full one beside it
  if not args.range then
    gen = gen + 1
    vim.b[buf].cogitator_tshl_gen = gen
  end
  client:exec_cmd({ command = "dbt.highlight", arguments = { args } }, { bufnr = buf }, function(err, result)
    if not vim.api.nvim_buf_is_valid(buf) or (vim.b[buf].cogitator_tshl_gen or 0) ~= gen then
      return
    end
    if err or type(result) ~= "table" then
      return
    end
    if not opts.keep_syntax and vim.bo[buf].syntax ~= "OFF" then
      vim.bo[buf].syntax = "OFF"
    end
    local painted = M.apply(buf, result, opts.first, { skip = opts.skip, lines = opts.lines })
    if opts.on_done then
      opts.on_done(painted)
    end
  end)
end

--- Paint the text `lines[first+1..]` of `buf` as `language`.
---@param buf integer
---@param language string "dbt" | "sql" | "yaml"
---@param opts {first?: integer, client?: vim.lsp.Client, skip?: table, keep_syntax?: boolean}?
function M.paint_lines(buf, language, opts)
  opts = opts or {}
  if not vim.api.nvim_buf_is_valid(buf) then
    return
  end
  local lines = vim.api.nvim_buf_get_lines(buf, opts.first or 0, -1, false)
  M.paint(buf, { text = table.concat(lines, "\n"), language = language }, opts)
end

--- Mark lines `from`..`to` (1-based, inclusive) as a definition: a sign
--- and the line's background.
---@param buf integer
---@param from integer?
---@param to integer?
function M.mark_definition(buf, from, to)
  if not vim.api.nvim_buf_is_valid(buf) then
    return
  end
  vim.api.nvim_buf_clear_namespace(buf, M.def_ns, 0, -1)
  if not from then
    return
  end
  local count = vim.api.nvim_buf_line_count(buf)
  for row = from, math.min(to or from, count) do
    pcall(vim.api.nvim_buf_set_extmark, buf, M.def_ns, row - 1, 0, {
      sign_text = "▌",
      sign_hl_group = "CogitatorDefinition",
      line_hl_group = "CogitatorDefinitionLine",
      priority = 10,
    })
  end
end

vim.api.nvim_set_hl(0, "CogitatorDefinition", { link = "Title", default = true })
vim.api.nvim_set_hl(0, "CogitatorDefinitionLine", { link = "CursorLine", default = true })

--- A picker preview: colour it when the preview says it holds dbt, SQL or
--- YAML (`p.code`), below its header lines (`p.first`), and mark the
--- definition's lines (`p.lnum` .. `p.end_lnum`, 1-based in the buffer).
---@param buf integer
---@param p {code?: string, first?: integer, lnum?: integer, end_lnum?: integer}
function M.preview(buf, p)
  if p.code then
    M.paint_lines(buf, p.code, { first = p.first })
  else
    M.clear(buf)
  end
  M.mark_definition(buf, p.end_lnum and p.lnum or nil, p.end_lnum)
end

-- ---------------------------------------------------------------------------
-- highlight on open (§7.1)
-- ---------------------------------------------------------------------------

--- Buffers the server paints, with how: {mode, skip?, keep_syntax?}.
M._painted = {}

--- Is Neovim's own tree-sitter highlighter running on `buf`?
---@param buf integer
local function neovim_highlights(buf)
  local active = vim.treesitter.highlighter and vim.treesitter.highlighter.active or {}
  return active[buf] ~= nil
end

--- Paint `buf` now: all of it, and when it is long its visible window too,
--- which a range answer brings back first.
---@param buf integer
local function repaint(buf)
  local state = M._painted[buf]
  if not state or not vim.api.nvim_buf_is_valid(buf) then
    return
  end
  local client = client_of(buf)
  local language = M.language_of(vim.api.nvim_buf_get_name(buf))
  local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
  local text = table.concat(lines, "\n")
  local version = vim.b[buf].changedtick
  local opts = { client = client, skip = state.skip, keep_syntax = state.keep_syntax }
  M.paint(buf, { text = text, language = language, version = version }, opts)
  if #lines > M.LARGE then
    local win = vim.fn.bufwinid(buf)
    if win ~= -1 then
      local info = vim.fn.getwininfo(win)[1]
      local from, to = math.max(0, info.topline - 101), info.botline + 100
      M.paint(buf, {
        text = text,
        language = language,
        version = version,
        range = { start = { line = from }, ["end"] = { line = to } },
      }, vim.tbl_extend("force", opts, { lines = { from, to } }))
    end
  end
end

--- Start painting `buf` and keep up with its edits (debounced).
---@param buf integer
---@param how {mode: string, skip?: table, keep_syntax?: boolean}
function M.enable(buf, how)
  M._painted[buf] = how
  vim.b[buf].cogitator_tshl = how.mode
  repaint(buf)
  local group = vim.api.nvim_create_augroup("cogitator.tshl." .. buf, { clear = true })
  local timer = vim.uv.new_timer()
  vim.api.nvim_create_autocmd({ "TextChanged", "InsertLeave" }, {
    group = group,
    buffer = buf,
    callback = function()
      timer:stop()
      timer:start(
        150,
        0,
        vim.schedule_wrap(function()
          repaint(buf)
        end)
      )
    end,
  })
  vim.api.nvim_create_autocmd("BufWipeout", {
    group = group,
    buffer = buf,
    callback = function()
      M._painted[buf] = nil
      timer:stop()
      if not timer:is_closing() then
        timer:close()
      end
    end,
  })
end

--- Stop painting `buf` and take the colours off.
---@param buf integer
function M.disable(buf)
  M._painted[buf] = nil
  pcall(vim.api.nvim_del_augroup_by_name, "cogitator.tshl." .. buf)
  if not vim.api.nvim_buf_is_valid(buf) then
    return
  end
  vim.b[buf].cogitator_tshl = nil
  vim.b[buf].cogitator_tshl_gen = (vim.b[buf].cogitator_tshl_gen or 0) + 1
  M.clear(buf)
  if vim.bo[buf].syntax == "OFF" then
    vim.bo[buf].syntax = vim.bo[buf].filetype
  end
end

--- How `buf` would be painted under `mode`, or nil: SQL buffers without
--- Neovim's tree-sitter highlighter, every YAML buffer (its yaml layer left
--- to Neovim's yaml highlighter when that runs) — or everything with
--- "always".
---@param buf integer
---@param mode string|false|nil
---@return {mode: string, skip?: table, keep_syntax?: boolean}?
function M.decide(buf, mode)
  if not mode then
    return nil
  end
  if mode == "always" then
    return { mode = mode }
  end
  local ft = vim.bo[buf].filetype
  if ft == "yaml" then
    if neovim_highlights(buf) then
      return { mode = mode, skip = { yaml = true }, keep_syntax = true }
    end
    return { mode = mode }
  end
  if ft == "sql" and not neovim_highlights(buf) then
    return { mode = mode }
  end
  return nil
end

--- On attach: paint the buffer when the settings say so.
---@param buf integer
function M.attach(buf)
  local how = M.decide(buf, (settings() or {}).server)
  if how and not M._painted[buf] then
    M.enable(buf, how)
  end
end

--- Colour the current buffer, or take the colours off again.
function M.toggle()
  local buf = vim.api.nvim_get_current_buf()
  if M._painted[buf] then
    M.disable(buf)
    return
  end
  if not client_of(buf) then
    vim.notify("cogitator is not running: no server to highlight with", vim.log.levels.WARN)
    return
  end
  M.enable(buf, { mode = "command" })
end

--- :DbtHighlight — colour the current buffer from the server, or take the
--- colours off. :DbtHighlight! {auto|always|off} sets the mode for the
--- session and applies it to every attached buffer.
---@param opts {bang?: boolean, args?: string}?
function M.command(opts)
  opts = opts or {}
  if not opts.bang then
    M.toggle()
    return
  end
  local mode = vim.trim(opts.args or "")
  local value = ({ auto = "auto", always = "always", off = false })[mode]
  if value == nil then
    vim.notify(":DbtHighlight! auto|always|off", vim.log.levels.INFO)
    return
  end
  local cogitator = require("cogitator")
  if cogitator.config then
    cogitator.config.highlight.server = value
  end
  for _, client in ipairs(vim.lsp.get_clients({ name = "cogitator" })) do
    for buf in pairs(client.attached_buffers or {}) do
      local how = M.decide(buf, value)
      local painted = M._painted[buf]
      if how and not (painted and painted.mode == "command") then
        M.enable(buf, how)
      elseif not how and painted and painted.mode ~= "command" then
        M.disable(buf)
      end
    end
  end
  vim.notify("cogitator: server highlighting " .. mode)
end

--- What health reports: the mode, and the buffers the server paints.
---@return string mode, integer[] buffers
function M.status()
  local bufs = {}
  for buf in pairs(M._painted) do
    if vim.api.nvim_buf_is_valid(buf) then
      bufs[#bufs + 1] = buf
    end
  end
  table.sort(bufs)
  local mode = (settings() or {}).server
  return mode == false and "off" or tostring(mode), bufs
end

return M
