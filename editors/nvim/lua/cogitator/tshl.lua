--- tree-sitter-dbt highlighting from the server (dbt.highlight), painted
--- as extmarks: Jinja by the grammar's own highlights query, the SQL
--- between the tags by the SQL grammar it injects. For buffers Neovim's own
--- parser does not colour — the explorer's dbt SQL and compiled SQL panes,
--- picker previews, any buffer with :DbtHighlight — and it needs nothing
--- installed in Neovim.
---
--- Captures become Neovim's standard tree-sitter groups (`@keyword`,
--- `@tag.delimiter`, `@function.builtin`), so every colourscheme applies.

local M = {}

M.ns = vim.api.nvim_create_namespace("cogitator.tshl")

--- Above regex syntax and semantic tokens (125 is the LSP's default),
--- below diagnostics.
M.PRIORITY = 126

--- The highlight group of a capture: Neovim's tree-sitter group of the
--- same name. An undefined `@keyword.conditional` resolves to `@keyword`
--- by itself, so colourschemes that only define the parent still apply.
---@param capture string
---@return string
function M.group(capture)
  return "@" .. capture
end

--- UTF-16 column → byte column of `line`.
local function byte_col(line, character)
  local ok, col = pcall(vim.str_byteindex, line, "utf-16", character, false)
  return ok and col or math.min(character, #line)
end

--- Paint `result` (dbt.highlight's answer) into `buf`, its line 0 at
--- `first` (0-based; header lines above it stay unpainted).
---@param buf integer
---@param result {spans: {range: table, capture: string}[]}
---@param first integer?
---@return integer painted
function M.apply(buf, result, first)
  if not vim.api.nvim_buf_is_valid(buf) then
    return 0
  end
  first = first or 0
  vim.api.nvim_buf_clear_namespace(buf, M.ns, 0, -1)
  local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
  local painted = 0
  for _, span in ipairs(result.spans or {}) do
    local s, e = span.range.start, span.range["end"]
    local srow, erow = s.line + first, e.line + first
    local sline, eline = lines[srow + 1], lines[erow + 1]
    if sline and eline then
      local ok = pcall(vim.api.nvim_buf_set_extmark, buf, M.ns, srow, byte_col(sline, s.character), {
        end_row = erow,
        end_col = byte_col(eline, e.character),
        hl_group = M.group(span.capture),
        priority = M.PRIORITY,
      })
      if ok then
        painted = painted + 1
      end
    end
  end
  return painted
end

--- Clear what apply() painted.
---@param buf integer
function M.clear(buf)
  if vim.api.nvim_buf_is_valid(buf) then
    vim.api.nvim_buf_clear_namespace(buf, M.ns, 0, -1)
  end
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
--- for an older text (the buffer was painted again meanwhile) is dropped.
--- On success the buffer's regex syntax is turned off so the two do not
--- fight; without an answer it keeps whatever colours it had.
---@param buf integer
---@param args table {uri} | {text, language?} | {model?, compiled = true}
---@param opts {first?: integer, client?: vim.lsp.Client, on_done?: fun(painted: integer)}?
function M.paint(buf, args, opts)
  opts = opts or {}
  local client = opts.client or client_of(nil)
  if not client or client:is_stopped() or not vim.api.nvim_buf_is_valid(buf) then
    return
  end
  local gen = (vim.b[buf].cogitator_tshl_gen or 0) + 1
  vim.b[buf].cogitator_tshl_gen = gen
  client:exec_cmd({ command = "dbt.highlight", arguments = { args } }, { bufnr = buf }, function(err, result)
    if not vim.api.nvim_buf_is_valid(buf) or vim.b[buf].cogitator_tshl_gen ~= gen then
      return
    end
    if err or type(result) ~= "table" then
      return
    end
    if vim.bo[buf].syntax ~= "OFF" then
      vim.bo[buf].syntax = "OFF"
    end
    local painted = M.apply(buf, result, opts.first)
    if opts.on_done then
      opts.on_done(painted)
    end
  end)
end

--- Paint the text `lines[first+1..]` of `buf` as `language` ("dbt" or
--- "sql").
---@param buf integer
---@param language string
---@param opts {first?: integer, client?: vim.lsp.Client}?
function M.paint_lines(buf, language, opts)
  opts = opts or {}
  if not vim.api.nvim_buf_is_valid(buf) then
    return
  end
  local lines = vim.api.nvim_buf_get_lines(buf, opts.first or 0, -1, false)
  M.paint(buf, { text = table.concat(lines, "\n"), language = language }, opts)
end

--- A picker preview: colour it when the preview says it holds dbt or SQL
--- code (`p.code`), below its header lines (`p.first`).
---@param buf integer
---@param p {code?: string, first?: integer}
function M.preview(buf, p)
  if p.code then
    M.paint_lines(buf, p.code, { first = p.first })
  else
    M.clear(buf)
  end
end

--- :DbtHighlight — colour the current buffer with tree-sitter-dbt (from
--- the server), or take the colours off again.
function M.toggle()
  local buf = vim.api.nvim_get_current_buf()
  if vim.b[buf].cogitator_tshl then
    vim.b[buf].cogitator_tshl = nil
    M.clear(buf)
    vim.bo[buf].syntax = vim.bo[buf].filetype
    return
  end
  local client = client_of(buf)
  if not client then
    vim.notify("cogitator is not running: no server to highlight with", vim.log.levels.WARN)
    return
  end
  vim.b[buf].cogitator_tshl = true
  local compiled = vim.api.nvim_buf_get_name(buf):match("[/\\]target[/\\]compiled[/\\]") ~= nil
  M.paint_lines(buf, compiled and "sql" or "dbt", { client = client })
  -- keep up with edits, after a pause
  local group = vim.api.nvim_create_augroup("cogitator.tshl." .. buf, { clear = true })
  local timer = vim.uv.new_timer()
  vim.api.nvim_create_autocmd({ "TextChanged", "InsertLeave" }, {
    group = group,
    buffer = buf,
    callback = function()
      if not vim.b[buf].cogitator_tshl then
        timer:stop()
        timer:close()
        return true
      end
      timer:stop()
      timer:start(
        150,
        0,
        vim.schedule_wrap(function()
          M.paint_lines(buf, compiled and "sql" or "dbt", { client = client })
        end)
      )
    end,
  })
end

return M
