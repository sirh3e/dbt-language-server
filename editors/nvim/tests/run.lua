-- Headless test suite: `nvim --headless -l tests/run.lua` (spec §8).
-- Unit tests need only Neovim; the integration half starts a real server
-- (COGITATOR_BIN, default ../../target/debug/cogitator) against
-- testdata/jaffle_shop_duckdb.

local script = vim.uv.fs_realpath(arg[0] or debug.getinfo(1, "S").source:sub(2))
local plugin_dir = vim.fs.dirname(vim.fs.dirname(script))
local repo_root = vim.fs.dirname(vim.fs.dirname(plugin_dir))

vim.opt.runtimepath:prepend(plugin_dir)
vim.cmd.runtime("plugin/cogitator.lua")
-- `nvim -l` starts without filetype detection; vim.lsp.enable attaches on
-- the FileType event, so turn it on for the integration half
vim.cmd("filetype on")

local failures = 0
local function check(ok, label, detail)
  if ok then
    print("ok   " .. label)
  else
    failures = failures + 1
    print("FAIL " .. label .. (detail and (" — " .. detail) or ""))
  end
end

-- -------------------------------------------------------------------------
-- unit: config assembly (spec §3, §1.2)
-- -------------------------------------------------------------------------
local config = require("cogitator.config")

local built = config.build({ tree_sitter = false, debug = true, fusion = "/opt/dbt" })
check(
  vim.deep_equal(built.cmd, { "cogitator", "--no-tree-sitter", "--debug", "--fusion", "/opt/dbt" }),
  "config: flags assembled from options",
  vim.inspect(built.cmd)
)
check(config.build({ fusion = true }).cmd[2] == "--fusion", "config: fusion = true adds bare --fusion")
check(not pcall(config.build, { typo_key = 1 }), "config: unknown key is an error")
check(not pcall(config.build, { lens = { typo = 1 } }), "config: unknown lens key is an error")

-- unit: executeCommand argument shape (spec §1.6)
local buf = vim.api.nvim_create_buf(false, true)
vim.api.nvim_buf_set_name(buf, "/tmp/arg-shape.sql")
vim.api.nvim_set_current_buf(buf)
vim.api.nvim_buf_set_lines(buf, 0, -1, false, { "select 1" })
vim.api.nvim_win_set_cursor(0, { 1, 3 })
local arg_obj = require("cogitator.commands").build_argument(buf, true)
check(
  arg_obj.uri:find("^file://") ~= nil
    and arg_obj.position.line == 0
    and arg_obj.position.character == 3,
  "commands: argument object is { uri, position } (0-based)",
  vim.inspect(arg_obj)
)

-- -------------------------------------------------------------------------
-- integration: real server against the jaffle fixture (spec §8)
-- -------------------------------------------------------------------------
local bin = vim.env.COGITATOR_BIN or vim.fs.joinpath(repo_root, "target", "debug", "cogitator")
if not vim.uv.fs_stat(bin) then
  print("FAIL integration: server binary missing at " .. bin .. " (cargo build -p cogitator)")
  os.exit(1)
end
local fixture = vim.fs.joinpath(repo_root, "testdata", "jaffle_shop_duckdb")

require("cogitator").setup({
  cmd = { bin },
  profiles_dir = vim.fs.joinpath(fixture, ".dbt"),
})

local model = vim.fs.joinpath(fixture, "models", "customers.sql")
vim.cmd.edit(model)
local bufnr = vim.api.nvim_get_current_buf()

local attached = vim.wait(10000, function()
  return #vim.lsp.get_clients({ bufnr = bufnr, name = "cogitator" }) == 1
end, 50)
check(attached, "integration: client attaches inside the dbt project")
if not attached then
  os.exit(1)
end
local client = vim.lsp.get_clients({ bufnr = bufnr, name = "cogitator" })[1]

