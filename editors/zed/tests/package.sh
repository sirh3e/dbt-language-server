#!/usr/bin/env bash
# Package the extension the way Zed's dev-extension builder does: the
# wasm32-wasip1 module adapted into a component with wasmtime's WASI
# preview1 reactor adapter (Zed loads components only). Prints the path.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
out="$root/target/package"
adapter="$out/wasi_snapshot_preview1.reactor.wasm"
adapter_url="https://github.com/bytecodealliance/wasmtime/releases/download/v18.0.2/wasi_snapshot_preview1.reactor.wasm"
adapter_sha256="71b6d9b84f99ab0f0ca151fd1f8642f594e44190a8e23f6b16cb3d799765f3f0"

(cd "$root" && cargo build --quiet --target wasm32-wasip1)
mkdir -p "$out"
if ! echo "$adapter_sha256  $adapter" | sha256sum --check --status 2>/dev/null; then
    curl -fsSL -o "$adapter" "$adapter_url"
    echo "$adapter_sha256  $adapter" | sha256sum --check --status \
        || { echo "the WASI adapter's checksum does not match" >&2; exit 1; }
fi
wasm-tools component new "$root/target/wasm32-wasip1/debug/zed_cogitator.wasm" \
    --adapt "wasi_snapshot_preview1=$adapter" -o "$out/extension.wasm"
cp "$root/extension.toml" "$out/extension.toml"
echo "$out"
