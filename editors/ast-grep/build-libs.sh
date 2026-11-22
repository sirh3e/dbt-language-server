#!/bin/sh
# Build the two grammars the server's ast-grep uses as shared libraries
# for the ast-grep CLI (its customLanguages): tree-sitter-dbt (Jinja over
# SQL) and tree-sitter-sequel (SQL), from the very sources cargo built the
# server with — the pinned revisions, found with `cargo metadata`.
#
#   sh editors/ast-grep/build-libs.sh [out-dir]   (default target/ast-grep)
#
# Writes dbt.so and sql.so (.dylib on macOS, .dll on Windows).
set -eu

out=${1:-target/ast-grep}
cc=${CC:-cc}
case "$(uname -s)" in
  Darwin) ext=dylib ;;
  MINGW* | MSYS* | CYGWIN*) ext=dll ;;
  *) ext=so ;;
esac

# the package directory of a crate in the workspace's resolved graph
crate_dir() {
  cargo metadata --format-version 1 --locked 2>/dev/null |
    python3 -c 'import json,sys,os
name=sys.argv[1]
for p in json.load(sys.stdin)["packages"]:
    if p["name"] == name:
        print(os.path.dirname(p["manifest_path"])); break' "$1"
}

build() {
  name=$1 crate=$2
  dir=$(crate_dir "$crate")
  if [ -z "$dir" ] || [ ! -f "$dir/src/parser.c" ]; then
    echo "build-libs: cannot find $crate's sources (run cargo fetch first)" >&2
    exit 1
  fi
  sources="$dir/src/parser.c"
  [ -f "$dir/src/scanner.c" ] && sources="$sources $dir/src/scanner.c"
  # shellcheck disable=SC2086
  "$cc" -shared -fPIC -O2 -std=c11 -I "$dir/src" $sources -o "$out/$name.$ext"
  echo "$out/$name.$ext  ($crate, $dir)"
}

mkdir -p "$out"
build dbt tree-sitter-dbt
build sql tree-sitter-sequel
