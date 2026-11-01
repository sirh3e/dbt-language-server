# Feature Requests — Missing LSP Features

Status of the Rust implementation (`crates/`) against the LSP specification.
The server currently implements exactly the original Go server's surface:

| Implemented today | Notes |
|---|---|
| `initialize` / `shutdown` / `exit` | capabilities per `specs/rust-port/01-protocol.md` §3.1 |
| `textDocument/didOpen` / `didChange` / `didSave` | incremental sync (`textDocumentSync: 2`) |
| `textDocument/hover` | refs, sources, vars, macros, dialect SQL functions |
| `textDocument/definition` | models, sources/tables, vars, macros, in-file CTEs |
| `textDocument/completion` | context-aware inside `ref('` / `source('` / `var('` / `{{` |
| `workspace/executeCommand` → `dbt.goToSchema` | jump to the model's properties-YAML entry |
| `textDocument/publishDiagnostics` (push) | dbt Fusion compile only (`--fusion`) |

Everything below is missing. Ordered by suggested priority; "building block"
names what already exists in the codebase.

## P1 — cheap wins, real gaps

- [ ] **`textDocument/didClose`** — closed documents currently stay in the
      `Documents` store forever (leak inherited from the Go server).
      Building block: `Documents` (`crates/dbt-ls-analysis/src/lib.rs`) just
      needs a `close(uri)` + dispatch arm.
- [ ] **Wire DuckDB parse diagnostics into the dispatcher** — the masking,
      confidence rule and `json_serialize_sql` engine exist
      (`crates/dbt-ls-duckdb`, feature `duckdb-parser`, spec 07 §3) but are
      not yet called on didOpen/didSave for duckdb projects.
- [ ] **`textDocument/references`** — find all models referencing a model.
      Building block: the petgraph dependency graph already answers it
      (`ProjectGraph::dependents_of`, `crates/dbt-ls-analysis/src/graph.rs`);
      only dispatch + response wiring is missing.
- [ ] **Dangling-ref diagnostics** — warn on `ref('nonexistent')` without
      Fusion. Building block: `ProjectGraph::dangling_refs` (spec 05 §5).
- [ ] **`initialized` notification** — currently ignored; handle explicitly.

## P2 — quality of life

- [ ] **`completionItem/resolve`** — send lean completion items, resolve
      documentation lazily (today every item carries full docs).
- [ ] **`$/cancelRequest`** — ignored today; matters once any request gets
      slower than the current sub-millisecond handlers.
- [ ] **`textDocument/signatureHelp`** for macros — feasible once the
      minijinja-parsed macro parameters land (spec 06 §2.2 enrichment:
      `MacroParam { name, default }`).
- [ ] **`workspace/didChangeWatchedFiles`** — file watching as the context
      refresh trigger instead of the didOpen/didSave heuristic
      (pairs with the incremental-refresh fast path, spec 05 §6.2).
- [ ] **Proper JSON-RPC error responses** — malformed requests are silently
      dropped (faithful to Go); unknown methods should answer
      `MethodNotFound`, bad params `InvalidParams`.

## P3 — planned phases (already specced)

- [ ] **Tree-sitter batch** (spec 06 §3.6, after the tree-sitter phase):
      - `textDocument/semanticTokens`
      - `textDocument/foldingRange`
      - `textDocument/documentSymbol`
      - `textDocument/selectionRange`
- [ ] **dbt YAML intelligence** (spec 08 §2.6, building block:
      `schemas/dbt/` + typify types):
      - schema-validation diagnostics for `dbt_project.yml` / properties files
      - field-name & enum completion inside dbt `.yml` files
- [ ] **Pull diagnostics** (`textDocument/diagnostic`,
      `workspace/diagnostic`) — once more diagnostic sources exist.
- [ ] **AST-driven features for duckdb projects** (spec 07 §4,
      `json_serialize_sql` AST): column-name completion, AST-based graph
      edges, better CTE information.

## P4 — not planned / needs a Jinja renderer or major new scope

- `textDocument/rename` / `prepareRename` (cross-file model rename; the
  graph makes it plausible, but write-side edits are a new scope)
- `textDocument/formatting` / `rangeFormatting` / `onTypeFormatting`
  (would mean embedding a SQL formatter)
- `textDocument/codeAction`, `codeLens`, `documentHighlight`,
  `inlayHint`, `inlineValue`, `documentLink`, `linkedEditingRange`,
  `documentColor`
- `textDocument/typeDefinition` / `implementation` / `declaration`
  (little meaning for dbt sources)
- Call/type hierarchy (could model macro call chains someday)
- `workspace/symbol`, workspace folders / multi-root (only the deprecated
  `rootPath` is read), `workspace/didChangeConfiguration`,
  `workspace/applyEdit`, file-operation notifications, `$/progress`
