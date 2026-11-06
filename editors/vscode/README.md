# cogitator for VS Code

A thin VS Code client for the cogitator dbt language server. All
intelligence — completion, hover, definition, lineage, diagnostics,
lenses, dbt run/test/compile/preview — lives in the server; this
extension spawns it over stdio and forwards the dbt command surface
to the palette.

## Setup

1. Install the server (`install.sh` / `install.ps1` from the
   repository root, or a release asset) so `cogitator` is on PATH —
   or point `cogitator.serverPath` at the binary.
2. Build the extension:

   ```sh
   cd editors/vscode
   npm install
   npm run compile
   ```

3. Load it: `code --install-extension` a `vsce package` output, or
   open `editors/vscode` and press F5 (Extension Development Host).

The extension activates in workspaces containing `dbt_project.yml`
and attaches to SQL and YAML documents.

## Settings

| setting | default | meaning |
| --- | --- | --- |
| `cogitator.serverPath` | `cogitator` | server executable |
| `cogitator.serverArguments` | `[]` | extra CLI flags (`--profiles-dir`, `--target`, `--fusion`, …) |

Per-project behavior (lints, limits, log verbosity…) belongs in the
hot-reloaded `cogitator.yml` at the project root —
`schemas/cogitator.schema.json` validates it in any
yaml-language-server-capable editor, VS Code's YAML extension
included.

## Palette commands

`dbt: Run/Test/Compile/Preview current model`, `dbt: Parse project`,
`dbt: Show column lineage`, `dbt: Language server status`,
`dbt: Restart language server`. Everything else (code lenses, code
actions, rename, call hierarchy) rides the standard LSP features.
