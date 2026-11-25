--- Paths as the plugin shows them: below the project root when it is
--- known, else relative to the cwd or home.

local M = {}

--- `path` for display: below `root` when inside it, else `:~:.`.
---@param path string
---@param root string?
---@return string
function M.relative(path, root)
  if root and root ~= "" and path:sub(1, #root + 1) == root .. "/" then
    return path:sub(#root + 2)
  end
  return vim.fn.fnamemodify(path, ":~:.")
end

--- The file behind a uri, for display.
---@param uri string
---@param root string?
---@return string
function M.relative_uri(uri, root)
  return M.relative(vim.uri_to_fname(uri), root)
end

return M
