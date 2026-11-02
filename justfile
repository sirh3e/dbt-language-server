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
