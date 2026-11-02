# Classic nix-shell entry point: `nix-shell`, or `use nix` via direnv.
# Everything comes from nix/tooling.nix — add tools there, not here.
{
  pkgs ? import ./nix/nixpkgs.nix { },
}:
let
  tooling = import ./nix/tooling.nix pkgs;
in
pkgs.mkShell {
  name = "cogitator-dev";
  packages = tooling.rust ++ tooling.common;

  shellHook = ''
    echo "cogitator dev shell — just --list for the recipes"
    if command -v prek >/dev/null && [ ! -f .git/hooks/pre-commit ]; then
      echo "hint: run 'prek install' once to enable the git hooks"
    fi
  '';
}
