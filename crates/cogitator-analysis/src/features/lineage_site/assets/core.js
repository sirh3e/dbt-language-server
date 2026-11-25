// cogitator lineage site — the pure part: indexes, layout, the column
// walk and search. No DOM; tested with `node --test` (core.test.js).
// Embedded in the page by crates/cogitator-analysis (features/lineage_site).
(function (root, factory) {
  "use strict";
  const core = factory();
  if (typeof module === "object" && module.exports) {
    module.exports = core;
  } else {
    root.CogitatorLineage = core;
  }
})(typeof self !== "undefined" ? self : this, function () {
  "use strict";

  const SEP = "\u0000";

  /** The key of one column: node id and column name. */
  function columnKey(node, column) {
    return node + SEP + column;
  }

  /** [node, column] of a key. */
  function splitKey(key) {
    const at = key.indexOf(SEP);
    return [key.slice(0, at), key.slice(at + 1)];
  }

  /** Nodes by id, and the parents and children of each. */
  function index(site) {
    const byId = new Map();
    const parents = new Map();
    const children = new Map();
    for (const node of site.nodes) {
      byId.set(node.id, node);
      parents.set(node.id, []);
      children.set(node.id, []);
    }
    for (const edge of site.edges) {
      if (!byId.has(edge.parent) || !byId.has(edge.child)) continue;
      parents.get(edge.child).push(edge.parent);
      children.get(edge.parent).push(edge.child);
    }
    return { byId, parents, children };
  }

  /** Column-level edges: `up` maps a column to what it reads, `down` to
   *  what reads it. Hops to relations outside the project are kept apart. */
  function columnGraph(site) {
    const up = new Map();
    const down = new Map();
    const outside = new Map();
    for (const node of site.nodes) {
      for (const column of node.columns) {
        const key = columnKey(node.id, column.name);
        const reads = [];
        for (const hop of column.inputs) {
          if (hop.node === null || hop.node === undefined) {
            if (!outside.has(key)) outside.set(key, []);
            outside.get(key).push(hop);
            continue;
          }
          const from = columnKey(hop.node, hop.column);
          reads.push(from);
          if (!down.has(from)) down.set(from, []);
          down.get(from).push(key);
        }
        up.set(key, reads);
      }
    }
    return { up, down, outside };
  }

  /** Every column reachable from `start` along `edges` (a Map), with its
   *  distance, nearest first; `start` itself is left out. */
  function walk(edges, start) {
    const seen = new Map([[start, 0]]);
    const queue = [start];
    const out = [];
    while (queue.length) {
      const key = queue.shift();
      const depth = seen.get(key);
      for (const next of edges.get(key) || []) {
        if (seen.has(next)) continue;
        seen.set(next, depth + 1);
        out.push({ key: next, via: key, depth: depth + 1 });
        queue.push(next);
      }
    }
    return out;
  }

  /** The upstream and downstream columns of one column, and the nodes they
   *  touch (the column's own node included). */
  function trace(graph, node, column) {
    const start = columnKey(node, column);
    const upstream = walk(graph.up, start);
    const downstream = walk(graph.down, start);
    const nodes = new Set([node]);
    const columns = new Map([[node, new Set([column])]]);
    for (const item of upstream.concat(downstream)) {
      const [n, c] = splitKey(item.key);
      nodes.add(n);
      if (!columns.has(n)) columns.set(n, new Set());
      columns.get(n).add(c);
    }
    const outside = [];
    for (const key of [start].concat(upstream.map((u) => u.key))) {
      for (const hop of graph.outside.get(key) || []) outside.push(hop);
    }
    return { upstream, downstream, nodes, columns, outside };
  }

  /** Left-to-right layout: one column per DAG layer, rows ordered by the
   *  mean row of each node's parents (two sweeps), then by name. Only the
   *  nodes in `visible` (all when absent) take part. */
  function layout(site, options) {
    const opts = Object.assign(
      { width: 220, height: 40, gapX: 90, gapY: 16, visible: null },
      options || {}
    );
    const { parents, children } = index(site);
    const shown = site.nodes.filter((n) => !opts.visible || opts.visible.has(n.id));
    const layers = [...new Set(shown.map((n) => n.layer))].sort((a, b) => a - b);
    const column = new Map(layers.map((l, i) => [l, i]));
    const byLayer = layers.map(() => []);
    for (const node of shown) byLayer[column.get(node.layer)].push(node);
    const row = new Map();
    for (const nodes of byLayer) {
      nodes.sort(byName);
      nodes.forEach((n, i) => row.set(n.id, i));
    }
    const mean = (ids) => {
      const rows = ids.filter((id) => row.has(id)).map((id) => row.get(id));
      return rows.length ? rows.reduce((a, b) => a + b, 0) / rows.length : null;
    };
    const sweep = (nodes, neighbors) => {
      const keyed = nodes.map((n, i) => ({ n, k: mean(neighbors.get(n.id) || []), i }));
      keyed.sort((a, b) => {
        const ka = a.k === null ? a.i : a.k;
        const kb = b.k === null ? b.i : b.k;
        return ka - kb || byName(a.n, b.n);
      });
      keyed.forEach((e, i) => {
        nodes[i] = e.n;
        row.set(e.n.id, i);
      });
    };
    for (let pass = 0; pass < 2; pass++) {
      for (let i = 1; i < byLayer.length; i++) sweep(byLayer[i], parents);
      for (let i = byLayer.length - 2; i >= 0; i--) sweep(byLayer[i], children);
    }
    const tallest = Math.max(0, ...byLayer.map((n) => n.length));
    const span = tallest * (opts.height + opts.gapY);
    const positions = new Map();
    byLayer.forEach((nodes, x) => {
      const offset = (span - nodes.length * (opts.height + opts.gapY)) / 2;
      nodes.forEach((n, y) => {
        positions.set(n.id, {
          x: x * (opts.width + opts.gapX),
          y: offset + y * (opts.height + opts.gapY),
          w: opts.width,
          h: opts.height,
        });
      });
    });
    return {
      positions,
      width: Math.max(0, byLayer.length * (opts.width + opts.gapX) - opts.gapX),
      height: Math.max(0, span - opts.gapY),
    };
  }

  function byName(a, b) {
    return a.name < b.name ? -1 : a.name > b.name ? 1 : a.id < b.id ? -1 : a.id > b.id ? 1 : 0;
  }

  /** An SVG path from the right edge of `a` to the left edge of `b`. */
  function edgePath(a, b) {
    const x1 = a.x + a.w;
    const y1 = a.y + a.h / 2;
    const x2 = b.x;
    const y2 = b.y + b.h / 2;
    const bend = Math.max(30, (x2 - x1) / 2);
    return `M${x1},${y1} C${x1 + bend},${y1} ${x2 - bend},${y2} ${x2},${y2}`;
  }

  /** Nodes and columns whose name holds every word of `query`, best first:
   *  exact names, then prefixes, then the rest; nodes before columns. */
  function search(site, query, limit) {
    const words = String(query || "").toLowerCase().split(/\s+/).filter(Boolean);
    if (!words.length) return [];
    const max = limit || 50;
    const score = (text) => {
      const t = text.toLowerCase();
      if (!words.every((w) => t.includes(w))) return null;
      const q = words.join(" ");
      return t === q ? 0 : t.startsWith(words[0]) ? 1 : 2;
    };
    const hits = [];
    for (const node of site.nodes) {
      const s = score(node.name);
      if (s !== null) hits.push({ type: "node", node: node.id, label: node.name, rank: s, kind: 0 });
      for (const column of node.columns) {
        const c = score(column.name) ?? score(node.name + "." + column.name);
        if (c !== null) {
          hits.push({
            type: "column",
            node: node.id,
            column: column.name,
            label: node.name + "." + column.name,
            rank: c,
            kind: 1,
          });
        }
      }
    }
    hits.sort(
      (a, b) => a.rank - b.rank || a.kind - b.kind || (a.label < b.label ? -1 : a.label > b.label ? 1 : 0)
    );
    return hits.slice(0, max);
  }

  /** What a column's transform says, in words. */
  function transformWords(column) {
    switch (column.transform) {
      case "passthrough":
        return "passed through";
      case "renamed":
        return column.detail ? `renamed from ${column.detail}` : "renamed";
      case "derived":
        return column.detail ? `derived: ${column.detail}` : "derived";
      case "constant":
        return "a literal";
      case "unknown":
        return "how it is made is not known";
      default:
        return column.star ? "columns of * not known" : "";
    }
  }

  return {
    columnKey,
    splitKey,
    index,
    columnGraph,
    walk,
    trace,
    layout,
    edgePath,
    search,
    transformWords,
  };
});
