# cogitator.nvim — implementation spec

A Neovim plugin that drives the `cogitator` dbt language server. This spec is
written against the server as of the repository head; every wire shape quoted
here is ground truth from `crates/cogitator-lsp/src/types/init.rs` and
`crates/cogitator/src/server/dispatcher.rs` — when in doubt, those files win.

Target: **Neovim ≥ 0.11** (`vim.lsp.config` / `vim.lsp.enable`). §10 notes the
0.10 fallback via `nvim-lspconfig`. Language: Lua only, no external deps.

---

## 1. Server contract (what the plugin talks to)

### 1.1 Transport and lifecycle

- stdio, standard `Content-Length` framing. Nothing but LSP frames on stdout.
- `initialize` → `initialized` → normal traffic → `shutdown` → `exit`.
  The server honors `workspaceFolders[0]`, then `rootUri`, then the
  deprecated `rootPath`.
- The server never requests dynamic registration. There is no `$/progress`.
- `exit` terminates the process immediately (exit 0). EOF on stdin is also a
  clean shutdown.

### 1.2 CLI flags the plugin may surface

| flag | meaning |
|---|---|
| `--profiles-dir <dir>` / `-p` | profiles.yml directory (`~` expanded) |
| `--fusion [path]` / `-f` | enable dbt Fusion static analysis; optional binary path (default `dbt`) |
| `--no-tree-sitter` | fall back to the phase-1 minijinja+sqlparser pipeline |
| `--debug` / `-d` | debug log to `log.txt` next to the executable |
| `--otlp [endpoint]` | OTLP trace export |
| `-v` / `--version` | print version and exit |

### 1.3 initializationOptions

```json
{ "profilesDir": "/data/dbt-profiles" }
```

`profilesDir` (also accepted: `profiles_dir`) replaces a `--profiles-dir`
launch flag. Any other shape of `initializationOptions` is ignored, never an
error.

### 1.4 Advertised capabilities (initialize result)

- `textDocumentSync: 2` (incremental)
- `hoverProvider`, `definitionProvider`, `referencesProvider`,
  `documentSymbolProvider`, `workspaceSymbolProvider`,
  `documentHighlightProvider`, `foldingRangeProvider`,
  `selectionRangeProvider`, `renameProvider`: `true`
- `completionProvider: { resolveProvider: true }` — **no trigger characters
  advertised**; see §6.3
- `signatureHelpProvider: { triggerCharacters: ["(", ","] }`
- `semanticTokensProvider`: `full: true`, legend token types (in index
  order): `keyword, function, macro, variable, string, number, operator,
  namespace`; no modifiers
- `diagnosticProvider: { interFileDependencies: false, workspaceDiagnostics: false }`
  (document pull diagnostics)
- `codeLensProvider: { resolveProvider: false }`
- `executeCommandProvider.commands`:
  `dbt.goToSchema`, `dbt.columnLineage`, `dbt.openSchema`, `dbt.showLineage`,
  `dbt.run`, `dbt.test`, `dbt.compile`
- `serverInfo.name == "cogitator"`

### 1.5 Server→client traffic the plugin must handle

- `window/showMessage` — all user feedback, including multi-line
  `Reason: … Solution: …` texts and async dbt run/test/compile outcomes.
- `window/showDocument` (a server→client **request**) — used by
  `dbt.openSchema` and by `dbt compile` success to open the compiled SQL.
  The server ignores the response body, but the client should still reply.
- `textDocument/publishDiagnostics` — push diagnostics while Fusion mode is
  active (pull diagnostics are suppressed server-side in that mode).

### 1.6 workspace/executeCommand argument shape

Every command takes **one argument object**, not positional values:

```json
{ "command": "dbt.run",
  "arguments": [ { "uri": "file:///…/models/customers.sql",
                   "position": { "line": 0, "character": 0 } } ] }
```

`position` is only read by `dbt.goToSchema`. Results:

| command | result | side effect |
|---|---|---|
| `dbt.goToSchema` | `Location` or `null` | none (client jumps) |
| `dbt.openSchema` | `null` | `window/showDocument` or warning |
| `dbt.columnLineage` | lineage JSON or `null` | none |
| `dbt.showLineage` | `null` | formatted `window/showMessage` |
| `dbt.run` / `dbt.test` / `dbt.compile` | `null` (immediately) | async showMessage; compile success opens the compiled SQL via showDocument |

---

## 2. Plugin layout

```
cogitator.nvim/
├── plugin/cogitator.lua          -- guard + user commands (no setup needed)
├── lua/cogitator/
│   ├── init.lua                  -- setup(opts), vim.lsp.config registration
│   ├── config.lua                -- defaults + validation
│   ├── commands.lua              -- :Dbt* user commands → executeCommand
│   ├── lens.lua                  -- codelens refresh autocmds
│   └── health.lua                -- :checkhealth cogitator
└── doc/cogitator.txt
```

