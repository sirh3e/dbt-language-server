// cogitator lineage site — the page: draws the DAG, the details panel and
// the column lineage from the data in #cogitator-data (the static site) or
// from a `site` message (the VS Code webview). Text goes in through
// textContent only; nothing is fetched.
(function () {
  "use strict";
  const L = self.CogitatorLineage;
  const SVG = "http://www.w3.org/2000/svg";
  const KINDS = ["source", "seed", "model", "snapshot", "exposure", "metric", "semantic_model", "analysis", "test"];
  const HIDDEN_AT_FIRST = new Set(["test", "analysis"]);
  const vscode = typeof acquireVsCodeApi === "function" ? acquireVsCodeApi() : null;

  const state = {
    site: null,
    idx: null,
    graph: null,
    hidden: new Set(HIDDEN_AT_FIRST),
    node: null,
    column: null,
    trace: null,
    view: { x: 20, y: 20, k: 1 },
    positions: new Map(),
    hits: [],
    hit: -1,
  };

  function el(tag, attrs, children) {
    const e = document.createElement(tag);
    for (const [k, v] of Object.entries(attrs || {})) {
      if (v === null || v === undefined || v === false) continue;
      if (k === "text") e.textContent = v;
      else if (k === "class") e.className = v;
      else if (k.startsWith("on")) e.addEventListener(k.slice(2), v);
      else e.setAttribute(k, v === true ? "" : v);
    }
    for (const c of children || []) if (c) e.append(c);
    return e;
  }

  function svg(tag, attrs) {
    const e = document.createElementNS(SVG, tag);
    for (const [k, v] of Object.entries(attrs || {})) {
      if (k === "text") e.textContent = v;
      else e.setAttribute(k, v);
    }
    return e;
  }

  const $ = (id) => document.getElementById(id);

  // ---------------------------------------------------------------- shell

  function build() {
    const app = $("app");
    app.replaceChildren(
      el("header", { class: "bar" }, [
        el("h1", { id: "title" }),
        el("p", { id: "counts", class: "counts" }),
        el("div", { class: "search" }, [
          el("input", {
            id: "search",
            type: "search",
            placeholder: "Search models and columns  ( / )",
            "aria-label": "Search models and columns",
            "aria-controls": "hits",
            "aria-autocomplete": "list",
            autocomplete: "off",
            oninput: onSearch,
            onkeydown: onSearchKey,
          }),
          el("ul", { id: "hits", role: "listbox", class: "hits", hidden: true }),
        ]),
        el("fieldset", { id: "kinds", class: "kinds" }, [el("legend", { text: "Show" })]),
        el("button", { id: "theme", type: "button", class: "theme", onclick: cycleTheme }),
      ]),
      el("main", { class: "layout" }, [
        el("section", { id: "canvas", class: "canvas", tabindex: "0", "aria-label": "Project graph: drag or arrow keys to pan, + and - to zoom, 0 to fit" }, [
          svgRoot(),
          el("div", { class: "zoom" }, [
            el("button", { type: "button", text: "+", "aria-label": "Zoom in", onclick: () => zoom(1.25) }),
            el("button", { type: "button", text: "−", "aria-label": "Zoom out", onclick: () => zoom(0.8) }),
            el("button", { type: "button", text: "Fit", onclick: fit }),
          ]),
          el("p", { class: "legend" }, [
            el("span", { class: "chip passthrough", text: "passed through" }),
            el("span", { class: "chip renamed", text: "renamed" }),
            el("span", { class: "chip derived", text: "derived" }),
            el("span", { class: "chip unknown", text: "not determined" }),
          ]),
        ]),
        el("aside", { id: "panel", class: "panel", "aria-live": "polite" }),
      ]),
      el("footer", { id: "footer", class: "footer" })
    );
    const canvas = $("canvas");
    canvas.addEventListener("wheel", onWheel, { passive: false });
    canvas.addEventListener("pointerdown", onPointerDown);
    canvas.addEventListener("keydown", onCanvasKey);
    document.addEventListener("keydown", (e) => {
      if (e.key === "/" && document.activeElement !== $("search")) {
        e.preventDefault();
        $("search").focus();
      } else if (e.key === "Escape" && state.column) {
        selectNode(state.node);
      }
    });
    document.addEventListener("click", onLinkClick);
    applyTheme(localTheme());
  }

  function svgRoot() {
    const root = svg("svg", { id: "graph", role: "presentation" });
    const view = svg("g", { id: "viewport" });
    view.append(svg("g", { id: "edges" }), svg("g", { id: "nodes" }));
    root.append(view);
    return root;
  }

  // ----------------------------------------------------------------- data

  function load(site, focus) {
    state.site = site;
    state.idx = L.index(site);
    state.graph = L.columnGraph(site);
    $("title").textContent = site.project + " — lineage";
    document.title = site.project + " — lineage";
    const c = site.counts;
    $("counts").textContent =
      `${c.models} models · ${c.sources} sources · ${c.seeds} seeds · ${c.snapshots} snapshots · ${c.exposures} exposures — ` +
      `columns traced ${c.columnsTraced} of ${c.columns}, undetermined ${c.columnsUndetermined} · ` +
      `models traced ${c.modelsTraced}, declined ${c.modelsDeclined}`;
    $("footer").textContent = `Generated by ${site.generator} from static analysis: no dbt run, no warehouse.`;
    kindFilters();
    const keep = state.node && state.idx.byId.has(state.node) ? state.node : null;
    const keepColumn = keep ? state.column : null;
    draw();
    if (focus && state.idx.byId.has(focus)) {
      selectNode(focus, { center: true });
    } else if (keep) {
      keepColumn ? selectColumn(keep, keepColumn) : selectNode(keep);
    } else {
      fit();
      showOverview();
    }
  }

  function kindFilters() {
    const box = $("kinds");
    box.replaceChildren(el("legend", { text: "Show" }));
    const present = new Set(state.site.nodes.map((n) => n.kind));
    for (const kind of KINDS.filter((k) => present.has(k))) {
      const id = "kind-" + kind;
      const input = el("input", {
        id,
        type: "checkbox",
        checked: !state.hidden.has(kind),
        onchange: (e) => {
          e.target.checked ? state.hidden.delete(kind) : state.hidden.add(kind);
          draw();
          fit();
        },
      });
      box.append(el("label", { for: id, class: "kind " + kind }, [input, document.createTextNode(kind.replace("_", " "))]));
    }
  }

  // ---------------------------------------------------------------- graph

  function visible() {
    return new Set(state.site.nodes.filter((n) => !state.hidden.has(n.kind)).map((n) => n.id));
  }

  function draw() {
    const shown = visible();
    const laid = L.layout(state.site, { visible: shown });
    state.positions = laid.positions;
    state.size = laid;
    const edges = $("edges");
    const nodes = $("nodes");
    edges.replaceChildren();
    nodes.replaceChildren();
    const lit = litEdges();
    for (const edge of state.site.edges) {
      const a = laid.positions.get(edge.parent);
      const b = laid.positions.get(edge.child);
      if (!a || !b) continue;
      const on = lit.has(edge.parent + "\u0000" + edge.child);
      edges.append(svg("path", { d: L.edgePath(a, b), class: "edge" + (on ? " lit" : state.trace ? " dim" : "") }));
    }
    for (const node of state.site.nodes) {
      const p = laid.positions.get(node.id);
      if (p) nodes.append(nodeShape(node, p));
    }
    applyView();
  }

  function litEdges() {
    const out = new Set();
    if (!state.trace) return out;
    for (const item of state.trace.upstream) {
      out.add(L.splitKey(item.key)[0] + "\u0000" + L.splitKey(item.via)[0]);
    }
    for (const item of state.trace.downstream) {
      out.add(L.splitKey(item.via)[0] + "\u0000" + L.splitKey(item.key)[0]);
    }
    return out;
  }

  function nodeShape(node, p) {
    const inTrace = state.trace && state.trace.nodes.has(node.id);
    const classes = ["node", node.kind];
    if (node.lineage === "declined") classes.push("declined");
    if (node.id === state.node) classes.push("selected");
    if (state.trace) classes.push(inTrace ? "lit" : "dim");
    const g = svg("g", {
      class: classes.join(" "),
      transform: `translate(${p.x},${p.y})`,
      tabindex: "0",
      role: "button",
      "aria-label": `${node.kind} ${node.name}${node.lineage === "declined" ? ", lineage not traced" : ""}`,
    });
    g.append(svg("rect", { width: p.w, height: p.h, rx: 6 }));
    g.append(svg("text", { x: 10, y: 16, class: "name", text: clip(node.name, 28) }));
    const mat = node.materialized && node.materialized !== node.kind ? node.materialized : "";
    let sub = node.kind + (mat ? " \u00b7 " + mat : "");
    if (inTrace && state.trace.columns.has(node.id)) {
      sub = "↳ " + [...state.trace.columns.get(node.id)].join(", ");
    }
    g.append(svg("text", { x: 10, y: 31, class: "sub", text: clip(sub, 34) }));
    if (node.lineage === "declined") {
      g.append(svg("text", { x: p.w - 16, y: 16, class: "badge", text: "?" }));
    }
    g.append(svg("title", { text: node.name }));
    g.addEventListener("click", (e) => {
      e.stopPropagation();
      selectNode(node.id);
    });
    g.addEventListener("keydown", (e) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        selectNode(node.id);
      }
    });
    return g;
  }

  function clip(text, n) {
    return text.length > n ? text.slice(0, n - 1) + "…" : text;
  }

  // ------------------------------------------------------------ selection

  function selectNode(id, opts) {
    state.node = id;
    state.column = null;
    state.trace = null;
    draw();
    showNode(state.idx.byId.get(id));
    if (opts && opts.center) center(id);
  }

  function selectColumn(id, column) {
    state.node = id;
    state.column = column;
    state.trace = L.trace(state.graph, id, column);
    draw();
    showNode(state.idx.byId.get(id));
  }

  function showOverview() {
    const c = state.site.counts;
    $("panel").replaceChildren(
      el("h2", { text: "Column lineage" }),
      el("p", { text: "Select a node for its details, then a column to follow it upstream and downstream." }),
      el("dl", { class: "facts" }, [
        fact("Nodes", `${c.nodes} (${c.edges} edges)`),
        fact("Columns traced", `${c.columnsTraced} of ${c.columns}`),
        fact("Undetermined", String(c.columnsUndetermined)),
        fact("Models declined", String(c.modelsDeclined)),
      ]),
      el("p", { class: "muted", text: "Undetermined: an unexpanded *, opaque SQL, or a model whose lineage was declined (Jinja control flow without fresh compiled SQL, or SQL that did not parse)." })
    );
  }

  function fact(name, value) {
    return el("div", {}, [el("dt", { text: name }), el("dd", { text: value })]);
  }

  function showNode(node) {
    if (!node) return showOverview();
    const panel = $("panel");
    const head = el("div", { class: "head" }, [
      el("h2", { text: node.name }),
      el("span", { class: "kind-badge " + node.kind, text: node.kind.replace("_", " ") }),
    ]);
    const facts = el("dl", { class: "facts" });
    if (node.path) facts.append(el("div", {}, [el("dt", { text: "File" }), el("dd", {}, [fileLink(node.uri, node.path)])]));
    if (node.materialized) facts.append(fact("Materialized", node.materialized));
    if (node.package) facts.append(fact("Package", node.package));
    facts.append(fact("Lineage", lineageWords(node)));
    const parts = [head, facts];
    if (node.note) parts.push(el("p", { class: "note", text: node.note }));
    if (node.description) parts.push(el("p", { class: "description", text: node.description }));
    if (node.tags.length) parts.push(list("Tags", node.tags));
    if (node.tests.length) parts.push(list("Tests", node.tests));
    parts.push(neighbours(node));
    if (state.column) parts.push(columnTrace(node));
    parts.push(columnList(node));
    panel.replaceChildren(...parts);
  }

  function lineageWords(node) {
    switch (node.lineage) {
      case "traced":
        return "traced from the template";
      case "compiled":
        return "traced from fresh compiled SQL";
      case "declined":
        return "not traced";
      default:
        return "nothing to trace";
    }
  }

  function list(title, items) {
    return el("section", {}, [el("h3", { text: title }), el("ul", { class: "inline" }, items.map((t) => el("li", { text: t })))]);
  }

  function neighbours(node) {
    const link = (id) => {
      const n = state.idx.byId.get(id);
      return el("li", {}, [el("button", { type: "button", class: "link", text: n ? n.name : id, onclick: () => selectNode(id, { center: true }) })]);
    };
    const ups = state.idx.parents.get(node.id) || [];
    const downs = state.idx.children.get(node.id) || [];
    return el("section", { class: "neighbours" }, [
      el("h3", { text: `Upstream (${ups.length}) · Downstream (${downs.length})` }),
      el("ul", { class: "inline" }, ups.map(link)),
      el("ul", { class: "inline down" }, downs.map(link)),
    ]);
  }

  function columnList(node) {
    const items = node.columns.map((c) => {
      const undetermined = c.star || (c.produced && !c.inputs.length && c.transform !== "constant");
      const button = el(
        "button",
        {
          type: "button",
          class: "column" + (c.name === state.column ? " selected" : "") + (undetermined ? " undetermined" : ""),
          "aria-pressed": c.name === state.column ? "true" : "false",
          onclick: () => selectColumn(node.id, c.name),
          onkeydown: (e) => columnKey(e),
        },
        [
          el("span", { class: "cname", text: c.name }),
          c.dataType ? el("span", { class: "ctype", text: c.dataType }) : null,
          c.transform ? el("span", { class: "chip " + c.transform, text: L.transformWords(c) }) : null,
          c.star ? el("span", { class: "chip unknown", text: "columns not known" }) : null,
          !c.produced && !c.documented ? el("span", { class: "chip muted", text: "seen downstream only" }) : null,
        ]
      );
      const extra = [];
      if (c.description) extra.push(el("p", { class: "cdesc", text: c.description }));
      if (c.tests.length) extra.push(el("p", { class: "ctests", text: "tests: " + c.tests.join(", ") }));
      return el("li", {}, [button, ...extra]);
    });
    return el("section", {}, [
      el("h3", { text: `Columns (${node.columns.length})` }),
      node.columns.length ? el("ul", { class: "columns" }, items) : el("p", { class: "muted", text: "No columns known." }),
    ]);
  }

  function columnKey(e) {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    const buttons = [...document.querySelectorAll("#panel .columns button.column")];
    const at = buttons.indexOf(e.currentTarget);
    const next = buttons[at + (e.key === "ArrowDown" ? 1 : -1)];
    if (next) next.focus();
  }

  function columnTrace(node) {
    const t = state.trace;
    const entry = (item) => {
      const [id, name] = L.splitKey(item.key);
      const target = state.idx.byId.get(id);
      const column = target && target.columns.find((c) => c.name === name);
      const words = column ? L.transformWords(column) : "";
      return el("li", { class: "depth-" + Math.min(item.depth, 6) }, [
        el("button", {
          type: "button",
          class: "link",
          text: `${target ? target.name : id}.${name}`,
          onclick: () => selectColumn(id, name),
        }),
        words ? el("span", { class: "chip " + (column.transform || "unknown"), text: words }) : null,
        target && target.lineage === "declined" ? el("span", { class: "chip unknown", text: "its model is not traced" }) : null,
      ]);
    };
    const own = node.columns.find((c) => c.name === state.column);
    const parts = [
      el("h3", { text: `Lineage of ${node.name}.${state.column}` }),
      own ? el("p", { class: "muted", text: L.transformWords(own) || "how it is made is not known" }) : null,
      el("h4", { text: `Upstream (${t.upstream.length})` }),
      t.upstream.length ? el("ul", { class: "trace" }, t.upstream.map(entry)) : el("p", { class: "muted", text: upstreamEmpty(node, own) }),
    ];
    if (t.outside.length) {
      parts.push(el("h4", { text: "Outside the project" }));
      parts.push(el("ul", { class: "trace" }, t.outside.map((h) => el("li", { text: `${h.relation}.${h.column}` }))));
    }
    parts.push(el("h4", { text: `Downstream (${t.downstream.length})` }));
    parts.push(t.downstream.length ? el("ul", { class: "trace" }, t.downstream.map(entry)) : el("p", { class: "muted", text: "Nothing traced reads it." }));
    return el("section", { class: "lineage" }, parts);
  }

  function upstreamEmpty(node, column) {
    if (node.lineage === "declined") return "Not traced: " + (node.note || "this model's lineage was declined.");
    if (!column || !column.produced) return "Not produced by SQL cogitator traced.";
    if (column.star) return "An unexpanded *: its columns are not known.";
    if (column.transform === "constant") return "A literal: no column feeds it.";
    return "No upstream column found.";
  }

  // ---------------------------------------------------------------- links

  function fileLink(uri, label) {
    if (!uri) return document.createTextNode(label);
    return el("a", { href: uri, class: "file", "data-uri": uri, text: label });
  }

  function onLinkClick(e) {
    const a = e.target.closest && e.target.closest("a.file");
    if (!a || !vscode) return;
    e.preventDefault();
    vscode.postMessage({ type: "open", uri: a.dataset.uri, column: state.column });
  }

  // ------------------------------------------------------------- pan/zoom

  function applyView() {
    const v = state.view;
    $("viewport").setAttribute("transform", `translate(${v.x},${v.y}) scale(${v.k})`);
  }

  function zoom(factor, cx, cy) {
    const box = $("canvas").getBoundingClientRect();
    const x = cx === undefined ? box.width / 2 : cx;
    const y = cy === undefined ? box.height / 2 : cy;
    const v = state.view;
    const k = Math.min(4, Math.max(0.1, v.k * factor));
    v.x = x - ((x - v.x) * k) / v.k;
    v.y = y - ((y - v.y) * k) / v.k;
    v.k = k;
    applyView();
  }

  /** The whole graph when it fits readably, else its left end at a scale
   *  whose labels can still be read (pan or zoom out for the rest). */
  function fit() {
    const box = $("canvas").getBoundingClientRect();
    const size = state.size || { width: 1, height: 1 };
    const whole = Math.min((box.width - 40) / (size.width || 1), (box.height - 40) / (size.height || 1));
    const k = Math.min(1.2, Math.max(0.6, whole));
    const x = size.width * k <= box.width - 40 ? (box.width - size.width * k) / 2 : 20;
    const y = size.height * k <= box.height - 40 ? (box.height - size.height * k) / 2 : 20;
    state.view = { k, x, y };
    applyView();
  }

  function center(id) {
    const p = state.positions.get(id);
    if (!p) return;
    const box = $("canvas").getBoundingClientRect();
    const v = state.view;
    v.x = box.width / 2 - (p.x + p.w / 2) * v.k;
    v.y = box.height / 2 - (p.y + p.h / 2) * v.k;
    applyView();
  }

  function onWheel(e) {
    e.preventDefault();
    const box = $("canvas").getBoundingClientRect();
    zoom(e.deltaY < 0 ? 1.1 : 1 / 1.1, e.clientX - box.left, e.clientY - box.top);
  }

  function onPointerDown(e) {
    if (e.target.closest && (e.target.closest("g.node") || e.target.closest("button"))) return;
    const start = { x: e.clientX, y: e.clientY, vx: state.view.x, vy: state.view.y };
    const move = (m) => {
      state.view.x = start.vx + m.clientX - start.x;
      state.view.y = start.vy + m.clientY - start.y;
      applyView();
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }

  function onCanvasKey(e) {
    if (e.target !== $("canvas")) return;
    const step = 60;
    const moves = { ArrowLeft: [step, 0], ArrowRight: [-step, 0], ArrowUp: [0, step], ArrowDown: [0, -step] };
    if (moves[e.key]) {
      state.view.x += moves[e.key][0];
      state.view.y += moves[e.key][1];
      applyView();
    } else if (e.key === "+" || e.key === "=") zoom(1.25);
    else if (e.key === "-") zoom(0.8);
    else if (e.key === "0") fit();
    else return;
    e.preventDefault();
  }

  // --------------------------------------------------------------- search

  function onSearch() {
    state.hits = L.search(state.site, $("search").value, 30);
    state.hit = state.hits.length ? 0 : -1;
    showHits();
  }

  function showHits() {
    const ul = $("hits");
    ul.replaceChildren(
      ...state.hits.map((h, i) =>
        el("li", {
          id: "hit-" + i,
          role: "option",
          class: h.type,
          "aria-selected": i === state.hit ? "true" : "false",
          text: (h.type === "column" ? "column  " : "node  ") + h.label,
          onmousedown: (e) => {
            e.preventDefault();
            choose(i);
          },
        })
      )
    );
    ul.hidden = !state.hits.length;
    const input = $("search");
    if (state.hit >= 0) input.setAttribute("aria-activedescendant", "hit-" + state.hit);
    else input.removeAttribute("aria-activedescendant");
  }

  function onSearchKey(e) {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      if (!state.hits.length) return;
      const n = state.hits.length;
      state.hit = (state.hit + (e.key === "ArrowDown" ? 1 : n - 1)) % n;
      showHits();
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (state.hit >= 0) choose(state.hit);
    } else if (e.key === "Escape") {
      $("search").value = "";
      state.hits = [];
      showHits();
    }
  }

  function choose(i) {
    const h = state.hits[i];
    if (!h) return;
    state.hidden.delete(state.idx.byId.get(h.node).kind);
    kindFilters();
    if (h.type === "column") selectColumn(h.node, h.column);
    else selectNode(h.node);
    center(h.node);
    state.hits = [];
    showHits();
  }

  // ---------------------------------------------------------------- theme

  function localTheme() {
    try {
      return localStorage.getItem("cogitator-lineage-theme") || "auto";
    } catch (_) {
      return "auto";
    }
  }

  function applyTheme(theme) {
    const vscodeDark = document.body.classList.contains("vscode-dark") || document.body.classList.contains("vscode-high-contrast");
    const resolved = theme === "auto" && vscode ? (vscodeDark ? "dark" : "light") : theme;
    if (resolved === "auto") document.documentElement.removeAttribute("data-theme");
    else document.documentElement.setAttribute("data-theme", resolved);
    $("theme").textContent = "Theme: " + theme;
    $("theme").dataset.theme = theme;
  }

  function cycleTheme() {
    const order = ["auto", "light", "dark"];
    const next = order[(order.indexOf($("theme").dataset.theme) + 1) % order.length];
    try {
      localStorage.setItem("cogitator-lineage-theme", next);
    } catch (_) {
      /* storage may be off for file:// pages */
    }
    applyTheme(next);
  }

  // ----------------------------------------------------------------- boot

  build();
  const embedded = $("cogitator-data");
  if (embedded && embedded.textContent.trim()) {
    load(JSON.parse(embedded.textContent), null);
  }
  window.addEventListener("message", (e) => {
    const msg = e.data || {};
    if (msg.type === "site") load(msg.site, msg.focus || null);
    else if (msg.type === "focus" && state.idx && state.idx.byId.has(msg.node)) selectNode(msg.node, { center: true });
  });
  window.addEventListener("resize", () => applyView());
  if (vscode) vscode.postMessage({ type: "ready" });
})();
