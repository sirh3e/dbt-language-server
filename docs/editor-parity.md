# Editor parity: every command in every editor

*For contributors.* The server is the product; the editors are thin
clients of it. A feature reached through a `dbt.*` command exists for a
user only once their editor wraps it, so a new command reaches every editor
**in the same change**. A test enforces it.

## Why

Before the rule, the editors had drifted: of the server's 35 commands,
VS Code wrapped 10, Emacs 23 and Neovim 29, and nothing said which were
missing or why. Today there are 36 commands, 32 of them for users; Neovim
wraps all of them, Emacs misses 12 and VS Code 22 — each listed, and the
list only shrinks.

## The rule

Code actions and code lenses need no wrapping: every LSP client runs them,
Zed included. The rule is about **commands**.

1. Every command the server registers (`COMMANDS` in
   `crates/cogitator-lsp/src/types/init.rs`) is classified in
   [`editors/parity.toml`](../editors/parity.toml):

   ```toml
   [commands]
   "dbt.run"        = { kind = "user" }
   "dbt.openSchema" = { kind = "internal", via = "code lens and code action 'Go to schema' (…)" }
   ```

   - `user` — a person runs it, so **Neovim, Emacs and VS Code all wrap it**.
   - `internal` — the server only sends it inside a code lens or code
     action; `via` says where.
2. **Zed** cannot add commands (an extension has no command surface): it
   gets code actions and lenses, and its README says the rest needs an
   executeCommand client.
3. `[gaps.<editor>]` lists what each editor was missing when the rule was
   adopted. **It only shrinks**: a new command never gets a line there, and
   wrapping a listed command means deleting its line.
4. Every command is documented twice: a row in
   `specs/neovim-plugin/00-plugin.md`'s command table (the contract) and a
   section in [commands.md](commands.md) (the reference).

## The test

`cargo test -p cogitator-lsp --test consumer_parity` (part of `just check`
and of CI's editors job, so an editors-only change runs it too) fails, with
what to do, when:

- a registered command is not classified —
  ``"`dbt.x` is registered by the server but not classified in editors/parity.toml…"``;
- an editor misses a user command with no gap line —
  ``"vscode does not expose `dbt.x`: wrap it in editors/vscode (src/extension.ts palette, package.json contributes, README)…"``;
- a gap is closed but still listed — ``"nvim now exposes `dbt.x`: remove its line from [gaps.nvim]…"``;
- an editor names a command the server lacks (a typo, a removed command);
- a command has no row in the spec, or no section in `docs/commands.md`;
- VS Code's palette in `src/extension.ts` and `package.json`'s
  contributions disagree.

An editor "exposes" a command when one of its files holds the quoted name
(`"dbt.x"`).

## Adding a command

The full checklist is the `editor-parity` skill
([`.claude/skills/editor-parity/SKILL.md`](../.claude/skills/editor-parity/SKILL.md)).
In short:

1. **Server** — `COMMANDS`, the dispatch arm in
   `crates/cogitator/src/server/dispatcher/commands_exec.rs`, the handler,
   an e2e test; the spec row; the `editors/parity.toml` line; a section in
   `docs/commands.md`.
2. **Neovim** — a function in the fitting module of `lua/cogitator/commands/` (jobs, lists, analysis, views, node, split, navigation; `commands/init.lua` gathers them into the one `commands` table) with its
   rendering in a pure function the unit tests call, a `:Dbt…` command, the
   help, a unit and a wire check. Never block: `exec_cmd` with a callback,
   `actions.request` for other methods (a scan fails on `request_sync`,
   `vim.wait(` and `:wait(`).
3. **Emacs** — a `cogitator-…` command through `cogitator--execute`, a pure
   render helper, the README table, checks in `tests/run.el`.
4. **VS Code** — a palette entry in `src/extension.ts` with exported pure
   helpers, the `package.json` contribution, the README, checks in the
   suite.
5. **Zed** — nothing to wrap; mention it in its README when the feature is
   a code action.

Then: the parity test, `just nvim-test`, `just emacs-test`, `just
elisp-lint`, `just lua-lint`, `just vscode-build`.
