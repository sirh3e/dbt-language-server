# 01 — Wire Protocol, Message Types, Dispatch & Lifecycle

Source files: `rpc/rpc.go`, `lsp/*.go`, `main.go` (dispatch), `util/write_response.go`.

## 1. Base protocol framing (`rpc`)

LSP base protocol over stdio. Messages are framed as:

```
Content-Length: <n>\r\n
\r\n
<n bytes of JSON>
```

### 1.1 Encode

```
encode_message(msg: impl Serialize) -> String
  content = serde_json::to_string(msg)        // panic/abort on failure (Go panics)
  format!("Content-Length: {}\r\n\r\n{}", content.len(), content)
```

`content.len()` is the **byte** length of the JSON.

### 1.2 Decode

```
decode_message(msg: &[u8]) -> Result<(method: String, content: &[u8])>
```

1. Split on the first `\r\n\r\n` into `header` / `rest`. Error `"Did not find separator"` if absent.
2. `content_length = header["Content-Length: ".len()..].parse::<usize>()?`
   **[Q10]** No header scanning: the header block is assumed to be exactly one
   `Content-Length: ` header. **[DEVIATION — recommended]** parse headers properly
   (split on `\r\n`, find `Content-Length`, ignore others, case-insensitive name).
3. `content = &rest[..content_length]`.
4. Deserialize only `{"method": string}` from `content` (`BaseMessage`); return
   `(method, content)`. The full message is re-deserialized per method in the dispatcher.

### 1.3 Stream splitting

Go uses a `bufio.Scanner` with a custom split function; semantics to reproduce with a
buffered reader loop:

- Accumulate bytes until the buffer contains `\r\n\r\n` **and** at least
  `content_length` bytes after it; otherwise read more.
- One complete frame = `header + \r\n\r\n + content_length bytes`; consume exactly that.
- A malformed `Content-Length` value aborts the read loop (scanner error → Go exits the loop
  and the process ends). An EOF on stdin also ends the loop → process exits 0.

Illustrative shape (the binding implementation is the async reader task over
`tokio::io::stdin` with a reused buffer, 05 §6.2 / 08 §1.2 — same semantics):

```rust
fn read_frame(..., buf: &mut Vec<u8>) -> io::Result<Option<Frame>>
// None on clean EOF; frame bytes include the header (decode_message re-splits).
```

## 2. JSON types

