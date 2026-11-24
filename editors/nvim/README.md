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

### lazy.nvim

One line is enough — the repository's own [`lazy.lua`](../../lazy.lua)
puts `editors/nvim` on the runtimepath and calls `setup(opts)`:

```lua
{ "sirh3e/dbt-language-server", opts = {} }
```

A fuller spec, to copy into `~/.config/nvim/lua/plugins/cogitator.lua`
([examples/lazy.lua](examples/lazy.lua)):

```lua
return {
  "sirh3e/dbt-language-server",
  -- start when a SQL/YAML file is read, early enough for the server to attach
  event = { "BufReadPre *.sql", "BufReadPre *.yml", "BufReadPre *.yaml", "BufNewFile *.sql" },
  -- opt-in: install the server from the latest release on install and :Lazy update
  build = vim.fn.has("win32") == 1 and "powershell -NoProfile -ExecutionPolicy Bypass -File install.ps1"
    or "sh install.sh",
  opts = {
    -- cmd = { vim.fn.expand("~/.local/bin/cogitator") },
    -- profiles_dir = "~/.dbt",
    -- fusion = false,
  },
}
```

- `build` runs this repository's checksum-verified [`install.sh`](../../install.sh)
  (`install.ps1` on Windows) in the plugin's directory, so the server
  follows the plugin on every `:Lazy update`; it installs to
  `~/.local/bin` (set `COGITATOR_BIN_DIR` to change it). Leave it out if
  you install the server yourself (§1).
- Lazy-load on `BufReadPre`/`BufNewFile`, not `ft`: the server is enabled
  on the `FileType` event, which `ft` loading would arrive after.
- `:h cogitator` works: the plugin's help is tagged on first start.

To **try it without touching your config**,
[examples/minimal_init.lua](examples/minimal_init.lua) bootstraps
lazy.nvim and the plugin into a throwaway directory:

```sh
nvim -u editors/nvim/examples/minimal_init.lua path/to/models/some_model.sql
```

### vim-plug

```vim
Plug 'sirh3e/dbt-language-server', { 'rtp': 'editors/nvim' }
```
```lua
require("cogitator").setup({})
```

### Plain `runtimepath`

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
model, `:DbtFind` for fuzzy pickers (models, definitions, columns,
lineage… in fzf-lua, snacks.picker or telescope when installed, else a
built-in one), `:DbtBrowse` for the model explorer (upstream | model | downstream over
its dbt SQL | compiled SQL), `:DbtGraph`
and `:DbtDag` for the graph browsers, `:DbtReferences` (`grr`) for
where a model or macro is used and `:DbtMacros` for every macro's
callers, `:DbtRename` (`grn`) to rename a model or macro after a preview
of the edit, `:DbtSchema`,
`:DbtLineage`, `:DbtTarget`, `:DbtStatus` and more. `gra` (or
`<leader>dA`) opens every code action at the cursor — the server's fixes
and refactors with a diff preview, and the plugin's own commands — in a
yazi-style menu walked with `h j k l`, as every dialog of the plugin
is. The full list,
options and keymaps are in `:h cogitator`
([doc/cogitator.txt](doc/cogitator.txt)); the binding behavior spec is
[specs/neovim-plugin/00-plugin.md](../../specs/neovim-plugin/00-plugin.md).
