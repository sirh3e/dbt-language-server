#!/usr/bin/env python3
"""Preprocess vendored dbt-jsonschema files for cargo-typify 0.8 (08 §2.2).

typify limitations worked around here:
  1. if/then/else is unsupported            -> stripped (validation-only).
  2. enum values differing only in case     -> panic; deduped (keep first).
  3. `+alias` and `alias` style properties sanitize to one Rust ident
     -> duplicate struct fields; the `+`-prefixed spelling is dropped
        (unknown keys are ignored at deserialization, so files using the
        dropped spelling still parse).
  4. oneOf/anyOf of [X, null] names the inner type after the outer def ->
     duplicate type definitions; the null branch is dropped and X merged into
     the parent node (optional fields are Option<T> anyway, so a YAML `null`
     still deserializes to None).

Usage: preprocess.py <in.json> <out.json>
"""

import json
import re
import sys

def ident(s: str) -> str:
    return re.sub(r"[^a-z0-9]+", "_", s.lower()).strip("_")


def is_null_branch(b) -> bool:
    return isinstance(b, dict) and (
        b.get("type") == "null" or ("const" in b and b["const"] is None)
    )


def patch_node(node: dict) -> None:
    # Rule 5: schema `default` values are dropped entirely — typify 0.8 emits
    # broken default-constructor code for several of them, and the server
    # applies its own documented defaults (specs/rust-port/03 §3.3).
    for k in ("if", "then", "else", "default"):
        node.pop(k, None)
    enum = node.get("enum")
    if isinstance(enum, list) and all(isinstance(x, str) for x in enum):
        seen, out = set(), []
        for v in enum:
            if v.lower() not in seen:
                seen.add(v.lower())
                out.append(v)
        node["enum"] = out
    props = node.get("properties")
    if isinstance(props, dict):
        by_ident = {}
        for k in list(props):
            i = ident(k)
            if i in by_ident:
                drop = k if k.startswith("+") else by_ident[i]
                keep = by_ident[i] if k.startswith("+") else k
                props.pop(drop, None)
                by_ident[i] = keep
            else:
                by_ident[i] = k
    for key in ("oneOf", "anyOf"):
        branches = node.get(key)
        if not isinstance(branches, list) or len(branches) != 2:
            continue
        non_null = [b for b in branches if not is_null_branch(b)]
        if len(non_null) == 1 and isinstance(non_null[0], dict):
            node.pop(key)
            for k, v in non_null[0].items():
                node.setdefault(k, v)


def walk(node) -> None:
    if isinstance(node, dict):
        patch_node(node)
        for v in node.values():
            walk(v)
    elif isinstance(node, list):
        for v in node:
            walk(v)


def main() -> None:
    src = json.load(open(sys.argv[1]))
    walk(src)
    json.dump(src, open(sys.argv[2], "w"), indent=1)


if __name__ == "__main__":
    main()
