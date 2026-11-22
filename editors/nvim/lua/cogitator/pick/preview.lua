--- Previews read from disk without blocking: libuv reads the file in the
--- background and hands the lines back on the event loop.

local M = {}

M.MAX_BYTES = 256 * 1024

--- Read up to MAX_BYTES of `path`, then `done(lines | nil)`.
---@param path string
---@param done fun(lines: string[]?)
function M.read(path, done)
  vim.uv.fs_open(path, "r", 438, function(err, fd)
    if err or not fd then
      return vim.schedule(function()
        done(nil)
      end)
    end
    vim.uv.fs_read(fd, M.MAX_BYTES, 0, function(read_err, data)
      vim.uv.fs_close(fd)
      vim.schedule(function()
        if read_err or not data then
          return done(nil)
        end
        done(vim.split(data:gsub("\r\n", "\n"), "\n", { plain = true }))
      end)
    end)
  end)
end

--- The grammar a file is coloured with by the server: YAML for .yml and
--- .yaml, plain SQL under target/compiled or target/run, dbt (Jinja over
--- SQL) for any other .sql; nil for anything else.
---@param path string
---@return "dbt"|"sql"|"yaml"|nil
function M.code_of(path)
  if not (path:match("%.sql$") or path:match("%.ya?ml$")) then
    return nil
  end
  return require("cogitator.tshl").language_of(path)
end

--- The preview of an item that is a file: a header, then the file, the
--- item's line marked.
---@param item table
---@param header string[]?
---@param done fun(preview: {lines: string[], filetype: string?, lnum: integer?, end_lnum: integer?, code: string?, first: integer?})
function M.file(item, header, done)
  local path = item.path
  if not path and item.uri and item.uri ~= vim.NIL and item.uri ~= "" then
    path = vim.uri_to_fname(item.uri)
  end
  header = header or {}
  if not path then
    return done({ lines = #header > 0 and header or { "· no file on disk ·" } })
  end
  M.read(path, function(lines)
    local out = vim.list_extend({}, header)
    if #header > 0 then
      out[#out + 1] = ""
    end
    vim.list_extend(out, lines or { "· cannot read " .. path .. " ·" })
    local ext = path:match("%.(%w+)$")
    local filetypes = { sql = "sql", yml = "yaml", yaml = "yaml", md = "markdown", csv = "csv" }
    local body = #header > 0 and #header + 1 or 0
    done({
      lines = out,
      filetype = filetypes[ext or ""],
      lnum = item.lnum and body + item.lnum or nil,
      -- the definition's last line, when the item knows it (a macro, a test)
      end_lnum = item.end_lnum and body + item.end_lnum or nil,
      -- the server colours .sql with tree-sitter-dbt (compiled SQL as SQL)
      -- and .yml with tree-sitter-yaml, Jinja, SQL and dbt captures
      code = lines and M.code_of(path) or nil,
      first = body,
    })
  end)
end

return M