Rules: `require("cogitator")` must be cheap (no I/O at require time); all
`vim.fn`/`vim.loop` probing happens in `setup()` or lazily. Every module
≤ ~200 lines.

---

## 3. Configuration surface

```lua
require("cogitator").setup({
  cmd = { "cogitator" },        -- or full path; extra args appended from opts below
  profiles_dir = nil,           -- string → sent as initializationOptions.profilesDir
  fusion = false,               -- false | true | "/path/to/dbt"
  tree_sitter = true,           -- false → --no-tree-sitter
  debug = false,                -- true → --debug
  filetypes = { "sql", "yaml" },
  root_markers = { "dbt_project.yml" },
  attach_outside_project = false, -- see §4.2
  lens = { enabled = true, events = { "BufEnter", "CursorHold", "InsertLeave" } },
  keymaps = true,               -- buffer-local defaults of §5.3
})
```

Validation with `vim.validate`; unknown keys are an error (fail loudly at
setup, never mid-session). `fusion = true` appends `--fusion`;
`fusion = "<path>"` appends `--fusion <path>`. `profiles_dir` goes through
`init_options`, **not** the CLI, so one Neovim instance can serve several
projects with different launch-time settings later.

---

## 4. Server registration and attach

### 4.1 vim.lsp.config (0.11+)

```lua
vim.lsp.config("cogitator", {
  cmd = opts.cmd_with_flags,
  filetypes = opts.filetypes,
  root_markers = opts.root_markers,
  init_options = opts.profiles_dir and { profilesDir = opts.profiles_dir } or nil,
  capabilities = vim.tbl_deep_extend("force", vim.lsp.protocol.make_client_capabilities(), {
    window = { showDocument = { support = true } },
    workspace = {
      workspaceEdit = {
        documentChanges = true,
        resourceOperations = { "rename" },   -- model rename renames the file
      },
    },
  }),
})
vim.lsp.enable("cogitator")
```

### 4.2 Attach discipline

- Attach **only** when a `dbt_project.yml` is found upward from the buffer
  (`root_markers` does this), unless `attach_outside_project = true`. A dbt
  LSP on arbitrary SQL files is noise.
- YAML buffers: attach under the same root; the server serves schema-YAML
  diagnostics and property completion for them.
- One client per root dir — `vim.lsp.enable` already reuses by
  `(name, root_dir)`; do not spawn manually with `vim.lsp.start` in
  addition.

### 4.3 Handlers

- `window/showDocument`: Neovim 0.11 ships a default handler; keep it.
  Verify in an integration test that the schema file opens and the selection
  is applied.
- `window/showMessage`: route through `vim.notify`, mapping
  Error→`vim.log.levels.ERROR`, Warning→`WARN`, Info→`INFO`. The server
  sends multi-line bodies; pass them through unchanged (notifier plugins
  render them fine; `:messages` keeps them greppable).

---

## 5. User commands and keymaps

### 5.1 Commands (`plugin/cogitator.lua`, buffer-aware)

| command | executeCommand | notes |
|---|---|---|
| `:DbtRun` | `dbt.run` | current buffer's uri |
| `:DbtTest` | `dbt.test` | |
| `:DbtCompile` | `dbt.compile` | compiled SQL opens via showDocument |
| `:DbtSchema` | `dbt.goToSchema` | uri + cursor position; jump to the returned `Location` with `vim.lsp.util.show_document` |
| `:DbtLineage` | `dbt.showLineage` | result arrives as a showMessage |

