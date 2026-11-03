# cogitator for Zed

First-party Zed extension registering the cogitator dbt language server
for SQL and YAML buffers.

## Install (dev extension)

1. Put a `cogitator` binary on your PATH (release asset or
   `cargo build --release -p cogitator`).
2. In Zed: `zed: install dev extension` → pick this directory
   (`editors/zed`). Zed compiles the extension itself.

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
