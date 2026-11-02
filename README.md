# cogitator — a dbt language server in Rust

`cogitator` is a language server for [dbt](https://www.getdbt.com/)
projects: hover, completion, navigation, lineage, refactoring and dbt
execution for your models, sources, macros and schema YAML.

## Origin

This project is a **complete Rust rewrite** of
[j-clemons/dbt-language-server](https://github.com/j-clemons/dbt-language-server),
the original Go implementation — all credit for the idea, the initial
feature set and the wire behavior goes to that project. The rewrite was
done for speed (parsing, project scans and completion answer in
milliseconds on large projects) and as a base for features the Go server
did not have.

- The Go behavior was captured first as binding specs in
  [`specs/rust-port/`](specs/rust-port/) (00–09) and the port was written
  against them; wire shapes are Go-exact, extended only additively. The
  one deliberate deviation: the `shutdown` response carries `"result": null`
  as JSON-RPC requires (Neovim rejects the bare Go shape).
- The last Go state lives in git history at commit `724b0f2`.
- The binary is named `cogitator` (Adeptus Mechanicus naming); the library
  crates use the brand plus their technical role:
  `cogitator-{error,lsp,dialects,schemas,parser,analysis,duckdb,treesitter}`.

## Features inherited from the Go server

| Resource | Go to Definition | Hover | Completion |
| --- | --- | --- | --- |
| Model References | x | x | x |
| Sources | x | x | x |
| Seeds | x | x | x |
| Macros | x | x | x |
| Variables | x | x | x |
| Functions |   | x | x |

Plus: go to schema, bundled function documentation (Snowflake and
BigQuery), dbt Fusion static analysis, document symbols, and the
`DBT_PROFILES_DIR` dialect lookup.

## Features added in this project

**Language intelligence**

- Grammar-aware **completion engine**: classifies the cursor's grammatical
  position (member access `alias.`, relation position after FROM/JOIN,
  column position after SELECT/WHERE/ON, keyword continuations like
  `group ` → `by`) and ranks suggestions accordingly
- **Write assist**: local CTE/alias scope index, model and source columns
  from schema YAML as completion with documentation
- **Column lineage** (`dbt.columnLineage` / `dbt.showLineage`):
  CTE-transitive with `select *` substitution, down to staging columns
- **Column hover and go-to-definition**: YAML description plus lineage on
  hover, jump from a column to its schema-YAML entry
- Structured **signature help** for macros (real parameter objects with
  defaults, active-parameter tracking)
- **Semantic tokens**, folding ranges, selection ranges, document
  highlights, workspace symbols, find references
- **Diagnostics**: dangling `ref()` detection, dbt YAML schema validation
  (typify-generated types from [dbt-jsonschema]), pull diagnostics
  (`textDocument/diagnostic`)
- dbt **YAML intelligence**: property-name completion and schema errors in
  `schema.yml` / `dbt_project.yml`

**Refactoring and navigation**

- **Model rename** (`textDocument/rename`): rewrites every dependent's
  `ref()`, updates the schema-YAML entry and renames the model file — one
  WorkspaceEdit over the project graph
- **Code lenses** per model: `N downstream` / `N upstream`, go to schema,
  column lineage, dbt run/test/compile, `N tests` (documented) or
  `Extract schema YAML` (undocumented); per macro: `N call sites`
- **Extract model → schema YAML** (`dbt.extractModelYaml`): scaffolds
  `<model>.yml` with the model's real columns (from lineage) when no entry
  exists, or adds the next `versions:` entry (and bumps `latest_version`)
  when one does — applied in the editor via workspace/applyEdit
- **Go to tests** (`dbt.modelTests` / `dbt.goToTests`): jump to every
  `data_tests`/`tests` entry defined for a model — model-level, per
  column, and per version (a `<model>_v2.sql` buffer resolves to its base)
- **Model-graph browser** (`dbt.modelGraph`): direct upstream/downstream
  neighbors for the yazi-style three-pane UI in the Neovim and Emacs
  integrations
- **Tag and selector search**: model tags from YAML and
  `{{ config(tags=…) }}` plus `selectors.yml` entries are indexed;
  `workspace/symbol` understands `tag:x` and `selector:x` queries in any
  editor, and Neovim adds `:DbtTags` / `:DbtSelectors` pickers
- **Project DAG view — Neovim only**: `:DbtDag` renders the whole project
  dbt-docs style (layered left→right, box-drawn nodes with tags, edges),
  and docglow-style column information inside the nodes (`c`/`C` expand,
  `L` column lineage, `<CR>` opens the model). The `dbt.projectGraph`
  data command is editor-neutral, but this UI ships only in the Neovim
  plugin.

**dbt workflow**

- **dbt execution** from the editor (`dbt.run` / `dbt.test` /
  `dbt.compile` with `--select` on the current model); a successful
  compile opens the compiled SQL
- **dbt profiles from anywhere**: `--profiles-dir`,
  `initializationOptions.profilesDir`, `$DBT_PROFILES_DIR`,
  `<root>/profiles.yml`, `<root>/.dbt/profiles.yml`, `~/.dbt/profiles.yml`
  — first file whose profile resolves a target wins (details below)
- **Connection auth manager**: Snowflake auth classification (password,
  key pair, SSO, OAuth, MFA, JWT) with per-method validation, a redacted
  `dbt.profiles` inventory behind a profile allowlist, runtime
  profile/target switching (`dbt.setTarget`, `:DbtTarget`), and auth
  diagnostics inside profiles.yml
- **Actionable errors**: every failure surfaces as an editor message with
  a Reason and a Solution (missing project, unresolved dialect, missing
  dbt binary, failed runs with the exact retry command)
- **Status visibility**: context refreshes and dbt runs report through
  standard `$/progress` (visible in `vim.lsp.status()`, fidget, eglot's
  mode line), and `dbt.status` (`:DbtStatus`, `M-x cogitator-status`)
  answers with the full picture — backend, fusion state, project counts,
  dialect, active profile/target, refresh timings and cache stats

**Engine and infrastructure**

- Parsing by **minijinja** (Jinja layer) + **sqlparser** (SQL layer) — no
  hand-written lexer; zero-copy tokens over the document text
- **tree-sitter** backend (vendored `tree-sitter-jinja2`) for
  error-tolerant segmentation of broken buffers (`--no-tree-sitter` falls
  back to the plain pipeline)
- Optional **DuckDB 2.0 parser** diagnostics (`--features duckdb-parser`,
  C/C++ built with the Zig toolchain)
- **Project scan cache** keyed by mtime/size: unchanged files are never
  re-parsed across refreshes; petgraph project graph
- `rootUri`/`workspaceFolders` support (modern clients omit the deprecated
  `rootPath`)
- **Windows support**: drive-letter `file:///c%3A/…` URIs,
  `%USERPROFILE%` lookup, a `windows-amd64.exe` release artifact
- **Observability**: `tracing` with optional OTLP export (`--otlp`) and a
  tokio-console feature
- Tokio task architecture (reader → dispatcher → writer, debounced
  background context refresh), cross-platform releases via cargo-zigbuild,
  CI tests on Linux/macOS/Windows

**Editor integrations (first-party, in this repository)**

- **Neovim** plugin ([`editors/nvim`](editors/nvim)): `:Dbt*` commands,
  model-graph browser, code-lens wiring, `:DbtProfiles`,
  `:checkhealth cogitator`, headless test suite
- **Emacs** package ([`editors/emacs`](editors/emacs)): eglot + lsp-mode,
  the same commands and graph browser
- **Zed** extension ([`editors/zed`](editors/zed)): server registration
  with settings passthrough

[dbt-jsonschema]: https://github.com/dbt-labs/dbt-jsonschema

## Dialects

Function hover/completion documentation is bundled for **Snowflake** and
**BigQuery**. The dialect itself (from `profiles.yml`) also steers the SQL
parser and keyword set — DuckDB, Postgres and the other adapter types are
recognized, with generic function docs.

## dbt Fusion static analysis

With dbt Fusion installed, compilation results come back as diagnostics in
the editor; artifacts are written outside your project.

```
-f, --fusion=[path]
```

Without a path, `dbt` from `$PATH` is used. The binary is validated at
startup (its `--version` must report `dbt-fusion`), with a clear message
if not.

## dbt profiles

The SQL dialect comes from your dbt `profiles.yml`. It can live anywhere;
the first location that contains the project's profile with a resolvable
target wins:

1. `--profiles-dir <dir>` (CLI, `~` is expanded) or
   `initializationOptions.profilesDir` sent by your editor at initialize
   (the client option replaces the CLI flag)
2. `$DBT_PROFILES_DIR`
3. `<workspace root>/profiles.yml`
4. `<workspace root>/.dbt/profiles.yml`
5. `~/.dbt/profiles.yml` (`%USERPROFILE%\.dbt\profiles.yml` on Windows)

If no candidate resolves, the server sends a warning listing every path it
searched. In the Neovim plugin, `profiles_dir` in `setup()` or
`:DbtProfiles <dir>` at runtime configures this; in Emacs it is
`cogitator-profiles-dir`; in Zed it is
`lsp.cogitator.initialization_options.profilesDir`.

### Connection auth manager (Snowflake first-class)

Snowflake alone has half a dozen ways to connect — password, key pair,
SSO (`authenticator: externalbrowser`), OAuth, MFA, JWT. The server parses
profiles.yml into a **redacted inventory** (auth methods and what is
missing, never a credential value) and validates each target:

- unresolved `{{ env_var('…') }}` references (no value, no default)
- password auth without `user`/`password`, key-pair auth whose
  `private_key_path` does not exist, OAuth without `token`, missing
  `account`, unknown `authenticator` values

Problems surface three ways: a warning when the *active* target is broken,
diagnostics inside profiles.yml when you open it, and the
`dbt.profiles` report.

**Read scope**: by default only the profile named in `dbt_project.yml` is
parsed and exposed; widen it with `--profiles-allow name1,name2` or
`initializationOptions.profilesAllow`.

**Active target**: the profile's default `target:` unless overridden by
`--target` / `initializationOptions.target` — or switched at runtime with
the `dbt.setTarget` command, which re-resolves the SQL dialect and passes
`--target` to dbt run/test/compile. In Neovim: `:DbtTarget` (a picker
showing each target's auth method and issues); in Emacs:
`M-x cogitator-target`.

## Installation

On **Linux and macOS**, [install.sh](install.sh) downloads the latest
release into `~/.local/bin`:

```bash
curl -fsSL https://raw.githubusercontent.com/sirh3e/dbt-language-server/main/install.sh | sh
```

On **Windows**, [install.ps1](install.ps1) does the same into
`%LOCALAPPDATA%\Programs\cogitator` and adds it to your user `PATH`:

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://raw.githubusercontent.com/sirh3e/dbt-language-server/main/install.ps1 | iex"
```

Both scripts point at **this** repository (the upstream Go project's
installer no longer applies) and honor `COGITATOR_REPO`,
`COGITATOR_VERSION` and `COGITATOR_BIN_DIR` environment variables.
Alternatively grab a binary from the
[releases page](https://github.com/sirh3e/dbt-language-server/releases)
or build from source (below).

Release assets cover Linux (`cogitator-linux-amd64`), macOS
(`cogitator-darwin-amd64`, `cogitator-darwin-arm64`) and Windows
(`cogitator-windows-amd64.exe`). On Windows, put `cogitator.exe` somewhere
on `%PATH%` and configure your editor the same way as below.

### Neovim

First-party plugin at [`editors/nvim`](editors/nvim)
(spec: `specs/neovim-plugin/00-plugin.md`): `:DbtRun/Test/Compile/Schema/
Lineage/Graph/Profiles`, the yazi-style model-graph browser, code-lens
wiring, `<leader>d` keymaps and `:checkhealth cogitator`. With lazy.nvim:

```lua
{
  dir = "/path/to/dbt-language-server/editors/nvim",
  config = function()
    require("cogitator").setup({})
  end,
}
```

Or configure any LSP client manually (binary on `$PATH`):

```lua
vim.lsp.config("cogitator", {
  cmd = { "cogitator" },
  filetypes = { "sql", "yaml" },
  root_markers = { "dbt_project.yml" },
})
vim.lsp.enable("cogitator")
```

### Emacs

First-party integration at [`editors/emacs`](editors/emacs) for **eglot**
(built-in, Emacs 29+) and **lsp-mode**:
`M-x cogitator-run/test/compile/schema/lineage` and the same model-graph
browser (`M-x cogitator-graph`). See `editors/emacs/README.md`:

```elisp
(add-to-list 'load-path "/path/to/dbt-language-server/editors/emacs")
(require 'cogitator)
(require 'cogitator-graph)
(cogitator-setup-eglot)
(add-hook 'sql-mode-hook #'eglot-ensure)
```

### Zed

First-party extension at [`editors/zed`](editors/zed): install it via
`zed: install dev extension`, configure through the standard
`lsp.cogitator` settings (binary path, arguments,
`initialization_options`). The third-party
[zed-dbt](https://github.com/kylejbrk/zed-dbt) extension also works.

### Helix

Add the executable to `$PATH` and to `languages.toml`:

```toml
[language-server.cogitator]
command = "cogitator"

[[language]]
name = "dbt"
scope = "dbt_project.yml"
file-types = ["sql","yml","yaml"]
language-servers = ["cogitator"]
```

## Building and developing

The server is a Cargo workspace:

```
cargo build --release -p cogitator
```

Development goes through the [justfile](justfile): `just check` (format,
clippy with `-D warnings`, all test suites, schema freshness),
`just nvim-test` and `just emacs-test` (headless editor integration tests
against `testdata/jaffle_shop_duckdb`), `just zed-build`,
`just duckdb-check`, `just gen-schemas`. CI runs the Rust suite on Linux,
Windows and macOS; releases cross-compile with cargo-zigbuild.

The whole toolchain is declared in Nix, derived from one source
(`nix/tooling.nix` — add tools there, nowhere else). Three ways in:

```bash
nix-shell          # classic, via shell.nix
```

```bash
devenv shell       # devenv.nix (devenv.sh); `check` runs the CI set
```

```bash
direnv allow       # .envrc picks devenv when installed, else nix-shell
```

Git hooks run through [prek](https://prek.j178.dev)
(`prek install`, config in `.pre-commit-config.yaml`); the changelog is
generated from the conventional-commit history with
[git-cliff](https://git-cliff.org) (`just changelog`, config in
`cliff.toml`).

The feature workflow (trunk-based, conventional commits, squash-merged
PRs) and the release cycle (`just release-prep <version>`, tags start at
v0.5.0) are defined in [CONTRIBUTING.md](CONTRIBUTING.md).

See `AGENTS.md` for the development command reference and
[`specs/rust-port/`](specs/rust-port/) for the implementation specs the
port was written against.
