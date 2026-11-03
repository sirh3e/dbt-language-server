-- Lua linting for editors/nvim (just lua-lint, luacheck from the nix shell).
std = "luajit"
globals = { "vim" }
-- stylua owns formatting; luacheck checks semantics only
max_line_length = false
-- the test runner script uses a few intentionally-global helpers
files["editors/nvim/tests/run.lua"] = {
  allow_defined_top = true,
}
