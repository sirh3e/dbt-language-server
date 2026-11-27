# THE single source for the development toolchain. Both entry points
# derive from this file: shell.nix takes `rust ++ common`, devenv.nix
# takes `common` (its languages.rust module provides the toolchain).
# New tools go here, nowhere else.
pkgs:
with pkgs;
{
  # Rust stable: the workspace, clippy -D warnings, rustfmt, IDE support.
  rust = [
    rustc
    cargo
    clippy
    rustfmt
    rust-analyzer
  ];

  common =
    [
      # task runner and the schema preprocessor
      just
      python3
      git

      # release cross-builds (specs/rust-port/07 §5)
      zig
      cargo-zigbuild

      # linting / changelog / hooks
      shellcheck
      git-cliff

      # editor integration test suites (just nvim-test / emacs-test)
      neovim
      emacs-nox

      # Lua formatting + linting for the Neovim plugin
      # (just lua-fmt / lua-fmt-check / lua-lint)
      stylua
      luaPackages.luacheck
    ]
    # .cargo/config.toml selects clang + mold as the dev linker on Linux
    ++ lib.optionals stdenv.isLinux [
      mold
      clang

      # a real Zed client for the extension (editors/zed): `zeditor` on a
      # project, or under `xvfb-run` without a display (docs: editors/zed).
      # Linux only: nixpkgs does not build Zed for every Darwin. Its own
      # pin (nix/zed.nix), as the toolchain pin's Zed is too old for the
      # extension's API.
      (import ./zed.nix pkgs)
      xvfb-run
      # Zed loads a wasm *component*: `just zed-package` adapts the module
      wasm-tools
    ]
    # prek is new in nixpkgs; older pins fall back to `cargo install prek`
    ++ lib.optionals (pkgs ? prek) [ prek ];
}
