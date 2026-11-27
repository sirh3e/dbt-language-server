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

## Test it in a real Zed

The dev shell (`devenv shell`, or `nix-shell`) brings Zed (`zeditor`, from its
own pin in `nix/zed.nix`), Xvfb and `wasm-tools` on Linux.

```
just zed-package      # the extension as Zed installs it, in target/package/
just zed-smoke        # a real Zed, no display needed; `just zed-smoke 120` waits longer
```

`zed-package` adapts the `wasm32-wasip1` module into the wasm *component*
Zed loads, with wasmtime's WASI preview1 reactor adapter (v18.0.2, as Zed's
dev-extension builder uses), downloaded once and checked by its SHA-256.

`zed-smoke` starts Zed under Xvfb on Mesa's software Vulkan (lavapipe), with
a throwaway home that holds the packaged extension and trusts every worktree,
opens `testdata/jaffle_shop_duckdb`'s `models/schema.yml` (YAML is built into
Zed; SQL would need Zed's SQL extension from the network), and passes once
Zed has loaded the extension and started this checkout's debug `cogitator`.
It fails when Zed refuses the extension, saying why. Your own Zed settings
and extensions are never touched. Outside the dev shell, unset an
`LD_LIBRARY_PATH` that points at another glibc, or Zed cannot start.

## Notes

- Hover, completion, definition, references, rename, signature help,
  semantic tokens, diagnostics and code lenses come straight from the
  server; dbt run/test/compile surface as editor notifications.
- The third-party [zed-dbt](https://github.com/kylejbrk/zed-dbt)
  extension predates this one and also works; this extension is the
  first-party, settings-passthrough variant.

## Turn on inlay hints

Zed keeps inlay hints off until you turn them on, and the server sends
a lot through them: the inline column lineage after each output column
(`customer_id ← stg_customers.customer_id`), `var()` and `env_var()`
values, plan row estimates. The lineage hints are of kind *type*:

```json
{
  "inlay_hints": { "enabled": true, "show_type_hints": true }
}
```

(or the same under `"languages": { "SQL": { "inlay_hints": … } }` to
keep them to SQL). A hint's tooltip says where the column comes from,
and its upstream part carries the location of the column it names. Their length
and whether they show at all are `inlay.lineage` and
`inlay.lineage_max_len` in `cogitator.yml`, as in every editor.

## What Zed gets, and how

A Zed extension cannot add commands, panels or windows; what Zed gets
arrives through standard LSP requests:

- **SQL aliases, CTEs and column aliases**: go to definition, find
  references, document highlights and rename on a table alias, a CTE
  name or a `select … as x` alias, inside the file.
- **Column completion**: `alias.column` and bare columns of what the
  `FROM`/`JOIN` has in scope, ranked above functions and keywords by
  `sortText`, and after `alias.` only that relation's columns. Zed
  sorts by its own fuzzy score first and uses `sortText` only to break
  ties, so the columns lead whenever the typed text fits them and a
  global name equally well; a closer fuzzy match still comes first.
- **A column's lineage graph** — `dbt.columnLineageGraph` — is a code
  action: put the cursor on an output column of a model (in its SQL or
  its YAML entry), open the code actions (`cmd-.`), and pick
  **Show column lineage of model.column**. The walk, up- and
  downstream three hops, opens as a text file under
  `target/.cogitator/lineage/`: the columns layer by layer from the
  sources down (the start marked ▶, each with its file), the edges
  (`direct` when a column passes its input through, else `derived`),
  and the models still being computed (run the action again). The
  lineage comes from the server's project-wide cache, so the second
  walk anywhere is instant.
- **Rename a column up- and downstream** — `dbt.renameColumn` with
  `direction: "both"` — is the rename you already have (`F2`), once the
  settings ask for it:

  ```json
  { "lsp": { "cogitator": {
      "initialization_options": { "renameColumnDirection": "both" }
  } } }
  ```

  With `"both"`, renaming a column the model passes through renames it
  from the model that defines it, so every model on its way keeps one
  name; the default, `"down"`, renames it here and in its consumers
  and aliases it (`old as new`) where it comes in. It is a setting
  rather than a choice per rename because a rename request has no room
  for one; restart the server after changing it.
- **Code lenses and code actions** of a model: **Column lineage**
  (`dbt.showLineage`, as a message), **dbt run** / **dbt test** /
  **dbt compile** (`dbt.run`, `dbt.test`, `dbt.compile`, as
  notifications), **Extract schema YAML** (`dbt.extractModelYaml`),
  **Compare incremental / full refresh** (`dbt.incrementalDiff`, the
  diff opens as a file), and in the active profiles.yml
  **Switch target** (`dbt.setTarget`).

## Server features beyond stock LSP

Formatting, inlay hints, document links, linked CTE editing, quickfix
and refactor code actions, pull + workspace diagnostics all work in
Zed out of the box. The rest of the `dbt.*` executeCommand surface
(doctor, impact, deadModels, expandJinja, runSelection/cancel, the
column-lineage dialog of Neovim, …) has no generic Zed UI — use the
Neovim, Emacs or VS Code integrations, or any client that can send
workspace/executeCommand.

Splitting a table model at its incrementality frontier is a code
action, so it works here: "Split 'cte' into the incremental model … and
a table over it" in the code-actions menu creates the new incremental
model and rewrites this one to read it. Its plan as an S-expression
(`dbt.splitPlan`) and the project-wide `dbt.incrementality` list need
an executeCommand client.