Implementation: `client:exec_cmd({ command = …, arguments = { { uri = vim.uri_from_bufnr(0), position = <cursor> } } }, { bufnr = 0 }, handler)`.
Only `:DbtSchema` needs a result handler; the rest are fire-and-forget
(`null` results). All five must error cleanly ("cogitator is not attached to
this buffer") when no client is attached.

### 5.2 Code lens

- When `lens.enabled`, create an augroup per attached buffer:
  `vim.lsp.codelens.refresh({ bufnr = bufnr })` on the configured events and
  once on attach.
- `vim.lsp.codelens.run()` executes the lens under the cursor; it sends the
  lens's `command.arguments` verbatim — this matches §1.6, no translation
  layer needed.
- **Macro call-site lenses carry `command = ""`** (informational only). The
  plugin must not offer them for `run` (filter in a small wrapper around
  `vim.lsp.codelens.run`, or accept Neovim's no-op).
- Model lenses in display order: `N downstream`, `N upstream`,
  `Go to schema`, `Column lineage`, `dbt run`, `dbt test`, `dbt compile`.

### 5.3 Default buffer-local keymaps (when `keymaps = true`)

A `<leader>d` prefix, deliberately clear of Neovim 0.11's built-in `gr*`
LSP maps: `<leader>ds` → `:DbtSchema`, `<leader>dl` → `:DbtLineage`,
`<leader>dr` → `:DbtRun`, `<leader>dt` → `:DbtTest`, `<leader>dc` →
`:DbtCompile`, `<leader>dL` → `vim.lsp.codelens.run`. All with `desc` set,
all buffer-local on attach only.

---

## 6. Feature-specific client notes

### 6.1 Rename (`textDocument/rename`)

The server returns one `WorkspaceEdit` with `documentChanges`: text edits in
dependent files, the properties-YAML name edit, and a `RenameFile` **last**.
The plugin must advertise the capabilities in §4.1 or the file rename is
silently dropped by the server-agnostic client code. Caveat to document in
`:h cogitator`: dependents are read from disk, so unsaved changes in *other*
buffers are not seen — save before renaming.

### 6.2 Diagnostics

- Pull: Neovim 0.11 drives `textDocument/diagnostic` automatically from
  `diagnosticProvider`. Nothing to implement; dangling-`ref` and YAML schema
  errors arrive this way.
- Push: with `fusion` enabled the server publishes compile diagnostics and
  suppresses the static ones. Both paths land in `vim.diagnostic`; no merge
  logic in the plugin.

### 6.3 Completion

- No trigger characters are advertised: completion is grammar-driven
  server-side. The plugin should set
  `triggerCharacters = { ".", "'", '"', " ", "(" }` on the client's
  `server_capabilities.completionProvider` **after attach** so nvim-cmp /
  blink.cmp / native `vim.lsp.completion` fire in member-access
  (`alias.`), `ref('`, and post-keyword positions. (Client-side override;
  revisit if the server grows `triggerCharacters`.)
- Ranking arrives via `sortText` tiers (`0` < `a` < `b` < `c` < `m` < `z`);
  completion engines must be left on `sortText` ordering (document that
  `cmp.config.compare.sort_text` has to stay in the comparator list).
- `completionItem/resolve` fills documentation — enable
  `resolveSupport` (default in 0.11).
- `insertText` is plain text (no snippet syntax). Do not advertise
  `snippetSupport`-dependent behavior.

### 6.4 Semantic tokens

Automatic in 0.11. Provide optional highlight links for the legend's
non-standard entries, e.g. `@lsp.type.macro.sql → @function.macro`, and a
`semantic_tokens = false` escape hatch
(`vim.lsp.semantic_tokens.enable(false, { bufnr })`).

### 6.5 Signature help

Nothing custom: triggers `(` and `,` come from the server; structured macro
parameters render through the default handler.

---

## 7. Health check (`:checkhealth cogitator`)

Report, in order:

1. `cogitator` binary found on `PATH` (or configured `cmd[1]`) + its
   `--version` output.
2. Neovim version ≥ 0.11 (else point at §10 fallback).
3. For the current buffer/cwd: whether `dbt_project.yml` was found and where.
4. Profiles resolution preview: list the candidate paths in server order
   (§1.3 of the README: override → `$DBT_PROFILES_DIR` → `<root>/profiles.yml`
   → `<root>/.dbt/profiles.yml` → `~/.dbt/profiles.yml`) and mark the first
   that exists. (Duplicated client-side knowingly; keep in sync with
   `util::profiles_candidates`.)
5. If `fusion` is configured: the fusion binary exists and `--version`
   mentions `dbt-fusion` (mirror of the server's own validation).

---

## 8. Testing

- Unit (plenary busted): config validation, CLI-flag assembly from opts,
  command-argument construction (uri + position shapes of §1.6).
- Integration (headless `nvim -l`): against `testdata/jaffle_shop_duckdb`
  from this repository with a locally built `target/debug/cogitator`:
  1. attach on `models/customers.sql`, wait for the first context refresh,
  2. hover over a `ref` returns contents,
  3. `:DbtSchema` jumps to `schema.yml`,
  4. codelens list contains `… downstream` and `dbt run`,
  5. rename of a model produces a `RenameFile` documentChange.
- CI: one ubuntu job installing neovim stable + `cargo build -p cogitator`,
  running both suites.

---

## 9. Non-goals (v1)

- No `$/progress` UI (server does not send it); dbt run feedback stays
  showMessage-based.
- No multi-root workspaces (server reads `workspaceFolders[0]` only).
- No file watcher: the server refreshes on `didSave`/`didOpen`, and
  `workspace/didChangeWatchedFiles` is accepted but never registered for.
- No install/download of the server binary — health check points at the
  release assets (`cogitator-{linux-amd64,darwin-amd64,darwin-arm64,windows-amd64.exe}`)
  and `cargo build`.

## 10. Neovim 0.10 fallback

Ship a one-paragraph recipe in `doc/cogitator.txt` using `nvim-lspconfig`'s
custom-config API with the same `cmd`/`root_dir`/`init_options`/capabilities
tables; the plugin's own modules must not hard-require 0.11 APIs outside the
`vim.lsp.config` call (guard with `vim.fn.has("nvim-0.11")`).
