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

`dbt: Run/Test/Compile/Preview current model`, `dbt: Preview rows of
the CTE under the cursor` (the model run up to that CTE; the table opens
beside it, as do the "Preview rows" lenses), `dbt: Parse project`,
`dbt: Show column lineage`, `dbt: Column lineage graph` (the lineage around a column across
the project, an edge a line, from `dbt.columnLineageGraph`'s cache), `dbt: Split model at its incrementality
frontier` (the plan as an S-expression beside the model, with an
"Apply split" button), `dbt: Compare incremental / full refresh` (the
model rendered with `is_incremental()` false and true, as a unified diff
beside it), `dbt: Table models that could be incremental`
(a quick pick, slowest last run first), `dbt: Show syntax tree` (the
model's tree-sitter-dbt tree beside it), `dbt: Highlight with
tree-sitter-dbt` (colours the editor as the server highlights it — the
Jinja and the SQL between the tags, YAML with dbt-aware captures;
compiled SQL as SQL; the setting `cogitator.highlight.onOpen` —
`auto`, `always`, `off` — does it for every SQL and YAML file of the
project as it opens), `dbt: Show node info` (a macro's, test's or
source's card from `dbt.nodeInfo`), inlay hints with the value of each
`var()` (`editor.inlayHints.enabled`, docs/variables.md), `dbt:
Unresolved references` (every unresolved `ref`, `source`, `doc`, macro
and `var` of the project from `dbt.unresolved`, a quick pick that jumps
to each), `dbt: Breaking changes against the last build` (this model's
columns against `target/catalog.json` or the manifest's documented
columns from `dbt.breakingChanges`: removed, possibly renamed and retyped
columns and what reads them downstream; also a lens and a diagnostic,
refreshed on save), `dbt: Select nodes…` (dbt's selection syntax —
`tag:nightly`, `+dim_items`, `a,b`, `stg_*`, `selector:<name>`,
`… --exclude resource_type:test` — evaluated as you type by `dbt.select`,
the counts in the title; choosing a node opens it), `dbt: Show
lineage view` (the project DAG and column-level lineage from
`dbt.lineageSite` in a panel beside the editor — the same page as
`cogitator lineage-site`; it follows the active model, refreshes on save,
and clicking a file opens it at the column), `dbt: Sync
schema YAML` (the project's or this model's schema YAML brought in line
with its SQL by `dbt.syncYaml` — missing entries and columns added,
descriptions inherited down the column lineage; a summary, the plan as a
diff on request, then applied on confirm), `dbt: Rename column up- and
downstream…` (the model column at the cursor renamed by `dbt.renameColumn`:
from the model its lineage says defines it, through every model that
passes it on, or downstream only as F2 does; a summary, the edits on
request, then applied on confirm), `dbt:
Structural search (ast-grep)` (`{{ ref($M) }}` over the project's dbt
files, `sql:select $$$C from $T` over the compiled SQL; `pattern =>
rewrite` rewrites every match after asking), `dbt: Language server status`,
`dbt: Restart language server`. Everything else (code lenses, code
actions, rename, call hierarchy) rides the standard LSP features.
