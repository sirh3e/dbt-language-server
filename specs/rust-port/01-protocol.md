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
(no error response). **No request ever gets a JSON-RPC error object** — keep that.

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

- `initialized` notification from the client: ignored (falls through dispatch).

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
