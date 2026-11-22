#!/usr/bin/env bash
# Regenerate golden.json: the names a real dbt gives every generic test
# of this project (and of ../jaffle_shop_duckdb), which crates/cogitator-analysis/tests/test_names_golden.rs
# holds test_names.rs to. Runs dbt in a scratch copy, never in the tree.
#   DBT=/path/to/dbt ./golden.sh            (any dbt with dbt-duckdb)
#   ./golden.sh                             (uvx, the pinned versions)
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cp -r "$here/." "$work/nasty"
cp -r "$here/../jaffle_shop_duckdb" "$work/jaffle"
rm -rf "$work/jaffle/target"
if [ -n "${DBT:-}" ]; then
  dbt=("$DBT")
else
  dbt=(uvx --from 'dbt-core==1.11.17' --with 'dbt-duckdb==1.11.1' dbt)
fi
(cd "$work/nasty" && "${dbt[@]}" deps --profiles-dir .dbt >/dev/null)
for project in nasty jaffle; do
  (cd "$work/$project" && "${dbt[@]}" parse --profiles-dir .dbt --no-partial-parse >/dev/null)
done
cd "$work"
python3 - "$here/golden.json" <<'PY'
import json, sys

def generic_test_names(project):
    manifest = json.load(open(f"{project}/target/manifest.json"))
    names = sorted(
        n["name"] for n in manifest["nodes"].values()
        if n["resource_type"] == "test" and "test_metadata" in n
    )
    return manifest["metadata"]["dbt_version"], names

version, names = generic_test_names("nasty")
_, jaffle = generic_test_names("jaffle")
golden = {"dbt_version": version, "names": names, "jaffle_shop_duckdb": jaffle}
with open(sys.argv[1], "w") as f:
    json.dump(golden, f, indent=2)
    f.write("\n")
PY
