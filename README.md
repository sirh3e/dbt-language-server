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

## Building from source

The server is a Rust workspace:

```
cargo build --release -p cogitator
```

The binary (`cogitator`) lands at `target/release/cogitator`. See `AGENTS.md` for
development commands and `specs/rust-port/` for the implementation specs.

## Installation

Download [latest release](https://github.com/j-clemons/dbt-language-server/releases/latest) or install via curl
```
curl -fsSL https://j-clemons.com/dbt-language-server/install | bash
```

### Neovim

Add executable to $PATH
Configure with your LSP client (e.g., nvim-lspconfig):

```lua
require'lspconfig'.dbt.setup{
  cmd = { "cogitator" },
  filetypes = { "sql", "yaml" },
  root_dir = require'lspconfig'.util.root_pattern("dbt_project.yml"),
}
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
