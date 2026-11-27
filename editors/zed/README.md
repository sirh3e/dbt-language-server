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

## The dbt menu (dialogs)

A Zed extension cannot add commands, pickers or panels, so the server
draws the dialogs itself. The extension asks for them
(`initializationOptions.dialogs`, on by default), and every SQL and YAML
file of the project gets one more entry in the code-actions menu
(`ctrl-.` / `cmd-.`):

> **dbt… (every dbt command)**

It opens the same menu as Neovim's `:Dbt`, one prompt at a time, each
choice a button:

- **Model** (on a model's `.sql` file): run, test, compile, preview rows,
  the rendered Jinja, the compiled SQL, the SQL each test runs, a CTE's
  SQL or rows (you pick the CTE), what an incremental run executes (you
  pick the strategy), incremental against full refresh, the query plan,
  column lineage, a column's downstream impact, lineage graph or warehouse
  profile (you pick the column), its changes since the last commit or the
  last dbt run (SQL, compiled SQL or lineage), breaking changes, dependencies, the model graph, its card and tests,
  its relation, its schema YAML entry, extract and sync the YAML, the
  split plan, the syntax tree.
- **Project**: dbt parse, source freshness, run/test/build/compile a
  selection (you pick this model, upstream/downstream, a `selectors.yml`
  entry or a tag), list a selection's nodes, cancel a running dbt job,
  dead models, unresolved names, models that could be incremental, the
  project graph, what the last run cost, the lineage site (opens in the
  browser).
- **Session**: status, the health report, switch profile/target (you
  pick from the profiles), the profiles, cache counters and clearing,
  reload `cogitator.yml`.

A long prompt pages with **More…**; **Cancel** (or closing the prompt)
ends it. A command's answer opens as a document: markdown, or the SQL
itself, under `target/.cogitator/dialogs/` (SQL the server already writes,
such as a CTE's or a test's, opens from where it is). A dbt job says it
started, and its outcome follows as a message.

Five commands are not in the menu, because a button cannot carry free
text or because Zed does the job itself: `dbt.astGrep` and `dbt.search`
(a pattern or a query), `dbt.setProfilesDir` (set
`initialization_options.profilesDir` instead), `dbt.renameColumn` (a new
name: rename with `F2`, below) and `dbt.highlight` (Zed colours buffers
with semantic tokens). Turn the
menu off with:

```json
{ "lsp": { "cogitator": { "initialization_options": { "dialogs": false } } } }
```

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
- Formatting, inlay hints, document links, linked CTE editing, quickfix
  and refactor code actions, pull + workspace diagnostics all work in
  Zed out of the box, and so do the "Preview rows" and "SQL" lenses on
  each CTE, the CTE hover card, and the test-SQL and incremental-strategy
  code actions.
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
Zed out of the box, and the rest of the `dbt.*` surface is in the dbt
menu (above). Splitting a table model at its incrementality frontier is
a code action too: "Split 'cte' into the incremental model … and a table
over it" creates the new incremental model and rewrites this one to read
it; its plan as an S-expression is "Split it at its incrementality
frontier: the plan" in the menu's Model group.
