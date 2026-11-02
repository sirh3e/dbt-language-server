#!/bin/sh
# Install the cogitator dbt language server from this project's GitHub
# releases. This replaces the old installer, which pointed at the upstream
# Go project (j-clemons/dbt-language-server) and would install the
# previous Go binary.
#
#   curl -fsSL https://raw.githubusercontent.com/sirh3e/dbt-language-server/main/install.sh | sh
#
# Overrides:
#   COGITATOR_REPO     owner/name of the GitHub repository
#   COGITATOR_VERSION  tag to install (default: latest release)
#   COGITATOR_BIN_DIR  install directory (default: ~/.local/bin)

set -eu

REPO="${COGITATOR_REPO:-sirh3e/dbt-language-server}"
VERSION="${COGITATOR_VERSION:-latest}"
BIN_DIR="${COGITATOR_BIN_DIR:-$HOME/.local/bin}"

case "$(uname -s)" in
    Linux) os="linux" ;;
    Darwin) os="darwin" ;;
    *)
        echo "error:    unsupported OS: $(uname -s)" >&2
        echo "solution: download cogitator-windows-amd64.exe (or build from" >&2
        echo "          source: cargo build --release -p cogitator)" >&2
        exit 1
        ;;
esac

case "$(uname -m)" in
    x86_64 | amd64) arch="amd64" ;;
    arm64 | aarch64) arch="arm64" ;;
    *)
        echo "error:    unsupported architecture: $(uname -m)" >&2
        echo "solution: build from source: cargo build --release -p cogitator" >&2
        exit 1
        ;;
esac

asset="cogitator-${os}-${arch}"
if [ "$os" = "linux" ] && [ "$arch" = "arm64" ]; then
    echo "error:    no linux-arm64 release asset yet" >&2
    echo "solution: build from source: cargo build --release -p cogitator" >&2
    exit 1
fi

if [ "$VERSION" = "latest" ]; then
    url="https://github.com/${REPO}/releases/latest/download/${asset}"
else
    url="https://github.com/${REPO}/releases/download/${VERSION}/${asset}"
fi

echo "installing cogitator (${asset}, ${VERSION}) from ${REPO} into ${BIN_DIR}"
mkdir -p "$BIN_DIR"
tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT

if ! curl -fSL --proto '=https' -o "$tmp" "$url"; then
    echo "error:    download failed: $url" >&2
    echo "reason:   the release or asset may not exist yet" >&2
    echo "solution: check https://github.com/${REPO}/releases, or build from" >&2
    echo "          source: cargo build --release -p cogitator" >&2
    exit 1
fi

install -m 755 "$tmp" "$BIN_DIR/cogitator"
echo "installed $BIN_DIR/cogitator"
case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *) echo "note: add $BIN_DIR to your PATH" ;;
esac
"$BIN_DIR/cogitator" --version
