# Pinned nixpkgs for shell.nix (devenv pins its own input in devenv.yaml).
#
# Best practice is to also pin the sha256. Fill it in with:
#   nix-prefetch-url --unpack \
#     https://github.com/NixOS/nixpkgs/archive/refs/tags/25.05.tar.gz
# and add `sha256 = "<result>";` to the fetchTarball attrset below.
import (builtins.fetchTarball {
  name = "nixpkgs-25.05";
  url = "https://github.com/NixOS/nixpkgs/archive/refs/tags/25.05.tar.gz";
})