All structs serialize with exactly these JSON field names. Go omits nothing
(no `omitempty` anywhere): **every field is always present** in output, including
empty strings, `0`, `null` ids and empty arrays. Keep that for parity
(i.e. don't use `skip_serializing_if`).

### 2.1 Base

```jsonc
// Request (incoming)
{ "jsonrpc": "2.0", "id": 1, "method": "..." , "params": {...} }

// Response (outgoing) — base part
{ "jsonrpc": "2.0", "id": 1, ... }        // id is *int; serialized as number or null

// Notification (both directions)
{ "jsonrpc": "2.0", "method": "..." , "params": {...} }
```

Rust:

```rust
#[derive(Serialize)]
struct Response { jsonrpc: &'static str /* "2.0" */, id: Option<i64> }
```

Note: incoming `id` is deserialized as Go `int` — clients that send string ids are not
supported (keep, or widen to `serde_json::Value` as a deviation).

### 2.2 Common structures

```jsonc
Position: { "line": int, "character": int }         // 0-based
Range:    { "start": Position, "end": Position }
Location: { "uri": string, "range": Range }
TextDocumentIdentifier: { "uri": string }
VersionedTextDocumentIdentifier: { "uri": string, "version": int }
TextDocumentItem: { "uri": string, "languageId": string, "version": int, "text": string }
TextDocumentPositionParams: { "textDocument": TextDocumentIdentifier, "position": Position }
```

### 2.3 Per-method payloads

| Method | Params (incoming) | Result (outgoing) |
|---|---|---|
| `initialize` | `{ clientInfo: {name, version}, rootPath: string }` (only these fields read) | see §3.1 |
| `textDocument/didOpen` | `{ textDocument: TextDocumentItem }` | — |
| `textDocument/didChange` | `{ textDocument: VersionedTextDocumentIdentifier, contentChanges: [{ range: Range, rangeLength: int, text: string }] }` | — |
| `textDocument/didSave` | `{ textDocument: TextDocumentIdentifier }` | — |
| `textDocument/hover` | `TextDocumentPositionParams` | `{ "contents": string }` |
| `textDocument/definition` | `TextDocumentPositionParams` | `Location` |
| `textDocument/completion` | `TextDocumentPositionParams` | `[CompletionItem]` |
| `workspace/executeCommand` | `{ command: string, arguments: [any] }` | `Location` or `null` |
| `shutdown` | (id only) | *(absent — see §3.3)* |
| `exit` | — | — (process exits 0) |

`CompletionItem` (all fields always serialized):

```jsonc
{ "label": string, "detail": string, "documentation": string,
  "kind": int, "insertText": string, "sortText": string }
```

Completion kinds used: `Reference = 18`, `Snippet = 15`, `Variable = 6`, `Function = 3`
(full LSP table 1..=25 in `lsp/completionKind` — port as consts or an enum).

Diagnostics push notification:

```jsonc
{ "jsonrpc": "2.0", "method": "textDocument/publishDiagnostics",
  "params": { "uri": string, "diagnostics": [
      { "range": Range, "message": string, "severity": int, "code": string, "source": string }
  ]}}
```

Severity consts: Error=1, Warning=2, Info=3, Hint=4.

**Non-standard shapes to be aware of** (clients tolerate them today; `lsp-types`
would change them):

- Hover result is `{"contents": "<plain string>"}` — a bare string, not MarkupContent.
  The strings contain markdown (code fences, links); most editors render it anyway.
- Definition always returns a `Location` object — when nothing is found it returns the
  *request's own* uri/position (a zero-width range at the cursor), never `null`.
- `executeCommand` result is `null` or a `Location` (client-side code in the Zed/editor
  extension opens it).

## 3. Lifecycle & dispatch (`main.go` `handleMessage`)

Single loop: read frame → `decode_message` → `match method`. Unknown methods are
silently ignored. JSON parse errors per message are logged and the message dropped
- Error objects exist, narrowly: `MethodNotFound` (-32601) for unknown
  requests, `InvalidParams` (-32602) for undecodable request params, and
  `RequestCancelled` (-32800) for `$/cancelRequest`-aborted in-flight work.
  Notifications never get responses. (The original "no error objects" rule
  predated these.)

### 3.1 `initialize`

- Store `params.rootPath` into `state.lsp_client_root_path` (note: `rootPath` is the
  deprecated LSP field; `rootUri`/`workspaceFolders` are ignored).
- Reply (exact result):

```jsonc
{
  "capabilities": {
    "textDocumentSync": 2,
    "hoverProvider": true,
    "definitionProvider": true,
    "completionProvider": {},
    "executeCommandProvider": { "commands": ["dbt.goToSchema"] }
  },
  "serverInfo": { "name": "dbt-language-server", "version": "<version>" }
}
```

- `initialized` notification from the client: acknowledged as a no-op.

> **Post-parity note:** the Rust server extends this response with additive
> capabilities beyond the Go shape (references, document/workspace symbols,
> document highlight, folding/selection ranges, semantic tokens, signature
> help, pull diagnostics, `completionProvider.resolveProvider`) — see
> FEATURE_REQUESTS.md. The Go-era fields above are unchanged.

Ordering quirk: Go writes the initialize response *before* assigning
`LspClientRootPath` — irrelevant semantically (same thread), assign in any order.

### 3.2 Document sync

- `didOpen`: `state.open_document(uri, text)` → refresh dbt context (full project
  rescan, see 03) then parse the document. Then `fusion_compile(...)` (04).
- `didSave`: `state.save_document(uri)` → refresh dbt context only (document text is
  *not* re-read; the server trusts its didChange-maintained copy). Then `fusion_compile`.
- `didChange`: apply all content changes in order to the stored text, then re-parse
  the document (algorithm in 03 §2). No context refresh, no fusion.

### 3.3 `shutdown` / `exit`

- `shutdown`: reply with the bare response `{"jsonrpc":"2.0","id":<id>}` —
  **no `result` field at all** (Go struct has none). Standard LSP expects
  `"result": null`; clients accept the Go form. `lsp-types` would emit `result: null` —
  acceptable deviation.
- `exit`: `std::process::exit(0)` immediately, regardless of whether shutdown was called.

### 3.4 `workspace/executeCommand`

Only command: `dbt.goToSchema`. Arguments arrive as untyped JSON:
`arguments[0]` is an object `{ "uri": string, "position": { "line": number, "character": number } }`.
Numbers arrive as f64 → cast to int. If `arguments` is empty or `arguments[0]` is not an
object, do nothing (no response at all — the request is dropped). Missing keys default
to `""` / `0` (Go's ignored type-assertion errors). Then `state.go_to_schema(id, uri, position)`
and write the response.

## 4. Writing responses

`write_response(writer, msg)` = `writer.write_all(encode_message(msg).as_bytes())`.
Errors ignored in Go; in Rust at minimum log them. All writes go to stdout. If the
fusion compile runs on a background thread (deviation), wrap stdout in
`Mutex<StdoutLock>` so frames can't interleave.

## 5. Logging

- `--debug/-d`: log to `log.txt` **in the directory of the server executable**
  (not CWD), created/appended with prefix `[dbt-lsp]` + date/time/file:line.
- Without the flag: all logging discarded.
- Never log to stdout (it would corrupt the LSP stream); Go's logger writes to the file only.
  **[DEVIATION — optional]** also/instead log to stderr; LSP clients capture stderr.

Rust: implemented as the `tracing` fmt layer described in 05 §6.3 (same `--debug`
file semantics; stderr is just another optional layer; OTLP export is opt-in and
separate). Keep the flag semantics.

## 6. Feature-batch wire extensions (binding)

Amendments for the spec.md feature batch; these supersede the narrower
shapes above where they overlap.

### 6.1 Diagnostic

`Diagnostic` gains three optional fields (omitted from the wire when
`None`): `tags: Option<Vec<u8>>` (1 = Unnecessary, 2 = Deprecated),
`relatedInformation: Option<Vec<DiagnosticRelatedInformation>>`, and
`data: Option<DiagnosticData>`. `DiagnosticData` is a TYPED payload —
`{ fix: "replace_token" | "insert_text", tokenRange: Range,
suggestions: [String] }` — that rides did-you-mean suggestions to the
client and back (clients must preserve `data` per LSP). `Diagnostic`
therefore derives Deserialize as well; the struct is constructed
through `Diagnostic::plain(..)` where no extras apply.

### 6.2 Code actions

`CodeAction` becomes `{ title, kind?, diagnostics?, edit?:
WorkspaceEdit, command? }`. Quickfixes carry `kind: "quickfix"` plus
the `edit`; the executable-lens actions stay command-only with no
kind. `CodeActionContext.diagnostics` is now deserialized; a non-empty
`only` filters by kind prefix instead of returning nothing.
**`codeActionProvider` stays a plain `true`** — `codeActionKinds` is
optional in LSP and clients request fine without it.

### 6.3 New server capabilities

`inlayHintProvider: true`, `documentLinkProvider: { resolveProvider:
false }`, `linkedEditingRangeProvider: true`,
`documentFormattingProvider: true`, and `diagnosticProvider.
workspaceDiagnostics: true`. **`interFileDependencies` stays `false`**:
setting it true invites clients to re-pull every open document on every
change; cross-file truth travels through `workspace/diagnostic`.

### 6.4 workspace/diagnostic result ids

One global `context_revision` counter (bumped on every context rebuild)
is the `resultId` for every item. A `previousResultId` equal to the
current revision answers `kind: "unchanged"`; anything else answers a
full report. Stateless and always correct — a full report is a legal
response at any time. Per-file stamps were rejected: graph-derived
diagnostics for file B change when file A changes.

### 6.5 $/setTrace and the log stream

`$/setTrace {value}` and the `trace` field of initialize are handled
(05 §6.3 addendum): they tune the window/logMessage verbosity.
`$/logTrace` is not emitted by decision.

## Addendum: prepareRename and the macro call hierarchy

- `renameProvider` advertises `{prepareProvider: true}`:
  `textDocument/prepareRename` answers `{range, placeholder}` on a
  `ref('…')` literal and null anywhere else, so clients never open a
  rename box the server would ignore.
- `callHierarchyProvider: true`: `textDocument/prepareCallHierarchy`
  answers one Function-kind item for the macro call under the cursor
  (resolution order 03 §5, own project first), null otherwise.
  `callHierarchy/incomingCalls` walks the graph's MacroCall edges and
  re-reads each caller from disk for the exact call-token ranges
  (stale edges whose call vanished are dropped). `outgoingCalls` is
  deliberately empty — the macro hierarchy is one level deep today,
  and clients probe both directions.

## Addendum: multi-root workspaces

The FIRST workspace folder is the primary root, exactly as before:
its context lives on the watch channel and feeds every project-wide
surface — workspace/symbol, workspace/diagnostic, the cogitator.yml
watcher, dbt.status's project block, the refresh degradation
warnings. Every OTHER dbt project gets a secondary context in a side
map, keyed by resolved project root:

- initialize refreshes each further `workspaceFolders` entry;
- didOpen of a file outside every known root sends the file's
  directory, which the refresh task resolves upward to
  dbt_project.yml — foreign projects join lazily;
- per-URI dispatch (snapshot_for, push diagnostics, dialect choice,
  dbt run/test/compile/preview, fusion) picks the covering context by
  longest project-root prefix, compared as Path components (08 §1).
  The position-shaped request handlers route the same way — hover,
  completion, references and their respond_position siblings, the
  call-hierarchy calls (by the item's uri), dbt.modelGraph and
  dbt.goToSchema — never the primary snapshot. Requests with no URI
  (workspace/symbol, workspace/diagnostic, completionItem/resolve,
  dbt.deadModels, dbt.projectGraph) stay on the primary context;
- the refresh task debounces per DISTINCT root and keeps one
  ContextBuilder per secondary root (same scan-cache warm start and
  persistence; no watch publish, no user-facing warnings);
- dbt.status reports the live secondary roots as
  `project.extraRoots`.

Deliberate limits, documented here: profiles overrides (--target,
dbt.setTarget, cogitator.yml) apply globally to every root, and the
config watcher reads the primary root's cogitator.yml only.
