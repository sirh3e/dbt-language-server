# cogitator — a dbt language server

`cogitator` is a [language server](https://microsoft.github.io/language-server-protocol/)
for [dbt](https://www.getdbt.com/) projects, written in Rust. It knows your
models, sources, seeds, snapshots, macros, docs blocks, variables, tests,
selectors and user-defined functions, follows the column lineage between
them, and runs dbt from the editor.

It works in any LSP editor. Neovim, Emacs, VS Code and Zed have first-party
integrations in [`editors/`](editors).

```sql
select
    o.order_id,
    {{ function('is_positive_int') }}(o.amount) as amount_ok,  -- hover: the signature
    c.customer_name                                             -- completion: the columns of c
from {{ ref('stg_orders') }} as o                               -- go to definition, rename
join {{ source('shop', 'customers') }} as c using (customer_id)
where o.ordered_at >= '{{ var("since") }}'                      -- the value inline
```

## Quick start

1. **Install the server.** On Linux and macOS:

   ```bash
   curl -fsSL https://raw.githubusercontent.com/sirh3e/dbt-language-server/main/install.sh | sh
   ```

   On Windows:

   ```powershell
   powershell -ExecutionPolicy Bypass -c "irm https://raw.githubusercontent.com/sirh3e/dbt-language-server/main/install.ps1 | iex"
   ```

   Both put `cogitator` on your `PATH` ([other ways](#installation)).

2. **Add your editor's integration** ([Neovim](#neovim), [VS Code](#vs-code),
   [Emacs](#emacs), [Zed](#zed), [Helix](#helix)). With lazy.nvim it is one line:

   ```lua
   { "sirh3e/dbt-language-server", opts = {} }
   ```

3. **Open a file of a dbt project.** The server finds `dbt_project.yml`, and
   your `profiles.yml` wherever dbt would look for it. Nothing else needs to
   be set up; [docs/configuration.md](docs/configuration.md) covers the rest.

## Features

### Understanding the code

- **Hover** on a `ref()`, `source()`, `var()`, `env_var()`, `doc()`,
  `function()` or macro call: its description, value or signature. On a
  column: its YAML description and where it comes from. On a `{{ … }}`: what
  it renders to. On a dialect SQL function: its documentation (Snowflake and
  BigQuery bundled).
- **Column lineage** (`dbt.columnLineage`) through CTEs and `select *` down
  to the staging columns, a self-contained lineage site
  (`cogitator lineage-site`) and a live view in VS Code.
  [docs/lineage-site.md](docs/lineage-site.md)
- **Variables**: each `var()`'s value inline, where it is defined (scopes,
  packages, overrides), and the vars nothing reads.
  [docs/variables.md](docs/variables.md)
- **Output-column types**, checked against the `data_type:` the YAML declares.
- **Highlighting** from the server for SQL, Jinja, compiled SQL and YAML
  properties files, through
  [tree-sitter-dbt](https://github.com/sirh3e/tree-sitter-dbt).
  [docs/highlighting.md](docs/highlighting.md)

### Writing it

- **Completion** that knows where the cursor is: models in `ref('`, sources
  and tables in `source('`, docs blocks in `doc('`, functions in
  `function('`, relations after `FROM` and `JOIN`, columns after `SELECT` and
  `alias.`, keyword continuations (`group ` → `by`), and the keys and values
  of properties files. [docs/properties-yaml.md](docs/properties-yaml.md)
- **Signature help** for macros, with defaults and the active parameter.
- **Diagnostics** as you type: Jinja and SQL syntax errors; a `ref()`,
  `source()`, `doc()` or `function()` that names nothing, with did-you-mean;
  unknown vars; macro calls with the wrong arguments; dbt YAML schema errors;
  private `ref()`s; contract violations; and opt-in performance advice.
  [docs/lint-and-format.md](docs/lint-and-format.md),
  [docs/macros.md](docs/macros.md)
- **Quick fixes**: declare a missing model, source, var or docs block, add the
  missing columns to the YAML, run `dbt deps`.
- **User-defined functions** (dbt 1.11+): hover, definition, completion,
  references and the dangling check for `{{ function('…') }}`, read from
  `functions/` and its `functions:` entries.
  [docs/functions.md](docs/functions.md)

### Navigating and changing it

- **Go to definition** of every name above, **go to schema** (its YAML
  entry), **go to tests**, and **go to implementation** of a dispatched
  macro. [docs/go-to-schema.md](docs/go-to-schema.md)
- **Find references** of models, sources, macros, docs blocks, functions,
  columns and vars, and the **call hierarchy** of sources, macros, docs
  blocks and columns.
- **Rename** a model (every `ref()`, its YAML entry and its file), a model's
  output column across the project, a var, a macro, or a name local to the
  file (CTEs, aliases, Jinja variables).
  [docs/column-rename.md](docs/column-rename.md),
  [docs/local-names.md](docs/local-names.md)
- **File operations**: renaming, moving or deleting a model file in the
  editor updates what refers to it. [docs/file-operations.md](docs/file-operations.md)
- **Code lenses** per model: upstream and downstream counts, tests and
  coverage, run, test, compile, and preview rows.
- **Search** by tag and selector (`tag:x`, `selector:x` in workspace
  symbols), and dbt's own node-selection language (`dbt.select`).
  [docs/selectors.md](docs/selectors.md)

### Running dbt

- **dbt run, test, compile, parse and deps** from the editor, with progress
  and cancelling; the compiled SQL comes from the server (`dbt.compiledSql`).
- **Preview the rows of a CTE** (`dbt.previewCte`). [docs/preview.md](docs/preview.md)
- **Breaking changes**: a model's columns against its last built schema, and
  what that breaks downstream. [docs/breaking-changes.md](docs/breaking-changes.md)
- **Query plan** (`dbt.queryPlan`): each IR level of a model, the optimiser's
  findings, and which `table` models could be incremental.
  [docs/incrementality.md](docs/incrementality.md)
- **Schema YAML in sync** with the SQL, project-wide (`dbt.syncYaml`,
  `cogitator yaml-sync`). [docs/yaml-sync.md](docs/yaml-sync.md)
- **Profiles and targets**: profiles.yml found anywhere, targets switched at
  runtime, Snowflake's auth methods checked.
  [docs/configuration.md](docs/configuration.md)
- **dbt Fusion**: its compile results as diagnostics, with `--fusion`.

### In CI

`cogitator lint` (syntax, lint checks, refs, YAML) and `cogitator format`
run the server's checks over a whole project, with exit codes and
machine-readable output. [docs/lint-and-format.md](docs/lint-and-format.md)

Every `dbt.*` command the server answers is in
[docs/commands.md](docs/commands.md), and all the documentation is indexed in
[docs/README.md](docs/README.md).

## Editors

| Capability | Neovim | Emacs | VS Code | Zed and any LSP editor |
|---|---|---|---|---|
| Diagnostics, completion, hover, definition, references, rename, symbols, folding, semantic tokens, signature help | ✓ | ✓ | ✓ | ✓ |
| Code actions: the dbt commands at the cursor, quick fixes, target switching | ✓ | ✓ | ✓ | ✓ |
| Code lenses | ✓ | ✓ | ✓ | editor-dependent |
| dbt run, test, compile and parse; schema and tests navigation | ✓ commands, keymaps | ✓ commands | ✓ palette | via code actions |
| Pickers: targets, tags, selectors, projects | ✓ | ✓ | — | — |
| Model-graph browser | ✓ `:DbtGraph` | ✓ `cogitator-graph` | — | — |
| Project explorer and DAG | ✓ `:DbtBrowse`, `:DbtDag` | — | — | — |
| Live column lineage | — | — | ✓ | — |
| Health report | ✓ `:checkhealth cogitator` | ✓ `cogitator-doctor` | ✓ server status | — |

Zed's extension API cannot add commands, pickers or panels, so what stock
LSP carries is its ceiling. That is why the dbt workflows are served as code
actions and lenses rather than as client-side features. Which command reaches
which editor is held by a test ([docs/editor-parity.md](docs/editor-parity.md)).

## Installation

[install.sh](install.sh) (Linux and macOS, into `~/.local/bin`) and
[install.ps1](install.ps1) (Windows, into `%LOCALAPPDATA%\Programs\cogitator`,
added to your user `PATH`) download the latest release of this repository.
Both honor `COGITATOR_REPO`, `COGITATOR_VERSION` and `COGITATOR_BIN_DIR`.

Or take a binary from the
[releases page](https://github.com/sirh3e/dbt-language-server/releases):
`cogitator-linux-amd64.tar.gz`, `cogitator-darwin-amd64.tar.gz`,
`cogitator-darwin-arm64.tar.gz` and `cogitator-windows-amd64.zip`, each with
the binary, LICENSE and README, plus a `SHA256SUMS` to verify them and the
VS Code extension, `cogitator-<version>.vsix`. Or build it
([below](#building-and-developing)).

Every editor pairs the same way: install the server, then the editor's
integration. Each has a step-by-step guide in its directory.

### Neovim

[`editors/nvim`](editors/nvim) ([install guide](editors/nvim/README.md),
[docs/neovim.md](docs/neovim.md)): `:Dbt…` commands, the model explorer, the
DAG, fzf-style pickers, `<leader>d` keymaps and `:checkhealth cogitator`.
With lazy.nvim it is the one line above (the repository's
[`lazy.lua`](lazy.lua) does the rest). A fuller spec, with lazy-loading and
an opt-in `build` that installs the server, is
[editors/nvim/examples/lazy.lua](editors/nvim/examples/lazy.lua).

Without the plugin, Neovim 0.11 or later needs only:

```lua
vim.lsp.config("cogitator", {
  cmd = { "cogitator" },
  filetypes = { "sql", "yaml" },
  root_markers = { "dbt_project.yml" },
})
vim.lsp.enable("cogitator")
```

### VS Code

[`editors/vscode`](editors/vscode) ([install guide](editors/vscode/README.md)):
install `cogitator-<version>.vsix` from the releases page through
*Extensions → ⋯ → Install from VSIX* (it is not on the marketplace yet). It
has palette commands for dbt run, test, compile and preview, the live
lineage view and the server's status. `cogitator.serverPath` points at the
binary when it is not on `PATH`.

### Emacs

[`editors/emacs`](editors/emacs) ([install guide](editors/emacs/README.md)),
for eglot (built in since Emacs 29) and lsp-mode:

```elisp
(add-to-list 'load-path "/path/to/dbt-language-server/editors/emacs")
(require 'cogitator)
(require 'cogitator-graph)
(cogitator-setup-eglot)
(add-hook 'sql-mode-hook #'eglot-ensure)
```

### Zed

[`editors/zed`](editors/zed) ([install guide](editors/zed/README.md)): run
`zed: install dev extension`, then configure it through the standard
`lsp.cogitator` settings (binary path, arguments,
`initialization_options`). The third-party
[zed-dbt](https://github.com/kylejbrk/zed-dbt) extension works too.

### Helix

```toml
[language-server.cogitator]
command = "cogitator"

[[language]]
name = "dbt"
scope = "dbt_project.yml"
file-types = ["sql", "yml", "yaml"]
language-servers = ["cogitator"]
```

## Configuration

Everything is optional, and all of it is in
[docs/configuration.md](docs/configuration.md):

- **profiles.yml** is found the way dbt finds it, or wherever
  `--profiles-dir`, `initializationOptions.profilesDir` or
  `$DBT_PROFILES_DIR` say. Its SQL dialect steers parsing and keywords.
- **`cogitator.yml`** at the project root, hot-reloaded: profile and target,
  lint checks, inlay hints, performance rules, preview limits, var
  overrides.
- **dbt Fusion** (`--fusion`), **performance advice** (`perf.profile`), and
  **OpenTelemetry** traces and metrics (`--otlp`).

## Building and developing

```bash
cargo build --release -p cogitator
```

Development goes through the [justfile](justfile): `just check` is what CI
runs (format, clippy with `-D warnings`, every test suite, schema
freshness), `just system-test` drives Neovim, Emacs and VS Code headlessly,
and `just nvim-test`, `just emacs-test`, `just vscode-test` and
`just zed-build` run one editor each. CI runs on Linux, Windows and macOS,
and releases cross-compile with cargo-zigbuild.

The toolchain is declared once in Nix (`nix/tooling.nix`) and reached
through `nix-shell`, `devenv shell` or `direnv allow`. Git hooks run through
[prek](https://prek.j178.dev), and the changelog is generated from the
Conventional Commits history with [git-cliff](https://git-cliff.org)
(`just changelog`). The workflow (trunk-based, squash-merged pull requests)
and the release cycle are in [CONTRIBUTING.md](CONTRIBUTING.md), the command
reference is in [AGENTS.md](AGENTS.md), and the binding specifications are in
[`specs/`](specs).

## Origin

cogitator is a complete Rust rewrite of
[j-clemons/dbt-language-server](https://github.com/j-clemons/dbt-language-server),
the original Go implementation. The idea, the first feature set and the wire
behavior are that project's. The rewrite was made for speed (parsing,
project scans and completion answer in milliseconds on large projects) and
as a base for what the Go server did not have.

The Go behavior was first written down as binding specs in
[`specs/rust-port/`](specs/rust-port/), and the port was written against
them. Wire shapes are Go-exact and extended only additively, with one
deliberate deviation: the `shutdown` response carries `"result": null`, as
JSON-RPC requires. The last Go state is in git history at commit `724b0f2`.
The binary is named `cogitator`, and the library crates
`cogitator-{parser,analysis,lsp,schemas,…}`.

## License

[MIT](LICENSE).
