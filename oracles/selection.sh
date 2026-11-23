#!/usr/bin/env sh
# dbt.select against dbt itself: for each selection, the names `dbt ls
# --select … --output name` prints and the ones the server's evaluator
# answers (crates/cogitator-analysis/examples/select.rs), diffed.
#
#   oracles/selection.sh <project dir> <selection>...
#
# DBT is the dbt command (default `dbt`), e.g.
#   DBT="uvx --from dbt-oss==2.0.8 dbt" oracles/selection.sh ../proj "+orders" "tag:nightly"
# The project's own profiles.yml is used when it has one, else the directory
# PROFILES_DIR names (as dbt's --profiles-dir). IGNORE is an extended regex
# of names to leave out of both sides, for a difference already understood
# (e.g. tests whose refs dbt renders away on this target). Exits 1 when a
# selection differs. Not part of CI: it needs a dbt and a project.
set -eu

[ $# -ge 2 ] || { echo "usage: $0 <project dir> <selection>..." >&2; exit 2; }
project=$(cd "$1" && pwd)
shift
dbt=${DBT:-dbt}
profiles=""
if [ -n "${PROFILES_DIR:-}" ]; then
    profiles="--profiles-dir $(cd "$PROFILES_DIR" && pwd)"
elif [ -f "$project/profiles.yml" ]; then
    profiles="--profiles-dir $project"
fi

cd "$(dirname "$0")/.."
cargo build -q -p cogitator-analysis --example select
ours=target/debug/examples/select
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

status=0
for selection in "$@"; do
    # shellcheck disable=SC2086 # $dbt and $profiles are words on purpose
    $dbt ls --project-dir "$project" $profiles --select "$selection" --output name 2>/dev/null \
        | grep -E '^[A-Za-z0-9_.]+$' | sort -u >"$work/dbt" || true
    "$ours" "$project" "$selection" | sort -u >"$work/ours"
    if [ -n "${IGNORE:-}" ]; then
        for side in dbt ours; do
            grep -Ev "$IGNORE" "$work/$side" >"$work/$side.kept" || true
            mv "$work/$side.kept" "$work/$side"
        done
    fi
    if diff -u "$work/dbt" "$work/ours" >"$work/diff"; then
        printf 'same  %-45s %s nodes\n' "$selection" "$(wc -l <"$work/ours" | tr -d ' ')"
    else
        status=1
        printf 'DIFF  %s  (- dbt, + dbt.select)\n' "$selection"
        sed -n '3,$p' "$work/diff" | grep -E '^[-+]' | head -40
    fi
done
exit "$status"
