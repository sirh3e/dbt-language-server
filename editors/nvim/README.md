# cogitator for Neovim

First-party Neovim (0.11+) plugin for the cogitator dbt language
server: LSP wiring, `:Dbt*` commands, code lenses, `<leader>d`
keymaps, the model-graph browser and `:checkhealth cogitator`.

## 1. Install the server

The plugin spawns a `cogitator` binary; put one on your PATH:

```sh
curl -fsSL https://raw.githubusercontent.com/sirh3e/dbt-language-server/main/install.sh | sh
```

(verifies the download against the release's `SHA256SUMS` and
installs to `~/.local/bin`). Alternatively unpack an archive from the
[releases page](https://github.com/sirh3e/dbt-language-server/releases)
or build from source: `cargo build --release -p cogitator`.

Not on PATH? Point the plugin at it with
`setup({ cmd = { "/path/to/cogitator" } })`.

## 2. Install the plugin

The plugin lives in this repository under `editors/nvim`.

With **lazy.nvim** (clone once, point `dir` at the subdirectory):

```sh
git clone https://github.com/sirh3e/dbt-language-server ~/src/dbt-language-server
```

```lua
{
  dir = "~/src/dbt-language-server/editors/nvim",
  config = function()
    require("cogitator").setup({})
  end,
}
```

With **vim-plug** (handles the subdirectory itself):

```vim
Plug 'sirh3e/dbt-language-server', { 'rtp': 'editors/nvim' }
```
```lua
require("cogitator").setup({})
```

Or plain `runtimepath`:

```lua
vim.opt.rtp:append("~/src/dbt-language-server/editors/nvim")
require("cogitator").setup({})
```

## 3. Wire them together

`setup({})` is enough when `cogitator` is on PATH: the server attaches
to `sql` and `yaml` buffers inside a dbt project (a directory with
`dbt_project.yml` upward). The options that bind plugin and server:

```lua
require("cogitator").setup({
  cmd = { "cogitator" },   -- the server binary; flags are appended
  profiles_dir = nil,      -- profiles.yml dir → initializationOptions
  fusion = false,          -- true, or "/path/to/dbt" for Fusion mode
})
```

Verify the pairing with `:checkhealth cogitator` — it reports the
binary, the project root and the profiles.yml resolution.

## 4. Use it

`:DbtRun` / `:DbtTest` / `:DbtCompile` / `:DbtPreview` on the current
model, `:DbtGraph` and `:DbtDag` for the graph browsers, `:DbtSchema`,
`:DbtLineage`, `:DbtTarget`, `:DbtStatus` and more — the full list,
options and keymaps are in `:h cogitator`
([doc/cogitator.txt](doc/cogitator.txt)); the binding behavior spec is
[specs/neovim-plugin/00-plugin.md](../../specs/neovim-plugin/00-plugin.md).
