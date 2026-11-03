# cogitator — development recipes (`just --list` shows them all)

# default: show available recipes
default:
    @just --list

# build the whole workspace (debug)
build:
    cargo build --workspace

# build the release binary (target/release/cogitator)
release:
    cargo build --release -p cogitator

# run the full test suite
test:
    cargo test --workspace

# format all crates in place
fmt:
    cargo fmt --all

# verify formatting without writing
fmt-check:
    cargo fmt --all --check

# lint; warnings are errors (the workspace stays at zero)
lint:
    cargo clippy --workspace --all-targets -- -D warnings

# everything CI checks: formatting, lints, tests, schema freshness
check: fmt-check lint test gen-schemas-check

# type-check the feature-gated DuckDB engine (bundled C++ build — slow cold)
duckdb-check:
    cargo check -p cogitator --features duckdb-parser

# regenerate the preprocessed dbt schemas (specs/rust-port/08 §2.2)
gen-schemas:
    for f in dbt_project dbt_yml_files packages selectors; do \
        python3 schemas/dbt/preprocess.py "schemas/dbt/$f-latest.json" "schemas/dbt/processed/$f.json"; \
    done

# fail if the preprocessed schemas are stale (CI job, 09 §5)
gen-schemas-check: gen-schemas
    git diff --exit-code -- schemas/dbt/processed

# cross-compile the release matrix via the Zig toolchain (07 §5)
zigbuild target:
    cargo zigbuild --release --target {{target}} -p cogitator

# remove build artifacts
clean:
    cargo clean

# Headless Neovim plugin tests (needs nvim >= 0.11 and a debug server build)
nvim-test:
    cargo build -p cogitator
    nvim --headless -l editors/nvim/tests/run.lua

# Batch Emacs integration tests (needs emacs >= 29 and a debug server build)
emacs-test:
    cargo build -p cogitator
    emacs -Q --batch -L editors/emacs -l editors/emacs/tests/run.el

# Format the Neovim plugin's Lua (stylua from the nix shell)
lua-fmt:
    stylua editors/nvim

# Check Lua formatting without writing
lua-fmt-check:
    stylua --check editors/nvim

# Lint the Neovim plugin's Lua (luacheck from the nix shell)
lua-lint:
    luacheck editors/nvim

# Byte-compile the Emacs integration with warnings as errors
elisp-lint:
    emacs -Q --batch --eval "(setq byte-compile-error-on-warn t)" -L editors/emacs -f batch-byte-compile editors/emacs/cogitator.el editors/emacs/cogitator-graph.el
    rm -f editors/emacs/*.elc

# Compile the Zed extension to WebAssembly the way Zed does
zed-build:
    cd editors/zed && cargo build --target wasm32-wasip1

# Regenerate CHANGELOG.md from the conventional commit history (git-cliff).
# The range starts at the fork point: upstream Go commits and tags stay out.
changelog:
    git cliff 724b0f2..HEAD -o CHANGELOG.md

# Install the git hooks via prek (the Rust pre-commit runner)
hooks:
    prek install

# Lint our own shell scripts (the testdata fixture is third-party)
lint-shell:
    git ls-files '*.sh' '*.bash' | grep -v '^testdata/' | xargs -r shellcheck

# Prepare a release: guards, version bump, check, changelog section,
# release commit and annotated tag. Review, then:
#   git push origin main v<version>
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
