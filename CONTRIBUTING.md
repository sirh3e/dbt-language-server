# Contributing

How a change travels into `main`, and what a release consists of.

## The feature workflow (trunk-based)

`main` is always releasable: CI must be green, and every commit on it is
a [Conventional Commit](https://www.conventionalcommits.org/) — the
changelog is generated from the history (`just changelog`).

1. **Set up once**: enter the dev environment (`nix-shell`,
   `devenv shell` or `direnv allow`) and install the git hooks with
   `prek install` (fmt check, clippy with zero warnings, dbt-schema
   freshness, shellcheck).
2. **Branch per change**, named after the conventional type it will
   land as:
   `feat/<slug>`, `fix/<slug>`, `docs/<slug>`, `refactor/<slug>`,
   `chore/<slug>`, `test/<slug>`.
3. **Commit conventionally**: `feat(completion): rank CTE columns first`,
   `fix(lsp): …`. Scopes in use: `server`, `lsp`, `parser`, `analysis`,
   `schemas`, `completion`, `profiles`, `nvim`, `emacs`, `zed`, `install`,
   `deps`, `specs`, `readme`, `changelog`, `nix`, `github`.
4. **Quality bar** before the PR:
   - `just check` (fmt, clippy `-D warnings`, all Rust suites, schema
     freshness) — this is exactly what CI runs. `just test` passes
     `--features cogitator/e2e` (the fake-dbt test bin); a bare
     `cargo test --workspace` skips the job e2e tests.
   - `just system-test` (or `just nvim-test` / `just emacs-test` /
     `just vscode-test` for a quick dev loop) when `editors/` changed,
     `just zed-build` when the Zed extension changed. The suites self-sandbox on a temp copy of
     the fixture; `COGITATOR_E2E_DEADLINE_MS` widens the e2e deadlines
     on slow machines.
   - The specs are **binding**: a change to wire behavior updates
     `specs/rust-port/` (server) or `specs/neovim-plugin/` (editor
     surface) in the same PR.
   - New user-visible failures carry a Reason and a Solution.
5. **Open a PR against `main`** on
   [sirh3e/dbt-language-server](https://github.com/sirh3e/dbt-language-server).
   CI is path-gated: the cargo matrix and lint run only when the Rust/C
   side changed; shellcheck and the nix-shell job run for their files.
6. **Squash-merge** with a conventional title — one PR becomes one
   changelog line. Dependabot's weekly `chore(deps)` PRs ride the same
   path.

**Hotfixes**: when `main` has moved past the latest release, branch
`fix/<slug>` from the release tag, fix, tag a patch release from that
branch, and cherry-pick the fix back to `main`.

## The release cycle

Releases are cut from `main` when meaningful work has accumulated —
no calendar. Versioning is semver over the conventional history, with
one hard rule: **tags start at v0.5.0**, because v0.1.x–v0.4.x belong to
the upstream Go project this fork was rewritten from (git-cliff ignores
those tags; see cliff.toml).

The whole mechanical part is one recipe:

```bash
just release-prep 0.5.0
```

which, in order:

1. guards: working tree clean, on `main`, version ≥ 0.5.0;
2. bumps `[workspace.package] version` in the root `Cargo.toml` (every
   crate inherits it) and refreshes `Cargo.lock`;
3. runs `just check`;
4. regenerates `CHANGELOG.md` with the new section
   (`git cliff 724b0f2..HEAD --tag v<version>`);
5. commits `chore(release): v<version>` and creates the annotated tag.

Then a human reviews the commit and publishes:

```bash
git push origin main v0.5.0
```

The tag push triggers `.github/workflows/release.yml`: the
cargo-zigbuild matrix builds
`cogitator-{linux-amd64,darwin-amd64,darwin-arm64,windows-amd64.exe}`
from one host, creates the GitHub release with git-cliff notes, and
uploads the assets. From that moment `install.sh` / `install.ps1`
resolve the new "latest".

**Post-release checks**: the release page lists all four assets;
`curl -fsSL …/install.sh | sh` installs; `cogitator --version` prints
the new version.
