#!/bin/sh
# Install the cogitator dbt language server on Linux/macOS from this
# project's GitHub releases (Windows users: install.ps1).
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
        echo "solution: Windows uses install.ps1; otherwise build from" >&2
        echo "          source: cargo build --release -p cogitator" >&2
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

name="cogitator-${os}-${arch}"
asset="${name}.tar.gz"
if [ "$os" = "linux" ] && [ "$arch" = "arm64" ]; then
    echo "error:    no linux-arm64 release asset yet" >&2
    echo "solution: build from source: cargo build --release -p cogitator" >&2
    exit 1
fi

if [ "$VERSION" = "latest" ]; then
    base="https://github.com/${REPO}/releases/latest/download"
else
    base="https://github.com/${REPO}/releases/download/${VERSION}"
fi

echo "installing cogitator (${asset}, ${VERSION}) from ${REPO} into ${BIN_DIR}"
mkdir -p "$BIN_DIR"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

if ! curl -fSL --proto '=https' -o "$tmp/$asset" "$base/$asset"; then
    echo "error:    download failed: $base/$asset" >&2
    echo "reason:   the release or asset may not exist yet" >&2
    echo "solution: check https://github.com/${REPO}/releases, or build from" >&2
    echo "          source: cargo build --release -p cogitator" >&2
    exit 1
fi

# verify against the release's SHA256SUMS; a missing checksum tool or
# checksum file downgrades to a warning, a MISMATCH is always fatal
if command -v sha256sum >/dev/null 2>&1; then
    sha() { sha256sum "$1"; }
elif command -v shasum >/dev/null 2>&1; then
    sha() { shasum -a 256 "$1"; }
else
    sha() { :; }
    echo "warning: neither sha256sum nor shasum found - skipping checksum verification" >&2
fi
if ! curl -fsSL --proto '=https' -o "$tmp/SHA256SUMS" "$base/SHA256SUMS"; then
    echo "warning: no SHA256SUMS in this release - skipping checksum verification" >&2
else
    expected="$(grep " ${asset}\$" "$tmp/SHA256SUMS" | cut -d' ' -f1)"
    actual="$(sha "$tmp/$asset" | cut -d' ' -f1)"
    if [ -n "$actual" ]; then
        if [ "$expected" != "$actual" ]; then
            echo "error:    checksum mismatch for $asset" >&2
            echo "reason:   expected $expected, got $actual" >&2
            echo "solution: re-run the installer; if it persists, the download" >&2
            echo "          is corrupted or tampered with - do not use it" >&2
            exit 1
        fi
        echo "checksum verified"
    fi
fi

tar -xzf "$tmp/$asset" -C "$tmp"
install -m 755 "$tmp/$name/cogitator" "$BIN_DIR/cogitator"
echo "installed $BIN_DIR/cogitator"
case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *) echo "note: add $BIN_DIR to your PATH" ;;
esac
"$BIN_DIR/cogitator" --version
