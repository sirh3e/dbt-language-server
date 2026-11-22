---
name: editor-parity
description: Expose every new or changed dbt.* command in every editor in the same change, so the server and its consumers (Neovim, Emacs, VS Code, Zed) stay in sync. Use whenever adding, renaming or removing a workspace/executeCommand command (cogitator_lsp::COMMANDS), adding a code action or code lens that carries a command, or changing anything under editors/. A test (crates/cogitator-lsp/tests/consumer_parity.rs) fails while a command is unclassified or an editor misses it, so the change is not done until it passes.
---

# Editor parity

The server is the product; the editors are thin clients of it. A feature a
person reaches through a `dbt.*` command exists for a user only once their
editor wraps it. **Code actions and code lenses need no wrapping** — every
LSP client runs them — so the rule is about commands.

## The rule

1. Every command the server registers (`COMMANDS` in
   `crates/cogitator-lsp/src/types/init.rs`) is classified in
   `editors/parity.toml`:
   - `kind = "user"`: a person runs it. **Neovim, Emacs and VS Code all wrap
     it, in the same change.**
   - `kind = "internal"`: the server only sends it inside a code lens or code
     action (`dbt.openSchema`, `dbt.goToTests`, …). Say where with `via`.
2. Zed cannot add commands (an extension has no command surface). It gets code
   actions and lenses for free; its README says that the rest needs an
   executeCommand client. Name a headline feature there when it arrives as a
   code action.
3. `[gaps.<editor>]` holds what was missing when the rule was adopted
   (2026-11-22). **It only shrinks.** Never add a line for a new command; when
   you wrap a listed one, delete its line (the test tells you to).
4. Every command has a row in `specs/neovim-plugin/00-plugin.md`'s command
   table: its argument and its result shape.

`crates/cogitator-lsp/tests/consumer_parity.rs` enforces 1–4, plus that no
editor references a `"dbt.x"` the server lacks and that VS Code's
`extension.ts` and `package.json` contribute the same palette commands. An
editor "exposes" a command when one of its files holds the quoted name.

## Adding a command: the checklist, in order

**Server**
- `COMMANDS` (`cogitator-lsp/src/types/init.rs`), the dispatch arm
  (`cogitator/src/server/dispatcher/commands_exec.rs`), the handler
  (`cogitator/src/server/handlers/`), long work through `offload_blocking`.
- An e2e test (`crates/cogitator/tests/e2e_*.rs`) of the command over the wire.
- The row in `specs/neovim-plugin/00-plugin.md` (§ command table) and, for a
  `:Dbt*` command, §5.1.
- Its line in `editors/parity.toml`.
- README/AGENTS.md where the feature is described.

**Neovim** (`editors/nvim`)
- A function in `lua/cogitator/commands.lua` that calls `client:exec_cmd`;
  rendering goes in a pure function exposed on `M` (`plan_lines`,
  `split_lines`, `incrementality_items`) so it is testable without a server.
- A `:Dbt*` user command in `plugin/cogitator.lua`, a line in
  `doc/cogitator.txt`.
- A unit check in `tests/steps/01_unit.lua` (the pure function) and a wire
  check in `tests/steps/03_features.lua` (`H.exec_cmd`, which returns
  `nil, true` for a `null` that did arrive).

**Emacs** (`editors/emacs`)
- An autoloaded `cogitator-*` command through `cogitator--execute`; a pure
  `cogitator--*` helper for what it shows; results are plists, `null` is `nil`
  or `:null`.
- A row in the README's command table.
- Unit and integration checks in `tests/run.el` (`run--exec`); `just
  elisp-lint` byte-compiles with warnings as errors.

**VS Code** (`editors/vscode`)
- A palette entry in `src/extension.ts` through `execDbt`, exported pure
  helpers for what it shows, and the command in `package.json`
  `contributes.commands` (the test holds the two together).
- The README's "Palette commands".
- Checks in `src/test/suite/index.ts`.

**Zed** — nothing to wrap; a README note when the feature is a code action.

**Every client**: handle a `null` result (say why there is nothing), go to a
file from a `uri`, never assume a position the server did not send.

## Code actions and lenses

They are built server-side (`cogitator-analysis/src/features/code_actions/`,
`lenses.rs`) and reach every editor unchanged. If one carries a command,
that command is `internal` with its `via`. If people will also want it
without a cursor on the right spot, add a `user` command for it and follow the
checklist.

## Verify

```
cargo test -p cogitator-lsp --test consumer_parity
just nvim-test        # headless Neovim, needs a debug server build
just emacs-test
just elisp-lint
just lua-lint
just vscode-build     # tsc
just vscode-test      # extension host; on NixOS the downloaded VS Code exits 127 — say so
```

On NixOS run them through `devenv shell --`.

## Gotchas met so far

- `stylua` from the shell may reformat files you did not touch (its version
  differs from the one that formatted them): format your own files only,
  `stylua <files>`.
- A test that takes "the last quickfix" breaks when the server adds one; pick
  an action by its title.
- A `[gaps]` line for a new command is never the fix: wrap it, or argue in
  review that it is `internal`.
