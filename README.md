# dbt Language Server (`cogitator`)

LSP for dbt. The server binary is `cogitator` (Adeptus Mechanicus naming);
its library crates use the brand as prefix plus the technical role:
`cogitator-{lsp,parser,analysis,schemas,dialects,duckdb,treesitter,error}`.

## Features

- **Code Completion**
- **Hover Information**
- **Go to Definition**
- **[Go to Schema](docs/go-to-schema.md)**
- **Function Documentation**

| Resource | Go to Definition | Hover | Completion |
| --- | --- | --- | --- |
| Model References | x | x | x |
| Sources | x | x | x |
| Seeds | x | x | x |
| Macros | x | x | x |
| Variables | x | x | x |
| Functions |   | x | x |

### Function Documentation
This is the only part of the LSP that is dialect specific. The rest is parsed
from the file system using minijinja (Jinja layer) and sqlparser (SQL layer),
focused on dbt specific syntax rather than full SQL analysis.

Supported Dialects:
- Snowflake
- BigQuery

### dbt Fusion Static Analysis
If you have dbt fusion installed, you can use it for static analysis and the 
results from compilation will be returned as diagnostics in the editor.
All artifacts from the compilation will be written to a separate directory from 
the project you are editing.

Enabled via a cli argument.
```
-f, --fusion=[path]
```
If path to the dbt fusion executable is not provided, `dbt` will be used and will look for it in `$PATH`.

### dbt profiles

The SQL dialect comes from your dbt `profiles.yml`. It can live anywhere;
the first location that contains the project's profile with a resolvable
target wins:

1. `--profiles-dir <dir>` (CLI, `~` is expanded) or
   `initializationOptions.profilesDir` sent by your editor at initialize
   (the client option replaces the CLI flag)
2. `$DBT_PROFILES_DIR`
3. `<workspace root>/profiles.yml`
4. `<workspace root>/.dbt/profiles.yml`
5. `~/.dbt/profiles.yml` (`%USERPROFILE%\.dbt\profiles.yml` on Windows)

Example Neovim configuration with a custom location:

```lua
require'lspconfig'.dbt.setup{
  cmd = { "cogitator" },
  init_options = { profilesDir = "/data/dbt-profiles" },
}
```

If no candidate resolves, the server sends a warning listing every path it
searched.

## Building from source

The server is a Rust workspace:

```
cargo build --release -p cogitator
```

The binary (`cogitator`) lands at `target/release/cogitator`. See `AGENTS.md` for
development commands and `specs/rust-port/` for the implementation specs.

## Installation

This Rust rewrite has no published releases yet — build from source (above)
or grab a binary from this repository's releases page once the first tag is
cut. (The old curl installer belongs to the pre-Rust upstream releases and
would install the previous Go binary.)

Release assets cover Linux (`cogitator-linux-amd64`), macOS
(`cogitator-darwin-amd64`, `cogitator-darwin-arm64`) and Windows
(`cogitator-windows-amd64.exe`). On Windows, put `cogitator.exe` somewhere on
`%PATH%` and configure your editor the same way as below; `file:///c%3A/…`
URIs, `%USERPROFILE%`-based profile lookup and backslash paths are handled.

### Neovim

This repository ships a first-party plugin at `editors/nvim`
(spec: `specs/neovim-plugin/00-plugin.md`) with `:Dbt*` commands, code-lens
wiring, `<leader>d` keymaps and `:checkhealth cogitator`. With lazy.nvim:

```lua
{
  dir = "/path/to/dbt-language-server/editors/nvim", -- or a git subdir plugin
  config = function()
    require("cogitator").setup({})
  end,
}
```

Or configure any LSP client manually (binary on `$PATH`):

```lua
vim.lsp.config("cogitator", {
  cmd = { "cogitator" },
  filetypes = { "sql", "yaml" },
  root_markers = { "dbt_project.yml" },
})
vim.lsp.enable("cogitator")
```

### Helix

Add executable to $PATH and add to languages.toml

```toml
[language-server.cogitator]
command = "cogitator"

[[language]]
name = "dbt"
scope = "dbt_project.yml"
file-types = ["sql","yml","yaml"]
language-servers = ["cogitator"]
```

### Zed

Search Extensions -> dbt and install.

Source for the extension: [zed-dbt](https://github.com/kylejbrk/zed-dbt)
