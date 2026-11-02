# devenv entry point (https://devenv.sh): `devenv shell`, or direnv's
# `use devenv`. The Rust toolchain comes from devenv's own rust module;
# every other tool derives from nix/tooling.nix, the single source.
{ pkgs, ... }:
{
  packages = (import ./nix/tooling.nix pkgs).common;

  languages.rust = {
    enable = true;
    channel = "stable";
  };

  scripts.check.exec = "just check";

  enterShell = ''
    echo "cogitator devenv — 'check' runs the CI set, just --list for more"
  '';
}
