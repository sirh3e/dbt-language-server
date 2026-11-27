# Zed for the extension's real-client tests (editors/zed), from a newer
# nixpkgs than the toolchain's pin: editors/zed uses zed_extension_api
# 0.6.0, which Zed 0.189 (nixos-25.05) refuses as unreleased; nixos-26.05
# has Zed 1.25. Pinned by revision and hash, like nix/nixpkgs.nix ought to be.
# Bump: `nix flake metadata github:NixOS/nixpkgs/nixos-26.05 --json`
# gives `.locked.rev` and `.locked.narHash`.
pkgs:
let
  nixpkgs = builtins.fetchTarball {
    name = "nixpkgs-26.05-zed";
    url = "https://github.com/NixOS/nixpkgs/archive/0dc498bfe49baeb18b0f9891e800b53bb2603193.tar.gz";
    sha256 = "sha256-s/0jq8usZtLVOolYBwmyUmd3JsfAu7w3FU+a0dgMqSY=";
  };
in
(import nixpkgs { inherit (pkgs.stdenv.hostPlatform) system; }).zed-editor
