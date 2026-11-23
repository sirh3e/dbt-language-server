-- A smoke test of the plugin against a real dbt project of yours, which
-- stays where it is: `just nvim-real-test path/to/dbt_project` (or
-- COGITATOR_REAL_PROJECT=… nvim --headless -l editors/nvim/tests/real/run.lua).
-- It only reads: no action is applied, nothing is written into the
-- project. For every model it opens the file, asks for the code actions
-- on every `select *` and every CTE name and builds the menu, and opens
-- the navigator on it; every properties file opens the navigator on its
-- first entry. What it checks is what broke on real projects while the
-- fixture suite passed (see testdata/vault_lake/README.md).
--
--   COGITATOR_REAL_PROJECT  the directory holding dbt_project.yml
--   COGITATOR_REAL_LIMIT    at most this many models (default: all)
--   COGITATOR_BIN           the server (default: the cargo debug build)

local script = vim.uv.fs_realpath(arg[0] or debug.getinfo(1, "S").source:sub(2))
local tests_dir = vim.fs.dirname(vim.fs.dirname(script))
local plugin_dir = vim.fs.dirname(tests_dir)
local repo_root = vim.fs.dirname(vim.fs.dirname(plugin_dir))
local project = vim.env.COGITATOR_REAL_PROJECT and vim.fn.fnamemodify(vim.env.COGITATOR_REAL_PROJECT, ":p:h")

if not project or not vim.uv.fs_stat(vim.fs.joinpath(project, "dbt_project.yml")) then
  print("FAIL real: set COGITATOR_REAL_PROJECT to a directory holding dbt_project.yml")
  os.exit(1)
end

vim.opt.runtimepath:prepend(plugin_dir)
vim.cmd.runtime("plugin/cogitator.lua")
vim.cmd("filetype on")
local H = dofile(vim.fs.joinpath(tests_dir, "helpers.lua"))

local bin = H.server_bin(repo_root)
if not vim.uv.fs_stat(bin) then
  print("FAIL real: server binary missing at " .. bin .. " (cargo build -p cogitator)")
  os.exit(1)
end
local tmp = vim.fn.tempname()
vim.fn.mkdir(tmp, "p")
require("cogitator").setup({ cmd = { bin }, manager_path = vim.fs.joinpath(tmp, "manager.json") })

