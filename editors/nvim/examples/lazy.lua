-- cogitator.nvim with lazy.nvim: copy this file to
-- ~/.config/nvim/lua/plugins/cogitator.lua (or paste the table into your
-- require("lazy").setup({ … }) list). Everything but the first line is
-- optional; the repository's own lazy.lua puts editors/nvim on the
-- runtimepath and calls require("cogitator").setup(opts).
return {
  "sirh3e/dbt-language-server",

  -- Start when a SQL or YAML file is read, early enough for the language
  -- server to attach to that first buffer. (Not `ft`: the client is
  -- enabled on FileType, which would already have fired.)
  event = { "BufReadPre *.sql", "BufReadPre *.yml", "BufReadPre *.yaml", "BufNewFile *.sql" },

  -- Opt-in: install the cogitator server from the latest release on
  -- install and on every :Lazy update (checksum-verified, into
  -- ~/.local/bin; set COGITATOR_BIN_DIR to change it). Drop this line if
  -- you install the server yourself and set opts.cmd below.
  build = vim.fn.has("win32") == 1 and "powershell -NoProfile -ExecutionPolicy Bypass -File install.ps1"
    or "sh install.sh",

  ---@type table see :h cogitator-nvim-options for every option
  opts = {
    -- cmd = { vim.fn.expand("~/.local/bin/cogitator") }, -- the server, when not on PATH
    -- profiles_dir = "~/.dbt",   -- where profiles.yml lives (else dbt's own rules)
    -- fusion = false,            -- true, or "/path/to/dbt", for dbt Fusion
    -- keymaps = true,            -- the <leader>d… maps (:h cogitator-nvim-commands)
    -- inlay_hints = true,        -- column lineage, ref relations, var values
    -- lens = { enabled = true }, -- run/test/compile code lenses
  },
}
