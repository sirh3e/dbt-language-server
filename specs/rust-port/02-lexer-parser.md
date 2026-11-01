# 02 — Lexer, Parser & Token Index

Source files: `analysis/parser/token.go`, `lexer.go`, `parser.go`.
Reference tests to port verbatim: `analysis/parser/lexer_test.go`, `parser_test.go`.

> **Implementation note (05 §4):** the Rust port produces tokens via
> `minijinja::machinery::tokenize` (Jinja layer) + `sqlparser`'s dialect-aware
> tokenizer (SQL layer) instead of re-implementing the Go byte lexer (§2) and
> keyword tables (§1.2) — no hand-written lexer at all. This file remains the *behavioral*
> reference: which identifiers end up as REF/VAR/SOURCE/SOURCE_TABLE/MACRO/PACKAGE
> at which positions (§3), the CTE map, the TokenIndex and cursor/lookback
> semantics (§3.4–3.6) are unchanged and testable as specified. Port the Go test
> tables as behavior tests (refined kinds + positions), not raw token-stream dumps.

The "parser" is not an AST parser. It is a second pass over the lexer's token stream
that (a) re-types identifiers in dbt call positions (`ref('x')` → the `x` token gets
type `REF`, etc.), (b) collects CTE names for in-file go-to-definition, and
(c) builds a line-indexed structure for cursor lookups.

## 1. Token

```rust
struct Token {
    r#type: TokenType,
    literal: String,
    line: usize,    // 0-based
    column: usize,  // 0-based, BYTE offset within the line (see Q6)
}
```

> **Implementation note (08 §1.1):** the Rust port does not store `literal` —
> tokens carry a byte-span into the document's `Arc<str>` and the text is
> resolved on demand (zero-copy). Everywhere this file says "literal", read
> "the span's slice"; lengths, comparisons and map lookups are unchanged.

`TokenType` in Go is a string; symbolic token types use their literal text as the value
(`"("`, `"{{"`, …) and word types use names (`"IDENT"`, `"REF"`, …). In Rust use an enum;
nothing serializes token types.

### 1.1 Token type inventory

Structural: `ILLEGAL, EOF, IDENT, INT, SINGLE_QUOTE('), DOUBLE_QUOTE("), BACKTICK(`),
EQUAL(=), PLUS(+), MINUS(-), BANG(!), ASTERISK(*), SLASH(/), PERCENT(%), LT(<), GT(>),
NOT_EQ(!=), LT_EQ(<=), GT_EQ(>=), COMMA(,), SEMICOLON(;), DOT(.), LPAREN((), RPAREN()),
LBRACE({), RBRACE(})`.

dbt/Jinja: `DB_LBRACE({{), DB_RBRACE(}}), JINJA_LBRACE({%), JINJA_RBRACE(%}),
REF, VAR, SOURCE, SOURCE_TABLE, MACRO, PACKAGE, CONFIG`.

SQL keywords: one variant per keyword — the full list is in `token.go` lines 63–194
(ACCOUNT…WITH, incl. oddities like `MINUS_KW`, `PIVOT_LONGER`, `GSCLUSTER`). Copy the
list mechanically.

### 1.2 Keyword lookup

```
lookup_ident(ident: &str, dialect: Dialect) -> TokenType
```

1. Pick base keyword table by dialect: `"snowflake"` → snowflake table,
   `"duckdb"` → duckdb table, anything else → empty table.
   (Tables: `token.go` lines 196–366; copy verbatim. Keys are lowercase;
   lookup is **case-sensitive** — `SELECT` in caps lexes as `IDENT`. Keep.)
2. Overlay dbt keywords (always, any dialect): `"ref"→REF, "var"→VAR,
   "source"→SOURCE, "config"→CONFIG`.
3. Hit → that type; miss → `IDENT`.

Superseded mechanism (05 §4): keyword classification comes from sqlparser's
`Token::Word { keyword }` per dialect, not ported tables. The tables above remain
reference-only; the dbt overlay (`ref`/`var`/`source`/`config`) survives as part
of the refinement pass. (Go also rebuilds/overlays a map on every lookup — pure
waste; irrelevant once sqlparser classifies.)

## 2. Lexer

Byte-oriented over the input string. State: current byte `ch` (0 = EOF sentinel),
`line` (starts 0), `column` (starts **-1**; incremented on every byte read, so the
first byte is column 0). Dialect only feeds `lookup_ident`.

**[Q6]** Columns are byte offsets and `is_letter` is ASCII-only. Multibyte UTF-8 in a
line shifts all following columns relative to the editor's UTF-16 positions, and
non-ASCII identifier bytes lex as `ILLEGAL`. Port as-is for parity; a later fix must
convert positions at the LSP boundary.

```
next_token():
  skip_whitespace()         # ' ' '\t' '\n' '\r'; on '\n' or '\r': line += 1, column = -1
  match ch:
    '='  => EQUAL           ';' => SEMICOLON    '(' => LPAREN   ')' => RPAREN
    ','  => COMMA           '.' => DOT          '+' => PLUS     '-' => MINUS
    '!'  => two_char('=', NOT_EQ, BANG)
    '/'  => SLASH           '*' => ASTERISK
    '<'  => two_char('=', LT_EQ, LT)
    '>'  => two_char('=', GT_EQ, GT)
    '{'  => peek '%' ? two_char('%', JINJA_LBRACE, LBRACE)
          : peek '{' ? two_char('{', DB_LBRACE, LBRACE)
          : LBRACE
    '}'  => two_char('}', DB_RBRACE, RBRACE)
    '\'' => SINGLE_QUOTE    '"' => DOUBLE_QUOTE  '`' => BACKTICK
    '%'  => two_char('}', JINJA_RBRACE, PERCENT)
    0    => EOF (literal "")
    letter (a-z A-Z _) => read_identifier(); type = lookup_ident(lit, dialect); RETURN (no extra advance)
    digit => read_number(); type = INT; RETURN (no extra advance)
    else  => ILLEGAL (single byte)
  read_char()   # single/two-char tokens consume their last byte here
  return tok
