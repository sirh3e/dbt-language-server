-- lazy.nvim reads this file when a config installs "sirh3e/dbt-language-server"
-- and adds it to that plugin's spec. The Neovim plugin lives in editors/nvim,
-- not at the repository root, so this puts it on the runtimepath; a user's
-- spec then needs nothing more than
--
--   { "sirh3e/dbt-language-server", opts = {} }
--
-- See editors/nvim/README.md and editors/nvim/examples/lazy.lua.
return {
  "sirh3e/dbt-language-server",
  main = "cogitator",
  init = function(plugin)
    local nvim = plugin.dir .. "/editors/nvim"
    vim.opt.rtp:append(nvim)
    -- lazy.nvim tags only <plugin>/doc: make :h cogitator work
    if vim.fn.filereadable(nvim .. "/doc/tags") == 0 then
      pcall(vim.cmd.helptags, nvim .. "/doc")
    end
  end,
  config = function(_, opts)
    -- lazy.nvim sources only <plugin>/plugin; the user commands live here
    vim.cmd.runtime("plugin/cogitator.lua")
    require("cogitator").setup(opts or {})
  end,
}
