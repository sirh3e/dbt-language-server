# cogitator for VS Code

A thin VS Code client for the cogitator dbt language server. All
intelligence — completion, hover, definition, lineage, diagnostics,
lenses, dbt run/test/compile/preview — lives in the server; this
extension spawns it over stdio and forwards the dbt command surface
to the palette.

## 1. Install the server

The extension spawns a `cogitator` binary; put one on your PATH.
Linux/macOS:

```sh
curl -fsSL https://raw.githubusercontent.com/sirh3e/dbt-language-server/main/install.sh | sh
```

Windows (PowerShell):

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://raw.githubusercontent.com/sirh3e/dbt-language-server/main/install.ps1 | iex"
```

Both verify the download against the release's `SHA256SUMS`.
Alternatively unpack an archive from the
[releases page](https://github.com/sirh3e/dbt-language-server/releases)
or build from source: `cargo build --release -p cogitator`.

Not on PATH? Point `cogitator.serverPath` at the binary.

## 2. Install the extension

Download `cogitator-<version>.vsix` from the
[releases page](https://github.com/sirh3e/dbt-language-server/releases)
and install it — *Extensions → ⋯ → Install from VSIX*, or:

```sh
code --install-extension cogitator-<version>.vsix
```

(The extension is not on the marketplace yet.) For development,
build and run it from source instead:

```sh
cd editors/vscode
npm install
npm run compile
```

then open `editors/vscode` and press F5 (Extension Development
Host), or package your own vsix with `just vsix`.

The extension activates in workspaces containing `dbt_project.yml`
and attaches to SQL and YAML documents; with the server on PATH no
further configuration is needed.

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