```

Details:

- Single-char tokens: `literal` = that byte, `line`/`column` = at that byte.
- `two_char(next, two_type, one_type)`: if `peek == next`, token literal is both bytes,
  `column` = **column of the first byte** (Go records `l.column - 1` after consuming);
  else fall back to single-char token.
- Identifiers: `[A-Za-z_][A-Za-z0-9_]*`; `line`/`column` recorded at the first byte.
- Numbers: `[0-9]+` only (no decimals/floats — `1.5` lexes as INT, DOT, INT).
- **Strings are not lexed as units**: `'customers'` → `SINGLE_QUOTE, IDENT(customers),
  SINGLE_QUOTE`. The parser pass depends on this.
- There is no comment handling: `-- line comment`, `/* block */`, and `{# jinja #}`
  are all lexed as ordinary tokens. Keep (comments only pollute the index; hover just
  finds tokens there).
- Newlines inside the *unreached* `two_char` peek are impossible (whitespace skipped
  first), so no line-tracking edge cases there.

## 3. Parser pass

### 3.1 Data structures

```rust
struct Parser {
    tokens: Vec<TokenNode>,   // all tokens in document order (EOF excluded, see below)
    ctes: CteState,
}
struct TokenNode { token: Token, prev: Option<usize> }  // [Q11] index, not pointer
struct CteState {
    active: bool,         // Go: Ind
    paren_count: i32,     // starts -1
    tokens: Vec<Token>,   // CTE name tokens (IDENT at definition site)
}
```

Go keeps `curTok`/`peekTok` with one-token lookahead. `next_token()` on the parser:
append the *current* token (if its type is non-empty, i.e. not the initial blank) to
`tokens` with `prev` = previous index, then shift: `cur = peek; peek = lexer.next()`.
Because appending happens when the token is *left*, the final EOF token is never
appended (loop exits when `cur == EOF` before shifting again) — the `tokens` vec
contains every real token exactly once, in order.

### 3.2 Main loop (`parse_tokens`)

Start: parser is constructed with blank cur/peek, so call `next_token()` twice
implicitly via the loop's structure — in Go, `Parse()` → `parseTokens()` loops
`for cur.type != EOF`, and the *first* iteration runs with blank `curTok` falling
through the match to the trailing `next_token()`. Easiest faithful port: prime with
two `next_token()` calls, then loop; verify against `parser_test.go` token streams.

```
loop while cur.type != EOF:
  match cur.type:
    WITH        => parse_with()
    LPAREN      => inc_paren()
    RPAREN      => dec_paren(); then if ctes.active && paren_count == 0:
                     next();                       # move past RPAREN
                     if cur == COMMA { next(); if cur == IDENT { ctes.tokens.push(cur) } }
                     else { ctes.active = false }
    SOURCE      => parse_source()
    REF         => parse_ref()
    VAR         => parse_var()
    DB_LBRACE   => if peek == CONFIG { next(); parse_config() }
                   else if peek == IDENT { next(); parse_macro() }
  next_token()
```

`inc_paren`/`dec_paren` only mutate `paren_count` when `ctes.active`.

### 3.3 Sub-parsers (exact token patterns)

All "retype" operations mutate `cur.token.type` **before** it is appended to `tokens`.

- `parse_with`: after `WITH`, if cur is `IDENT`: mark `ctes.active = true`, push the
  IDENT into `ctes.tokens`; if peek is `AS` consume it; if (new) peek is `LPAREN` set
  `paren_count = 1`; consume one more token. (Matches `with name as (` — the LPAREN
  itself is consumed here without passing through the LPAREN arm.)
- `parse_ref`: pattern `REF LPAREN ('|") IDENT` → retype the IDENT to `REF`.
  (`inc_paren` is called for the LPAREN.) Both quote types accepted. If any step
  mismatches, stop silently (tokens keep their lexed types).
- `parse_var`: same shape, retypes to `VAR`.
- `parse_source`: pattern
  `SOURCE LPAREN ('|") IDENT ('|") COMMA ('|") IDENT` —
  first IDENT → `SOURCE`, second IDENT → `SOURCE_TABLE`.
  (No whitespace sensitivity; quotes may mix styles.)
- `parse_macro` (entered on `{{ IDENT`):
  - if peek is `DOT`: retype cur IDENT → `PACKAGE`, advance twice (consume DOT, land
    on the next token);
  - then if cur is `IDENT` and peek is `LPAREN`: retype cur → `MACRO`.
  So `{{ pkg.macro(...) }}` yields `PACKAGE(pkg) DOT MACRO(macro)`, and
  `{{ macro(...) }}` yields `MACRO(macro)`. `{{ some_var }}` (no parens) stays IDENT.
- `parse_config` (entered on `{{ CONFIG`): advance; if cur is LPAREN, `inc_paren()`.
  Nothing else (config args parse as normal tokens).

### 3.4 Outputs

- `create_token_name_map() -> HashMap<String, Token>`: CTE literal → its definition
  token (last definition wins). Used for in-file go-to-definition (`DefTokens`).
- `create_token_index() -> TokenIndex`:

```rust
struct TokenIndex { line_tokens: HashMap<usize, Vec<TokenNode>> }
// Go clones the nodes per line; in Rust prefer HashMap<usize, Vec<usize>> into a
// shared Vec<TokenNode> owned by TokenIndex — keeps prev-chain walking trivial.
```

Per line, tokens are in ascending column order (append order).

### 3.5 Cursor lookup

```
find_token_at_cursor(line, column) -> Result<&TokenNode>
```

1. `line_tokens.get(&line)` else error ("line does not exist").
2. Binary search (`partition_point`) for the first token with
   `token.column + token.literal.len() > column`.
3. Hit iff `idx < len && column >= tok.column && column < tok.column + tok.literal.len()`
   — i.e. cursor strictly inside the token's byte span (a cursor at the position
   immediately after the last char does not match). Else error ("token does not exist").

`literal.len()` is byte length (EOF literal "" never appears in the index).

### 3.6 Look-back matching

```
token_lookback_match(node, expected: TokenType, inc: usize) -> Option<&str>
```

Walk `prev` exactly `inc` steps (inc ≤ 0 → None; any None link on the way → None);
if the landed token's type == expected, return its literal.

Used by features (03): from a `SOURCE_TABLE` token, `(SOURCE, 4)` reaches the source
name through `" , " name` (chain: quote, comma, quote, SOURCE); from a `MACRO` token,
`(PACKAGE, 2)` reaches the package through the DOT.

## 4. Test fixtures

`parser_test.go` contains exact expected token streams for representative dbt SQL
(CTEs, refs, sources, vars, macros with/without packages, config blocks). Port those
tables 1:1 — they are the executable spec for §3. Same for `lexer_test.go` (symbols,
line/column accounting).
