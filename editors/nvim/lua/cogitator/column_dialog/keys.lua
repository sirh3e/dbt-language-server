--- The column-lineage dialog's keys, its filter prompt, its help and its
--- autocmds. Every key routes back through the dialog module, which owns
--- the state; the movement itself is Vim's own (any motion moves the
--- cursor, CursorMoved makes the panes follow).

local actions = require("cogitator.actions")
local locate = require("cogitator.column_dialog.locate")
local motion = require("cogitator.motion")

local M = {}

local GROUP = vim.api.nvim_create_augroup("CogitatorColumnDialog", { clear = true })

local function valid_uri(uri)
  return type(uri) == "string" and uri ~= "" and uri
end

--- The lines of a file: its loaded buffer, else read from disk now.
local function lines_of(uri)
  local fname = vim.uri_to_fname(uri)
  local bufnr = vim.fn.bufnr(fname)
  if bufnr ~= -1 and vim.api.nvim_buf_is_loaded(bufnr) then
    return vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)
  end
  local ok, lines = pcall(vim.fn.readfile, fname)
  return ok and lines or {}
end

--- <CR> / c: close and open the column's file ("raw") or the compiled SQL
--- ("compiled"), the cursor on the column.
---@param dialog table
---@param which "raw"|"compiled"
function M.open_at(dialog, which)
  local s = dialog._state
  local node = dialog.current()
  if not node then
    return
  end
  local column = require("cogitator.column_dialog.model").column_of(node)
  local uri, spot
  if which == "compiled" then
    uri = valid_uri(node.compiledUri)
      or (node.model and (s.compiled_file[node.model] or s.compiled_uri[node.model]))
    if not uri then
      vim.notify(
        "no compiled file for " .. (node.name or "?") .. " — :DbtCompile writes one",
        vim.log.levels.INFO
      )
      return
    end
    spot = locate.column(lines_of(uri), column)
  else
    uri = valid_uri(node.uri)
    if not uri then
      vim.notify("no file on disk for " .. (node.name or "?"), vim.log.levels.WARN)
      return
    end
    local yaml = uri:match("%.ya?ml$") ~= nil
    spot = locate.of_range(node.range) or locate.column(lines_of(uri), column, yaml)
  end
  dialog.close()
  if actions.open_uri(uri) and spot then
    pcall(vim.api.nvim_win_set_cursor, 0, { spot.line, spot.col })
    vim.cmd("normal! zz")
  end
end

--- The keys, as the help lists them: {lhs, description, action}.
---@param dialog table
---@return {[1]: string, [2]: string, [3]: fun()}[]
function M.table(dialog)
  local function scroll(lines)
    return function()
      local s = dialog._state
      motion.scroll(s.wins.raw, lines)
      motion.scroll(s.wins.compiled, lines)
    end
  end
  return {
    {
      "h",
      "walk upstream: what feeds the column becomes the list",
      function()
        dialog.walk("left")
      end,
    },
    {
      "l",
      "walk downstream: what reads the column becomes the list",
      function()
        dialog.walk("right")
      end,
    },
    {
      "<Left>",
      "as h",
      function()
        dialog.walk("left")
      end,
    },
    {
      "<Right>",
      "as l",
      function()
        dialog.walk("right")
      end,
    },
    {
      "<CR>",
      "open the column where it is defined",
      function()
        M.open_at(dialog, "raw")
      end,
    },
    {
      "c",
      "open the compiled SQL at the column",
      function()
        M.open_at(dialog, "compiled")
      end,
    },
    {
      "/",
      "filter the list (fzf syntax: 'exact ^start end$ !not a | b)",
      function()
        require("cogitator.column_dialog.overlays").prompt(dialog)
      end,
    },
    {
      "<Esc>",
      "clear the filter, else close",
      function()
        if dialog._state.nav.filter ~= "" then
          dialog.filter("")
        else
          dialog.close()
        end
      end,
    },
    {
      "q",
      "close",
      function()
        dialog.close()
      end,
    },
    {
      "y",
      "yank model.column",
      function()
        local node = dialog.current()
        if node then
          vim.fn.setreg('"', node.name)
          pcall(vim.fn.setreg, "+", node.name)
          vim.notify("yanked " .. node.name)
        end
      end,
    },
    {
      "D",
      "draw it as the DAG graph instead",
      function()
        dialog.graph()
      end,
    },
    {
      "+",
      "walk one hop deeper",
      function()
        dialog.reload(1)
      end,
    },
    {
      "-",
      "walk one hop shallower",
      function()
        dialog.reload(-1)
      end,
    },
    {
      "R",
      "walk again",
      function()
        dialog.reload()
      end,
    },
    { "<C-d>", "scroll the previews down", scroll(10) },
    { "<C-u>", "scroll the previews up", scroll(-10) },
    {
      "?",
      "this help",
      function()
        require("cogitator.column_dialog.overlays").help(dialog, M.table(dialog))
      end,
    },
  }
end

--- Map the keys on one of the dialog's buffers.
function M.attach(dialog, buf)
  for _, k in ipairs(M.table(dialog)) do
    motion.map(buf, k[1], k[3], k[2])
  end
end

--- The dialog's autocmds: follow the cursor; close once focus leaves for
--- a window that is not the dialog's (looked at a tick later, as WinEnter
--- fires before a new float shows its buffer); follow a resize.
---@param dialog table
---@param s table
function M.watch(dialog, s)
  vim.api.nvim_clear_autocmds({ group = GROUP })
  vim.api.nvim_create_autocmd("CursorMoved", {
    group = GROUP,
    buffer = s.bufs.center,
    callback = function()
      dialog._sync()
    end,
  })
  vim.api.nvim_create_autocmd("WinEnter", {
    group = GROUP,
    callback = function()
      vim.schedule(function()
        if dialog._state ~= s then
          return
        end
        local win = vim.api.nvim_get_current_win()
        if win == s.prompt or win == s.help or vim.tbl_contains(vim.tbl_values(s.wins), win) then
          return
        end
        -- noice / notify floats are no reason to close
        if
          vim.api.nvim_win_get_config(win).relative ~= "" and not vim.api.nvim_win_get_config(win).focusable
        then
          return
        end
        dialog.close()
      end)
    end,
  })
  vim.api.nvim_create_autocmd("VimResized", {
    group = GROUP,
    callback = function()
      if dialog._state ~= s then
        return
      end
      local g = require("cogitator.explorer.render").geometry()
      for name, win in pairs(s.wins) do
        if g[name] and vim.api.nvim_win_is_valid(win) then
          vim.api.nvim_win_set_config(win, vim.tbl_extend("force", { relative = "editor" }, g[name]))
        end
      end
    end,
  })
end

return M