local ca = require("cogitator.code_actions")
local explorer = require("cogitator.explorer")
local errors = {}
local function failed(label, detail)
  errors[#errors + 1] = label .. (detail and (" — " .. tostring(detail)) or "")
  print("FAIL " .. label .. (detail and (" — " .. tostring(detail):sub(1, 300)) or ""))
end

-- every error a callback raises counts, not only the ones a check sees
local notify = vim.notify
vim.notify = function(msg, level, opts)
  if level == vim.log.levels.ERROR then
    failed("real: an error was shown", msg)
  end
  return notify(msg, level, opts)
end

local files = vim.fs.find(function(name)
  return name:match("%.sql$") ~= nil
end, { path = vim.fs.joinpath(project, "models"), type = "file", limit = math.huge })
table.sort(files)
local limit = tonumber(vim.env.COGITATOR_REAL_LIMIT or "")
if limit then
  files = vim.list_slice(files, 1, limit)
end
print(("real: %d models in %s"):format(#files, project))

--- An error raised in a callback (a scheduled render, an LSP handler)
--- reaches no check: it lands in v:errmsg, which each model clears.
local function callback_errors(label)
  local msg = vim.v.errmsg
  if msg ~= "" then
    failed(label .. " raises no error", (msg:gsub("\n.*", "")))
  end
  vim.v.errmsg = ""
end

local client
local started = vim.uv.hrtime()
for index, file in ipairs(files) do
  local name = vim.fn.fnamemodify(file, ":t:r")
  vim.v.errmsg = ""
  vim.cmd.edit(vim.fn.fnameescape(file))
  local bufnr = vim.api.nvim_get_current_buf()
  local attached = vim.wait(20000, function()
    return #vim.lsp.get_clients({ bufnr = bufnr, name = "cogitator" }) == 1
  end, 50)
  if not attached then
    failed("real: " .. name .. " attaches")
    goto next
  end
  client = vim.lsp.get_clients({ bufnr = bufnr, name = "cogitator" })[1]
  if index == 1 and not H.wait_refresh(client, bufnr, 1, 60000) then
    failed("real: the project's first refresh")
    break
  end

  -- the code actions where there is something to do, and the menu built
  do
    local positions = {}
    for row, line in ipairs(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)) do
      local star = line:find("select %*")
      if star then
        positions[#positions + 1] = { row - 1, star - 1 + #"select " }
      end
      local cte = line:find("[%w_]+ as %($")
      if cte then
        positions[#positions + 1] = { row - 1, cte - 1 }
      end
    end
    positions[#positions + 1] = { 0, 0 }
    for _, pos in ipairs(positions) do
      local at = { line = pos[1], character = pos[2] }
      local res = vim.lsp.buf_request_sync(bufnr, "textDocument/codeAction", {
        textDocument = { uri = vim.uri_from_bufnr(bufnr) },
        range = { start = at, ["end"] = at },
        context = { diagnostics = {} },
      }, 15000)
      local entry = res and res[client.id]
      if not entry or entry.err then
        failed(
          ("real: %s code actions at %d:%d"):format(name, pos[1] + 1, pos[2]),
          entry and vim.inspect(entry.err)
        )
      else
        local ok, items = pcall(ca.items, bufnr, entry.result or {}, client.offset_encoding)
        if not ok then
          failed(("real: %s builds its menu at %d:%d"):format(name, pos[1] + 1, pos[2]), items)
        elseif items[1] and items[1].children and #(entry.result or {}) > 0 then
          failed(
            ("real: %s's menu opens on a group at %d:%d"):format(name, pos[1] + 1, pos[2]),
            items[1].label
          )
        end
      end
    end
  end

  -- the navigator on this model
  do
    local ok, err = pcall(explorer.open)
    if not ok then
      failed("real: " .. name .. " opens the explorer", err)
      goto next
    end
    local s = explorer._state
    local loaded = vim.wait(20000, function()
      return s and s.info ~= nil and s.relation ~= nil and s.info.model ~= nil
    end, 50)
    if not loaded then
      failed("real: " .. name .. " loads in the explorer", s and s.selected)
    else
      local card = table.concat(vim.api.nvim_buf_get_lines(s.bufs.center, 0, -1, false), "\n")
      if card:find("doc%(") then
        failed("real: " .. name .. "'s card shows a raw doc() call", card:match("[^\n]*doc%([^\n]*"))
      end
      if card:find("relation: · unresolved ·", 1, true) then
        failed("real: " .. name .. "'s relation resolves")
      end
    end
    explorer.close()
  end

  ::next::
  vim.wait(100, function()
    return false
  end)
  callback_errors("real: " .. name)
  if vim.api.nvim_buf_is_valid(bufnr) then
    pcall(vim.api.nvim_buf_delete, bufnr, { force = true })
  end
end

-- properties files: the navigator opens on the entry under the cursor
for _, yml in
  ipairs(vim.fs.find(function(name)
    return name:match("%.ya?ml$") ~= nil
  end, { path = vim.fs.joinpath(project, "models"), type = "file", limit = math.huge }))
do
  vim.cmd.edit(vim.fn.fnameescape(yml))
  local bufnr = vim.api.nvim_get_current_buf()
  local lines = vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)
  local graph = require("cogitator.pick.graph")
  for row = 1, #lines do
    local entry = graph.yaml_entry(lines, row)
    if entry then
      vim.api.nvim_win_set_cursor(0, { row, 0 })
      vim.wait(20000, function()
        return #vim.lsp.get_clients({ bufnr = bufnr, name = "cogitator" }) == 1
      end, 50)
      local ok, err = pcall(explorer.open)
      local s = explorer._state
      local landed = ok and vim.wait(20000, function()
        return s and s.selected ~= nil
      end, 50)
      local base = vim.fn.fnamemodify(yml, ":t")
      if not landed then
        failed("real: " .. base .. " opens the explorer on " .. entry, err)
      elseif s.selected ~= entry then
        failed("real: " .. base .. " opens on its entry " .. entry, s.selected)
      end
      explorer.close()
      break
    end
  end
  callback_errors("real: " .. vim.fn.fnamemodify(yml, ":t"))
  pcall(vim.api.nvim_buf_delete, bufnr, { force = true })
end

local seconds = (vim.uv.hrtime() - started) / 1e9
vim.fn.delete(tmp, "rf")
if #errors == 0 then
  print(("ALL PASSED: %d models of %s in %.1fs"):format(#files, project, seconds))
  os.exit(0)
end
print(("%d failure(s) in %d models (%.1fs)"):format(#errors, #files, seconds))
os.exit(1)
