#!/usr/bin/env bash
# The extension in a real Zed, without a display: Zed under Xvfb on Mesa's
# software Vulkan (lavapipe), with a throwaway home, the packaged extension
# installed in it and the jaffle fixture open on a YAML file (Zed has YAML
# built in; SQL would need Zed's SQL extension from the network). The
# throwaway home trusts every worktree, as restricted mode would otherwise
# hold every language server back until someone clicks. Passes when
# Zed loaded the extension and started the debug `cogitator` itself.
# Usage: smoke.sh [seconds to wait, default 60]
set -euo pipefail
repo=$(cd "$(dirname "$0")/../../.." && pwd)
wait=${1:-60}
work=$(mktemp -d "${TMPDIR:-/tmp}/cogitator-zed-smoke.XXXXXX")
trap 'kill -TERM -- "-${zed_pg:-0}" 2>/dev/null || true; rm -rf "$work"' EXIT

package=$("$repo/editors/zed/tests/package.sh")
(cd "$repo" && cargo build --quiet -p cogitator)
server="$repo/target/debug/cogitator"

home="$work/home"
installed="$home/.local/share/zed/extensions/installed/cogitator"
mkdir -p "$installed" "$home/.config/zed"
cp "$package/extension.toml" "$package/extension.wasm" "$installed/"
cp -r "$repo/testdata/jaffle_shop_duckdb" "$work/project"
cat > "$home/.config/zed/settings.json" <<JSON
{ "auto_update": false,
  "telemetry": { "diagnostics": false, "metrics": false },
  "session": { "trust_all_worktrees": true },
  "lsp": { "cogitator": { "binary": { "path": "$server" } } } }
JSON

# Xvfb speaks X11 only; a desktop's Wayland socket would be preferred.
unset WAYLAND_DISPLAY
icd=/run/opengl-driver/share/vulkan/icd.d/lvp_icd.x86_64.json
[ -f "$icd" ] && export VK_ICD_FILENAMES="$icd"
export HOME="$home" XDG_DATA_HOME="$home/.local/share" \
    XDG_CONFIG_HOME="$home/.config" ZED_ALLOW_EMULATED_GPU=1
setsid xvfb-run -a -s '-screen 0 1280x800x24' \
    zeditor --foreground "$work/project" "$work/project/models/schema.yml" \
    > "$work/zed.out" 2>&1 &
zed_pg=$!

log="$home/.local/share/zed/logs/Zed.log"
started=""
for _ in $(seq "$wait"); do
    # a cogitator whose parent is Zed, running this checkout's build
    for pid in $(pgrep -x cogitator || true); do
        exe=$(readlink "/proc/$pid/exe" 2>/dev/null || true)
        parent=$(ps -o comm= -p "$(ps -o ppid= -p "$pid" | tr -d ' ')" 2>/dev/null || true)
        # nixpkgs' wrapper runs Zed as `.zed-editor-wrapped` (comm is cut at 15)
        [ "$exe" = "$server" ] && [[ "$parent" == *zed* ]] && started=$pid
    done
    [ -n "$started" ] && break
    sleep 1
done

# Zed 0.189 writes "failed to load wasm extension cogitator", 1.25
# "Failed to load extension: cogitator"
failed='failed to load (wasm )?extension:? cogitator'
if grep -q -i -E "$failed" "$log" 2>/dev/null; then
    grep -i -E -A6 "$failed" "$log" >&2
    echo "FAIL: Zed could not load the extension" >&2; exit 1
fi
if [ -z "$started" ]; then
    { tail -20 "$work/zed.out"; grep -i cogitator "$log" 2>/dev/null | tail -10; } >&2
    echo "FAIL: Zed did not start cogitator within ${wait}s" >&2; exit 1
fi
echo "ok: Zed $(zeditor --version 2>/dev/null | awk '{print $2}') loaded the extension and started cogitator (pid $started)"
