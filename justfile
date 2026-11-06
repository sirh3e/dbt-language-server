# cogitator — development recipes, grouped (`just --list` shows them all)

# default: show available recipes
default:
    @just --list

# ---------------------------------------------------------------- build

# build the whole workspace (debug)
[group('build')]
build:
    cargo build --workspace

# build the release binary (target/release/cogitator)
[group('build')]
release:
    cargo build --release -p cogitator

# cross-compile the release matrix via the Zig toolchain (07 §5)
[group('build')]
zigbuild target:
    cargo zigbuild --release --target {{target}} -p cogitator

# ----------------------------------------------------------------- test

# run the full test suite (the e2e feature builds the fake-dbt test
# bin; a plain `cargo test --workspace` would skip the job e2e tests)
[group('test')]
test:
    cargo test --workspace --features cogitator/e2e

# type-check the feature-gated DuckDB engine (bundled C++ build — slow cold)
[group('test')]
duckdb-check:
    cargo check -p cogitator --features duckdb-parser

# --------------------------------------------------------------- format

# format all crates in place
[group('format')]
fmt:
    cargo fmt --all

# verify formatting without writing
[group('format')]
fmt-check:
    cargo fmt --all --check

# Format the Neovim plugin's Lua (stylua from the nix shell)
[group('format')]
lua-fmt:
    stylua editors/nvim

# Check Lua formatting without writing
[group('format')]
lua-fmt-check:
    stylua --check editors/nvim

# ----------------------------------------------------------------- lint

# lint; warnings are errors (the workspace stays at zero)
[group('lint')]
lint:
    cargo clippy --workspace --all-targets --features cogitator/e2e -- -D warnings

# Lint our own shell scripts (the testdata fixture is third-party)
[group('lint')]
lint-shell:
    git ls-files '*.sh' '*.bash' | grep -v '^testdata/' | xargs -r shellcheck

# Lint the Neovim plugin's Lua (luacheck from the nix shell)
[group('lint')]
lua-lint:
    luacheck editors/nvim

# Byte-compile the Emacs integration with warnings as errors
[group('lint')]
elisp-lint:
    emacs -Q --batch --eval "(setq byte-compile-error-on-warn t)" -L editors/emacs -f batch-byte-compile editors/emacs/cogitator.el editors/emacs/cogitator-graph.el
    rm -f editors/emacs/*.elc

# ------------------------------------------------------------------- ci

# everything CI checks: formatting, lints, tests, schema freshness
[group('ci')]
check: fmt-check lint test gen-schemas-check

# -------------------------------------------------------------- schemas

# regenerate the preprocessed dbt schemas (specs/rust-port/08 §2.2)
[group('schemas')]
gen-schemas:
    for f in dbt_project dbt_yml_files packages selectors; do \
        python3 schemas/dbt/preprocess.py "schemas/dbt/$f-latest.json" "schemas/dbt/processed/$f.json"; \
    done
    for f in run-results-v6 sources-v3 catalog-v1; do \
        python3 schemas/dbt/preprocess.py --mode=artifact "schemas/dbt/artifacts/$f.json" "schemas/dbt/processed/artifacts/$f.json"; \
    done
    python3 schemas/dbt/preprocess.py --mode=manifest-projection "schemas/dbt/artifacts/manifest-v12.json" "schemas/dbt/processed/artifacts/manifest-v12-projection.json" 

# fail if the preprocessed schemas are stale (CI job, 09 §5)
[group('schemas')]
gen-schemas-check: gen-schemas
    git diff --exit-code -- schemas/dbt/processed

# regenerate the committed cogitator.yml JSON Schema (03 §10; a sync
# test fails the suite while it is stale)
[group('schemas')]
gen-config-schema:
    cargo run -q -p cogitator -- schema > schemas/cogitator.schema.json

# -------------------------------------------------------------- editors

# The whole system layer: both editor suites driven by the Rust
# orchestrator against the exact binary cargo builds (a missing editor
# prints SKIPPED; CI sets COGITATOR_REQUIRE_EDITORS=1 to forbid that)
[group('test')]
system-test:
    cargo test -p cogitator --features e2e --test system_editors -- --ignored --nocapture

# Headless Neovim plugin tests (needs nvim >= 0.11 and a debug server build)
[group('editors')]
nvim-test:
    cargo build -p cogitator
    nvim --headless -l editors/nvim/tests/run.lua

# Batch Emacs integration tests (needs emacs >= 29 and a debug server build)
[group('editors')]
emacs-test:
    cargo build -p cogitator
    emacs -Q --batch -L editors/emacs -l editors/emacs/tests/run.el

# Compile the Zed extension to WebAssembly the way Zed does
[group('editors')]
zed-build:
    cd editors/zed && cargo build --target wasm32-wasip1

# -------------------------------------------------------------- release

# (range starts at the fork point: upstream Go commits and tags stay out)
# Regenerate CHANGELOG.md from the conventional commit history (git-cliff)
[group('release')]
changelog:
    git cliff 724b0f2..HEAD -o CHANGELOG.md

# (review the tagged commit, then: git push origin main v<version>)
# Prepare a release: guards, bump, check, changelog, commit + annotated tag
[group('release')]
release-prep version:
    #!/usr/bin/env bash
    set -euo pipefail
    v="{{version}}"
    [ -z "$(git status --porcelain)" ] || { echo "error: working tree not clean" >&2; exit 1; }
    [ "$(git branch --show-current)" = "main" ] || { echo "error: not on main" >&2; exit 1; }
    if [ "$(printf '%s\n' 0.5.0 "$v" | sort -V | head -1)" != "0.5.0" ]; then
        echo "error: fork releases start at 0.5.0 (upstream owns v0.1.x-v0.4.x)" >&2; exit 1
    fi
    perl -pi -e 'if (!$done && s/^version = ".*"/version = "'"$v"'"/) { $done = 1 }' Cargo.toml
    cargo update --workspace --quiet
    just check
    git cliff 724b0f2..HEAD --tag "v$v" -o CHANGELOG.md
    git add Cargo.toml Cargo.lock CHANGELOG.md
    git commit -m "chore(release): v$v"
    git tag -a "v$v" -m "cogitator v$v"
    echo "tagged v$v — review the commit, then: git push origin main v$v"

# ---------------------------------------------------------- maintenance

# remove build artifacts
[group('maintenance')]
clean:
    cargo clean

# Install the git hooks via prek (the Rust pre-commit runner)
[group('maintenance')]
hooks:
    prek install
