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
  column lineage, a column's downstream impact (you pick the column),
  breaking changes, dependencies, the model graph, its card and tests,
  its relation, its schema YAML entry, extract and sync the YAML, the
  split plan, the syntax tree.
- **Project**: dbt parse, source freshness, run/test/build/compile a
  selection (you pick this model, upstream/downstream, a `selectors.yml`
  entry or a tag), list a selection's nodes, cancel a running dbt job,
  dead models, unresolved names, models that could be incremental, the
  project graph, the lineage site (opens in the browser).
- **Session**: status, the health report, switch profile/target (you
  pick from the profiles), the profiles, cache counters and clearing,
  reload `cogitator.yml`.

A long prompt pages with **More…**; **Cancel** (or closing the prompt)
ends it. A command's answer opens as a document: markdown, or the SQL
itself, under `target/.cogitator/dialogs/` (SQL the server already writes,
such as a CTE's or a test's, opens from where it is). A dbt job says it
started, and its outcome follows as a message.

Three commands are not in the menu, because a button cannot carry free
text or because Zed does the job itself: `dbt.astGrep` (a pattern),
`dbt.setProfilesDir` (set `initialization_options.profilesDir` instead)
and `dbt.highlight` (Zed colours buffers with semantic tokens). Turn the
menu off with:

```json
{ "lsp": { "cogitator": { "initialization_options": { "dialogs": false } } } }
```

## Notes

- Hover, completion, definition, references, rename, signature help,
  semantic tokens, diagnostics and code lenses come straight from the
  server; dbt run/test/compile surface as editor notifications.
- Formatting, inlay hints, document links, linked CTE editing, quickfix
  and refactor code actions, pull + workspace diagnostics all work in
  Zed out of the box, and so do the "Preview rows" and "SQL" lenses on
  each CTE, the CTE hover card, and the test-SQL and incremental-strategy
  code actions.
- Splitting a table model at its incrementality frontier is a code
  action: "Split 'cte' into the incremental model … and a table over it"
  creates the new incremental model and rewrites this one to read it.
- The third-party [zed-dbt](https://github.com/kylejbrk/zed-dbt)
  extension predates this one and also works; this extension is the
  first-party, settings-passthrough variant.
