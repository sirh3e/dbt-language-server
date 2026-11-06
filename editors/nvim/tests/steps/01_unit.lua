-- Unit half: config assembly, executeCommand argument shape and the
-- manager store — no server involved (spec §3, §1.2, §1.6).

return function(ctx)
  local H = ctx.H

  local config = require("cogitator.config")
  local built = config.build({ tree_sitter = false, debug = true, fusion = "/opt/dbt" })
  H.check(
    vim.deep_equal(built.cmd, { "cogitator", "--no-tree-sitter", "--debug", "--fusion", "/opt/dbt" }),
    "config: flags assembled from options",
    vim.inspect(built.cmd)
  )
  H.check(config.build({ fusion = true }).cmd[2] == "--fusion", "config: fusion = true adds bare --fusion")
  H.check(not pcall(config.build, { typo_key = 1 }), "config: unknown key is an error")
  H.check(not pcall(config.build, { lens = { typo = 1 } }), "config: unknown lens key is an error")

  -- executeCommand argument shape ({ uri, position }, 0-based)
  local probe = vim.fs.joinpath(H.tmp, "arg-shape.sql")
  local buf = vim.api.nvim_create_buf(false, true)
  vim.api.nvim_buf_set_name(buf, probe)
  vim.api.nvim_set_current_buf(buf)
  vim.api.nvim_buf_set_lines(buf, 0, -1, false, { "select 1" })
  vim.api.nvim_win_set_cursor(0, { 1, 3 })
  local arg_obj = require("cogitator.commands").build_argument(buf, true)
  H.check(
    arg_obj.uri:find("^file://") ~= nil and arg_obj.position.line == 0 and arg_obj.position.character == 3,
    "commands: argument object is { uri, position } (0-based)",
    vim.inspect(arg_obj)
  )

  -- manager store: merge, vim.NIL clearing, roots listing (pure file I/O)
  local proj = vim.fs.joinpath(H.tmp, "proj-a")
  local manager = require("cogitator.manager")
  manager.remember(proj, { profiles_dir = "/etc/dbt", target = "prod" })
  manager.remember(proj, { target = vim.NIL })
  local entry = manager.for_root(proj)
  H.check(
    entry.profiles_dir == "/etc/dbt" and entry.target == nil and entry.last_used ~= nil,
    "manager: store merge + vim.NIL clear roundtrip",
    vim.inspect(entry)
  )
  H.check(vim.tbl_contains(manager.roots(), proj), "manager: roots lists the project")
end
