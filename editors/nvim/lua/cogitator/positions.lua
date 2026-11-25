--- LSP positions on the editor's side.

local M = {}

--- The byte column of LSP `character` (an `encoding` offset, utf-16 by
--- default) on `line`; past the end of the line, its length.
---@param line string?
---@param character integer
---@param encoding string?
---@return integer
function M.byte_col(line, character, encoding)
  if not line or character == 0 then
    return character
  end
  local ok, col = pcall(vim.str_byteindex, line, encoding or "utf-16", character, false)
  return ok and col or math.min(character, #line)
end

return M
