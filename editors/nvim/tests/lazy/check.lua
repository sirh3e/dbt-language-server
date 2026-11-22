-- Runs inside the Neovim run.lua starts on the sandboxed init.lua.

local failures = 0
if vim.env.COGITATOR_LAZY_DEBUG then
  local plugins = require("lazy.core.config").plugins
  for name, p in pairs(plugins) do
    print(
      ("plugin %s installed=%s url=%s lazy=%s loaded=%s config=%s opts=%s main=%s init=%s pkg=%s"):format(
        name,
        tostring(p._.installed),
        tostring(p.url),
        tostring(p.lazy),
        vim.inspect(p._.loaded),
        type(p.config),
        vim.inspect(p.opts),
        tostring(p.main),
        type(p.init),
        vim.inspect(p._.pkg and p._.pkg.source)
      )
    )
  end
end
local function check(ok, label, detail)
  print((ok and "ok   " or "FAIL ") .. label .. ((not ok and detail) and (" — " .. tostring(detail)) or ""))
  if not ok then
    failures = failures + 1
  end
end

-- the example loads on reading a SQL file: read one
if vim.env.COGITATOR_LAZY_SPEC == "example" then
  local was_loaded = require("lazy.core.config").plugins["dbt-language-server"]._.loaded ~= nil
  check(not was_loaded, "lazy: the example does not load the plugin before a SQL file is read")
  local file = vim.env.COGITATOR_SANDBOX .. "/model.sql"
  vim.fn.writefile({ "select 1" }, file)
  vim.cmd.edit(file)
  local example = dofile(vim.fs.dirname(debug.getinfo(1, "S").source:sub(2)) .. "/../../examples/lazy.lua")
  check(
    type(example.build) == "string" and example.build:find("install%.") ~= nil,
    "lazy: the example's build runs the install script"
  )
end

local ok, cogitator = pcall(require, "cogitator")
check(ok, "lazy: require('cogitator') resolves from editors/nvim", not ok and cogitator or nil)
local config = ok and cogitator.config or nil
check(
  config ~= nil and config.keymaps == false and config.inlay_hints == true,
  "lazy: setup() received the spec's opts",
  vim.inspect(config and config.keymaps)
)
check(
  vim.fn.exists(":DbtFind") == 2 or vim.fn.exists(":DbtRun") == 2,
  "lazy: the :Dbt* user commands are defined"
)
check(vim.fn.exists(":DbtBrowse") == 2, "lazy: :DbtBrowse is defined")
check(pcall(vim.cmd.help, "cogitator"), "lazy: :h cogitator finds the plugin's help")
check(vim.lsp.config.cogitator ~= nil, "lazy: the cogitator language server is configured")

if failures > 0 then
  print("messages:\n" .. vim.fn.execute("messages"))
end
print(failures == 0 and "ALL PASSED" or (failures .. " failure(s)"))
vim.cmd(failures == 0 and "qa!" or "cq")
