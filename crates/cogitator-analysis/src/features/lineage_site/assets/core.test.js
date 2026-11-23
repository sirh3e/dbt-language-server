// node --test crates/cogitator-analysis/src/features/lineage_site/assets/core.test.js
// (also run by crates/cogitator-analysis/tests/lineage_site_js.rs).
"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const L = require("./core.js");

const col = (name, inputs, extra) =>
  Object.assign({ name, inputs: inputs || [], star: false, transform: null, detail: null }, extra || {});
const hop = (node, column) => ({ node, column, relation: null });

// raw -> stg -> mart, plus a lone source
const site = {
  project: "p",
  nodes: [
    { id: "model:raw", name: "raw", kind: "seed", layer: 0, columns: [col("id")] },
    { id: "source:s", name: "s", kind: "source", layer: 0, columns: [col("t.x")] },
    {
      id: "model:stg",
      name: "stg",
      kind: "model",
      layer: 1,
      columns: [
        col("customer_id", [hop("model:raw", "id")], { transform: "renamed", detail: "id" }),
        col("x", [hop("source:s", "t.x"), { node: null, column: "y", relation: "raw.tbl" }]),
      ],
    },
    {
      id: "model:mart",
      name: "mart",
      kind: "model",
      layer: 2,
      columns: [col("customer_id", [hop("model:stg", "customer_id")], { transform: "passthrough" })],
    },
  ],
  edges: [
    { parent: "model:raw", child: "model:stg" },
    { parent: "source:s", child: "model:stg" },
    { parent: "model:stg", child: "model:mart" },
  ],
};

test("keys round-trip", () => {
  assert.deepEqual(L.splitKey(L.columnKey("model:a", "b.c")), ["model:a", "b.c"]);
});

test("index knows parents and children", () => {
  const idx = L.index(site);
  assert.deepEqual(idx.parents.get("model:stg"), ["model:raw", "source:s"]);
  assert.deepEqual(idx.children.get("model:stg"), ["model:mart"]);
});

test("a column is traced upstream and downstream across models", () => {
  const graph = L.columnGraph(site);
  const t = L.trace(graph, "model:stg", "customer_id");
  assert.deepEqual(
    t.upstream.map((u) => [L.splitKey(u.key), u.depth]),
    [[["model:raw", "id"], 1]]
  );
  assert.deepEqual(
    t.downstream.map((d) => L.splitKey(d.key)),
    [["model:mart", "customer_id"]]
  );
  assert.deepEqual([...t.nodes].sort(), ["model:mart", "model:raw", "model:stg"]);
  const fromTop = L.trace(graph, "model:mart", "customer_id");
  assert.deepEqual(
    fromTop.upstream.map((u) => [L.splitKey(u.key)[0], u.depth]),
    [["model:stg", 1], ["model:raw", 2]]
  );
});

test("relations outside the project are kept apart", () => {
  const t = L.trace(L.columnGraph(site), "model:stg", "x");
  assert.equal(t.upstream.length, 1);
  assert.deepEqual(t.outside, [{ node: null, column: "y", relation: "raw.tbl" }]);
});

test("a cycle does not loop", () => {
  const cyclic = {
    nodes: [
      { id: "a", name: "a", layer: 0, columns: [col("c", [hop("b", "c")])] },
      { id: "b", name: "b", layer: 1, columns: [col("c", [hop("a", "c")])] },
    ],
    edges: [],
  };
  const t = L.trace(L.columnGraph(cyclic), "a", "c");
  assert.equal(t.upstream.length, 1);
});

test("layout goes left to right without overlaps", () => {
  const { positions, width, height } = L.layout(site);
  const p = (id) => positions.get(id);
  assert.ok(p("model:raw").x < p("model:stg").x);
  assert.ok(p("model:stg").x < p("model:mart").x);
  assert.equal(p("model:raw").x, p("source:s").x);
  assert.notEqual(p("model:raw").y, p("source:s").y);
  assert.ok(width > 0 && height > 0);
  const hidden = L.layout(site, { visible: new Set(["model:stg", "model:mart"]) });
  assert.equal(hidden.positions.size, 2);
  assert.equal(hidden.positions.get("model:stg").x, 0);
});

test("layout is the same every time", () => {
  const a = [...L.layout(site).positions.entries()];
  const b = [...L.layout(site).positions.entries()];
  assert.deepEqual(a, b);
});

test("edges run from the right of the parent to the left of the child", () => {
  const d = L.edgePath({ x: 0, y: 0, w: 10, h: 10 }, { x: 100, y: 20, w: 10, h: 10 });
  assert.match(d, /^M10,5 C/);
  assert.match(d, / 100,25$/);
});

test("search ranks exact, then prefix, then the rest; nodes first", () => {
  const hits = L.search(site, "stg");
  assert.equal(hits[0].type, "node");
  assert.equal(hits[0].label, "stg");
  const columns = L.search(site, "customer");
  assert.deepEqual(
    columns.map((h) => h.label),
    ["mart.customer_id", "stg.customer_id"]
  );
  assert.deepEqual(L.search(site, "mart customer").map((h) => h.label), ["mart.customer_id"]);
  assert.deepEqual(L.search(site, "   "), []);
  assert.equal(L.search(site, "", 1).length, 0);
});

test("transforms are said in words", () => {
  assert.equal(L.transformWords(col("a", [], { transform: "renamed", detail: "id" })), "renamed from id");
  assert.equal(L.transformWords(col("a", [], { transform: "derived", detail: "sum() aggregate" })), "derived: sum() aggregate");
  assert.equal(L.transformWords(col("a", [], { transform: "passthrough" })), "passed through");
  assert.equal(L.transformWords(col("*", [], { star: true })), "columns of * not known");
  assert.equal(L.transformWords(col("a")), "");
});