-- position inside the first ref('…') argument
local text = table.concat(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false), "\n")
local ref_name = text:match("ref%('([%w_]+)'%)")
check(ref_name ~= nil, "integration: fixture model contains a ref()")
local before = text:sub(1, (text:find("ref%('" .. ref_name .. "'%)")) - 1)
local _, newlines = before:gsub("\n", "")
local line_start = before:match("[^\n]*$")
local position = { line = newlines, character = #line_start + 5 } -- inside the name
local doc = { textDocument = { uri = vim.uri_from_bufnr(bufnr) }, position = position }

-- the first context refresh is async: code lenses appearing is the
-- readiness signal (hover contents may legitimately be empty)
local lenses = {}
vim.wait(15000, function()
  local res = vim.lsp.buf_request_sync(bufnr, "textDocument/codeLens", {
    textDocument = { uri = vim.uri_from_bufnr(bufnr) },
  }, 2000)
  local r = res and res[client.id] and res[client.id].result
  if r and r ~= vim.NIL and #r > 0 then
    lenses = r
    return true
  end
  return false
end, 200)
check(#lenses > 0, "integration: context refresh produced code lenses")

-- hover over the ref answers with string contents (Go-exact shape; the
-- string is empty when the model has no description)
local hover_res = vim.lsp.buf_request_sync(bufnr, "textDocument/hover", doc, 5000)
local hover = hover_res and hover_res[client.id] and hover_res[client.id].result
check(
  hover ~= nil and hover ~= vim.NIL and type(hover.contents) == "string",
  "integration: hover over ref('" .. tostring(ref_name) .. "') answers",
  vim.inspect(hover)
)
local titles = vim.tbl_map(function(l)
  return l.command.title
end, lenses)
local function has_title(pattern)
  return vim.iter(titles):any(function(t)
    return t:find(pattern) ~= nil
  end)
end
check(has_title("downstream") and has_title("upstream"), "integration: downstream/upstream lenses", vim.inspect(titles))
check(has_title("dbt run"), "integration: dbt run lens present", vim.inspect(titles))

-- dbt.goToSchema returns a Location for the open model
local schema_result
client:exec_cmd({
  command = "dbt.goToSchema",
  arguments = { { uri = vim.uri_from_bufnr(bufnr), position = { line = 0, character = 0 } } },
}, { bufnr = bufnr }, function(err, result)
  schema_result = err == nil and result or false
end)
vim.wait(5000, function()
  return schema_result ~= nil
end, 50)
check(
  type(schema_result) == "table" and type(schema_result.uri) == "string" and schema_result.uri:find("%.yml$"),
  "integration: dbt.goToSchema returns a schema-YAML Location",
  vim.inspect(schema_result)
)

-- rename returns a WorkspaceEdit whose last documentChange renames the file
local rename_res = vim.lsp.buf_request_sync(
  bufnr,
  "textDocument/rename",
  vim.tbl_extend("force", doc, { newName = "renamed_model" }),
  5000
)
local edit = rename_res and rename_res[client.id] and rename_res[client.id].result
local changes = edit and edit.documentChanges or {}
local last = changes[#changes]
check(
  last ~= nil and last.kind == "rename" and last.newUri:find("renamed_model"),
  "integration: rename ends in a RenameFile documentChange",
  vim.inspect(last)
)

-- dbt.modelGraph: wire shape with direct neighbors
local graph_result
client:exec_cmd({
  command = "dbt.modelGraph",
  arguments = { { uri = vim.uri_from_bufnr(bufnr) } },
}, { bufnr = bufnr }, function(err, result)
  graph_result = err == nil and result or false
end)
vim.wait(5000, function()
  return graph_result ~= nil
end, 50)
local up_names = {}
if type(graph_result) == "table" and graph_result.upstream then
  up_names = vim.tbl_map(function(e)
    return e.name
  end, graph_result.upstream)
end
check(
  type(graph_result) == "table"
    and graph_result.model == "customers"
    and vim.tbl_contains(up_names, "stg_customers"),
  "integration: dbt.modelGraph lists direct upstream",
  vim.inspect(graph_result):sub(1, 200)
)

-- the yazi-style browser: three panes, upstream pane lists neighbors
local graph = require("cogitator.graph")
graph.open()
vim.wait(5000, function()
  return graph._state ~= nil and graph._state.data ~= nil
end, 50)
local s = graph._state
local ui_ok = s ~= nil
  and s.data ~= nil
  and vim.api.nvim_win_is_valid(s.wins.left)
  and vim.api.nvim_win_is_valid(s.wins.center)
  and vim.api.nvim_win_is_valid(s.wins.right)
check(ui_ok, "integration: graph browser opens three panes")
if ui_ok then
  local left = table.concat(vim.api.nvim_buf_get_lines(s.bufs.left, 0, -1, false), "\n")
  check(left:find("stg_customers") ~= nil, "integration: upstream pane lists stg_customers", left)
  local center = table.concat(vim.api.nvim_buf_get_lines(s.bufs.center, 0, -1, false), "\n")
  check(center:find("customers") ~= nil, "integration: center pane names the model", center)
end
graph.close()
check(graph._state == nil, "integration: graph browser closes cleanly")

-- profiles: candidate order and the :DbtProfiles runtime switch
local profiles = require("cogitator.profiles")
local cands = profiles.candidates({ profiles_dir = "/override" }, "/work")
check(
  cands[1] == vim.fs.joinpath("/override", "profiles.yml")
    and vim.tbl_contains(cands, vim.fs.joinpath("/work", ".dbt", "profiles.yml")),
  "profiles: candidates start with the override",
  vim.inspect(cands)
)

-- switch to a genuinely different directory so the restarted client is
-- distinguishable by its initializationOptions
local other_dir = vim.fs.joinpath(vim.uv.os_tmpdir(), "cogitator-nvim-profiles-" .. vim.uv.os_getpid())
vim.fn.mkdir(other_dir, "p")
vim.fn.writefile(
  { "jaffle_shop:", "  target: dev", "  outputs:", "    dev:", "      type: snowflake" },
  vim.fs.joinpath(other_dir, "profiles.yml")
)
profiles.set(other_dir)
-- the dying client can emit a late notification that would interrupt a
-- plain vim.wait, so pump events in small tolerant slices instead
local switched = false
for _ = 1, 150 do
  pcall(vim.wait, 100)
  local c = vim.lsp.get_clients({ bufnr = bufnr, name = "cogitator" })[1]
  if c and c.config.init_options and c.config.init_options.profilesDir == other_dir then
    switched = true
    break
  end
end
check(
  switched,
  "profiles: :DbtProfiles restarts with the new initializationOptions",
  vim.inspect(vim.tbl_map(function(c)
    return c.config.init_options
  end, vim.lsp.get_clients({ name = "cogitator" })))
)

-- the auth manager over the wire: dbt.profiles + dbt.setTarget against the
-- restarted client (its temp profiles dir holds a snowflake jaffle_shop)
local new_client = vim.lsp.get_clients({ bufnr = bufnr, name = "cogitator" })[1]
-- poll: right after the restart the first context refresh may not have
-- landed yet, and without it the project profile (the read scope) is empty
local report
for _ = 1, 75 do
  local pending
  new_client:exec_cmd({
    command = "dbt.profiles",
    arguments = { { uri = vim.uri_from_bufnr(bufnr) } },
  }, { bufnr = bufnr }, function(err, result)
    pending = err == nil and result or false
  end)
  pcall(vim.wait, 2000, function()
    return pending ~= nil
  end, 50)
  if type(pending) == "table" and pending.activeProfile == "jaffle_shop" then
    report = pending
    break
  end
  pcall(vim.wait, 200)
end
local target0 = type(report) == "table"
  and report.profiles
  and report.profiles[1]
  and report.profiles[1].targets
  and report.profiles[1].targets[1]
check(
  type(report) == "table"
    and report.activeProfile == "jaffle_shop"
    and target0
    and target0.type == "snowflake"
    and #target0.auth.issues > 0,
  "profiles: dbt.profiles reports targets with auth issues",
  vim.inspect(report):sub(1, 300)
)

local set_result
new_client:exec_cmd({
  command = "dbt.setTarget",
  arguments = { { target = "dev" } },
}, { bufnr = bufnr }, function(err, result)
  set_result = err == nil and result or false
end)
vim.wait(5000, function()
  return set_result ~= nil
end, 50)
check(
  type(set_result) == "table" and set_result.target == "dev",
  "profiles: dbt.setTarget switches the active target",
  vim.inspect(set_result)
)

-- tests navigation: dbt.modelTests wire shape
local tests_result
new_client:exec_cmd({
  command = "dbt.modelTests",
  arguments = { { uri = vim.uri_from_bufnr(bufnr) } },
}, { bufnr = bufnr }, function(err, result)
  tests_result = err == nil and result or false
end)
pcall(vim.wait, 5000, function()
  return tests_result ~= nil
end, 50)
local has_unique = type(tests_result) == "table"
  and tests_result.model == "customers"
  and vim.iter(tests_result.tests or {}):any(function(t)
    return t.name == "unique" and t.column == "customer_id"
  end)
check(has_unique, "yaml: dbt.modelTests lists unique on customer_id", vim.inspect(tests_result):sub(1, 200))

-- extract-to-version: applyEdit lands in the schema BUFFER (never saved)
new_client:exec_cmd({
  command = "dbt.extractModelYaml",
  arguments = { { uri = vim.uri_from_bufnr(bufnr) } },
}, { bufnr = bufnr })
local schema_buf
pcall(vim.wait, 10000, function()
  for _, b in ipairs(vim.api.nvim_list_bufs()) do
    if vim.api.nvim_buf_is_loaded(b) and vim.api.nvim_buf_get_name(b):find("models/schema%.yml$") then
      local text = table.concat(vim.api.nvim_buf_get_lines(b, 0, -1, false), "\n")
      if text:find("versions:") then
        schema_buf = b
        return true
      end
    end
  end
  return false
end, 100)
check(schema_buf ~= nil, "yaml: extract applies versions: to the schema buffer")
if schema_buf then
  local text = table.concat(vim.api.nvim_buf_get_lines(schema_buf, 0, -1, false), "\n")
  check(text:find("latest_version: 1") ~= nil, "yaml: latest_version added", text:sub(1, 200))
  check(vim.bo[schema_buf].modified, "yaml: schema buffer modified, not written")
  vim.api.nvim_buf_delete(schema_buf, { force = true }) -- discard, keep fixture clean
end

print(failures == 0 and "ALL PASSED" or (failures .. " failure(s)"))
os.exit(failures == 0 and 0 or 1)
