# cogitator for Zed

First-party Zed extension registering the cogitator dbt language server
for SQL and YAML buffers.

## 1. Install the server

The extension spawns a `cogitator` binary; put one on your PATH:

```sh
curl -fsSL https://raw.githubusercontent.com/sirh3e/dbt-language-server/main/install.sh | sh
```

(verifies the download against the release's `SHA256SUMS`; Windows
uses `install.ps1` the same way). Alternatively unpack an archive
from the
[releases page](https://github.com/sirh3e/dbt-language-server/releases)
or build from source: `cargo build --release -p cogitator`.

Not on PATH? Point `lsp.cogitator.binary.path` at the binary (below).

## 2. Install the extension (dev extension)

Clone this repository, then in Zed: `zed: install dev extension` →
pick this directory (`editors/zed`). Zed compiles the extension
itself.

## Settings

Everything is configured through Zed's standard LSP settings — the
`initialization_options` pass straight through to the server, so dbt
profiles work the same as in the other editors:

```json
{
  "lsp": {
    "cogitator": {
      "binary": {
        "path": "/path/to/cogitator",
        "arguments": ["--fusion", "/path/to/dbt"]
      },
      "initialization_options": {
        "profilesDir": "/data/dbt-profiles"
      }
    }
  }
}
```

Both keys are optional: without `binary.path` the extension finds
`cogitator` on PATH; without `profilesDir` the server walks its own
search order (`$DBT_PROFILES_DIR`, `<root>/profiles.yml`,
`<root>/.dbt/profiles.yml`, `~/.dbt/profiles.yml`).

## Check the build

```
just zed-build
```

compiles the extension to WebAssembly (`wasm32-wasip1`) the way Zed does.

## Notes

- Hover, completion, definition, references, rename, signature help,
  semantic tokens, diagnostics and code lenses come straight from the
  server; dbt run/test/compile surface as editor notifications.
- The third-party [zed-dbt](https://github.com/kylejbrk/zed-dbt)
  extension predates this one and also works; this extension is the
  first-party, settings-passthrough variant.

## Server features beyond stock LSP

Formatting, inlay hints, document links, linked CTE editing, quickfix
and refactor code actions, pull + workspace diagnostics all work in
Zed out of the box. The `dbt.*` executeCommand surface (doctor,
impact, deadModels, expandJinja, runSelection/cancel, …) has no
generic Zed UI yet — use the Neovim or Emacs integrations, or any
client that can send workspace/executeCommand.
