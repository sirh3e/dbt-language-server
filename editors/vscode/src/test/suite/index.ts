// The extension-host suite, hand-rolled on the shared ok/FAIL step
// contract (no mocha): activation, the LanguageClient reaching
// Running, the dbt.status refresh barrier, and the wiring proofs —
// hover/definition through VS Code's provider pipeline and the dbt.*
// command surface. Server intelligence itself is covered by the Rust
// e2e suites; this file proves the VS Code plumbing.

import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import type { CogitatorApi } from "../../extension";
import {
  astGrepLabel,
  breakingLines,
  captureColor,
  incrementalDiffText,
  incrementalityLabel,
  languageOf,
  nodeMarkdown,
  parseAstGrep,
  parseNode,
  previewCteText,
  skippedLayers,
  unresolvedLabel,
  selectLabel,
  syncYamlLines,
  syncYamlSummary,
  spansByColor,
  splitText,
  syntaxLine,
} from "../../extension";

let failures = 0;

function check(ok: unknown, label: string, detail?: unknown): boolean {
  if (ok) {
    console.log(`ok   ${label}`);
    return true;
  }
  failures += 1;
  console.log(
    `FAIL ${label}${detail === undefined ? "" : ` — ${JSON.stringify(detail)}`}`
  );
  return false;
}

/** A labeled wait that FAILS at its deadline — the nvim/emacs rule. */
async function waitFor<T>(
  label: string,
  ms: number,
  probe: () => Promise<T | undefined> | T | undefined
): Promise<T | undefined> {
  const due = Date.now() + ms;
  for (;;) {
    const value = await probe();
    if (value !== undefined && value !== false) {
      return value;
    }
    if (Date.now() >= due) {
      failures += 1;
      console.log(`FAIL ${label} (timed out after ${ms}ms)`);
      return undefined;
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
}

export async function run(): Promise<void> {
  const workspace = process.env.COGITATOR_TEST_WORKSPACE;
  if (!workspace) {
    throw new Error("COGITATOR_TEST_WORKSPACE not set");
  }

  // activation: workspaceContains:**/dbt_project.yml fires on its own;
  // ext.activate() awaits it and hands back the API
  const ext = vscode.extensions.getExtension<CogitatorApi>("sirh3e.cogitator");
  check(ext !== undefined, "vscode: extension is present");
  if (!ext) {
    throw new Error(`${failures} failure(s)`);
  }
  const api = await ext.activate();
  check(ext.isActive, "vscode: extension activates on dbt_project.yml");

  // the client reaches Running (State.Running === 2)
  const client = await waitFor("vscode: language client running", 15000, () => {
    const c = api.client();
    return c && c.state === 2 ? c : undefined;
  });
  if (!client) {
    throw new Error(`${failures} failure(s)`);
  }
  console.log("ok   vscode: language client running");

  const exec = (command: string, argument: Record<string, unknown>) =>
    client.sendRequest("workspace/executeCommand", {
      command,
      arguments: [argument],
    }) as Promise<Record<string, unknown> | null>;

  // THE readiness barrier: dbt.status refresh.count >= 1 (spec §8)
  const ready = await waitFor("vscode: refresh barrier (count >= 1)", 30000, async () => {
    const status = (await exec("dbt.status", {})) as {
      refresh?: { count?: number };
    } | null;
    return (status?.refresh?.count ?? 0) >= 1 ? status : undefined;
  });
  if (!ready) {
    throw new Error(`${failures} failure(s)`);
  }
  console.log("ok   vscode: first context refresh completed (dbt.status barrier)");

  // open the model THROUGH VS Code so the client routes didOpen
  const modelPath = path.join(workspace, "models", "customers.sql");
  const document = await vscode.workspace.openTextDocument(modelPath);
  await vscode.window.showTextDocument(document);
  check(document.languageId === "sql", "vscode: model routes as sql", document.languageId);

  // position inside the first ref('…') argument
  const text = fs.readFileSync(modelPath, "utf8");
  const offset = text.indexOf("stg_customers");
  check(offset > 0, "vscode: fixture model contains a ref()");
  const position = document.positionAt(offset + 1);

  // hover through the PROVIDER PIPELINE (documentSelector proof)
  const hovers = await vscode.commands.executeCommand<vscode.Hover[]>(
    "vscode.executeHoverProvider",
    document.uri,
    position
  );
  check(Array.isArray(hovers), "vscode: hover answers through the provider", hovers);

  // definition resolves across the project
  const definitions = await vscode.commands.executeCommand<vscode.Location[]>(
    "vscode.executeDefinitionProvider",
    document.uri,
    position
  );
  check(
    definitions?.some((loc) => loc.uri.fsPath.endsWith("stg_customers.sql")),
    "vscode: definition jumps to stg_customers.sql",
    definitions?.map((loc) => loc.uri.fsPath)
  );

  // vars: the value after var() as an inlay hint
  const hints = await vscode.commands.executeCommand<vscode.InlayHint[]>(
    "vscode.executeInlayHintProvider",
    document.uri,
    new vscode.Range(0, 0, document.lineCount, 0)
  );
  const hintLabels = (hints ?? []).map((h) =>
    typeof h.label === "string" ? h.label : h.label.map((p) => p.value).join("")
  );
  check(
    hintLabels.includes("= 'jaffle'"),
    "vscode: a var's value is an inlay hint",
    hintLabels
  );
  check(
    captureColor("variable.dbt.var") === "symbolIcon.variableForeground" &&
      captureColor("variable.dbt.var.unresolved") === "editorWarning.foreground",
    "vscode: a var in YAML has a colour, an undefined one a warning's"
  );

  // the dbt.* command surface over the live client
  const graph = (await exec("dbt.modelGraph", {
    uri: document.uri.toString(),
  })) as { model?: string; upstream?: Array<{ name?: string }> } | null;
  check(graph?.model === "customers", "vscode: dbt.modelGraph answers", graph);
  check(
    graph?.upstream?.some((e) => e.name === "stg_customers"),
    "vscode: direct upstream lists stg_customers",
    graph?.upstream
  );

  // the incrementality surface: the project list and the split plan
  const report = (await exec("dbt.incrementality", {
    uri: document.uri.toString(),
  })) as { models?: unknown[]; incremental?: number } | null;
  check(
    Array.isArray(report?.models) && typeof report?.incremental === "number",
    "vscode: dbt.incrementality lists the table models",
    report
  );
  const split = (await exec("dbt.splitPlan", {
    uri: document.uri.toString(),
  })) as { sexp?: string } | null;
  check(
    split === null || typeof split?.sexp === "string",
    "vscode: dbt.splitPlan answers a plan or null",
    split
  );
  // customers.sql calls a package macro the preview cannot stub
  const diff = (await exec("dbt.incrementalDiff", {
    uri: vscode.Uri.file(path.join(workspace, "models", "staging", "stg_customers.sql")).toString(),
  })) as { fullRefresh?: string; incremental?: string } | null;
  check(
    typeof diff?.fullRefresh === "string" && typeof diff?.incremental === "string",
    "vscode: dbt.incrementalDiff renders both ways",
    diff
  );
  check(
    incrementalDiffText({
      model: "m",
      fullRefresh: "select 1\n",
      incremental: "select 1\nwhere x\n",
      diff: "--- a\n+++ b\n",
      identical: false,
    }) === "--- a\n+++ b\n",
    "vscode: incrementalDiffText shows the unified diff"
  );
  check(
    previewCteText({
      model: "m",
      columns: ["id"],
      rows: [[1]],
      truncated: false,
      sql: "select 1",
      source: "compiled",
      table: "| id |\n| --- |\n| 1 |\n",
    }) === "| id |\n| --- |\n| 1 |\n",
    "vscode: previewCteText is the server's table"
  );
  check(
    splitText({
      model: "m",
      sexp: "(split m)",
      part: { name: "m__c", cte: "c", text: "select 1\n" },
    }) === "(split m)\n\n;; m__c.sql\nselect 1\n",
    "vscode: splitText renders the plan and the new model"
  );
  check(
    incrementalityLabel({
      model: "big",
      uri: "file:///p/big.sql",
      lastRunSeconds: 12.5,
      view: { verdict: "append", strategy: "append", summary: "append by ts" },
    }) === "big — append (append) [last run 12.5s]: append by ts",
    "vscode: incrementality labels name verdict, strategy, run time and why"
  );

  // the syntax tree: the open model's tree, and how a line reads
  const tree = (await exec("dbt.syntaxTree", {
    uri: document.uri.toString(),
  })) as { nodes?: Array<{ depth?: number }> } | null;
  check(
    (tree?.nodes?.length ?? 0) > 0 && tree?.nodes?.[0]?.depth === 0,
    "vscode: dbt.syntaxTree answers the model's tree",
    tree?.nodes?.length
  );
  check(
    syntaxLine({
      depth: 1,
      kind: "select",
      field: "body",
      range: { start: { line: 0, character: 0 }, end: { line: 1, character: 4 } },
    }) === "  body: select [1:0 - 2:4]",
    "vscode: a syntax-tree line has depth, field, kind and range"
  );

  // highlighting from the server: the model's spans, and their colours
  const spans = (await exec("dbt.highlight", {
    uri: document.uri.toString(),
  })) as { spans?: Array<{ capture: string }> } | null;
  check(
    spans?.spans?.some((s) => s.capture === "tag.delimiter"),
    "vscode: dbt.highlight colours the model's Jinja",
    spans?.spans?.length
  );
  check(
    captureColor("keyword.conditional") === "symbolIcon.keywordForeground" &&
      captureColor("variable.parameter") === "symbolIcon.variableForeground" &&
      captureColor("spell") === undefined,
    "vscode: a capture takes its longest prefix's colour"
  );
  const grouped = spansByColor([
    { capture: "keyword", range: { start: { line: 0, character: 0 }, end: { line: 0, character: 6 } } },
    { capture: "keyword.operator", range: { start: { line: 0, character: 7 }, end: { line: 0, character: 9 } } },
    { capture: "spell", range: { start: { line: 1, character: 0 }, end: { line: 1, character: 1 } } },
  ]);
  check(
    grouped.size === 1 && grouped.get("symbolIcon.keywordForeground")?.length === 2,
    "vscode: spans are grouped by colour, colourless ones dropped"
  );

  // highlighting v2 and node info (specs/highlighting)
  check(
    captureColor("type.dbt.model") === "symbolIcon.classForeground" &&
      captureColor("type.dbt.reference.unresolved") === "editorWarning.foreground" &&
      captureColor("property.dbt.column") === "symbolIcon.fieldForeground",
    "vscode: dbt captures and unresolved references have colours"
  );
  check(
    languageOf("/p/models/schema.yml") === "yaml" &&
      languageOf("/p/target/compiled/p/a.sql") === "sql" &&
      languageOf("/p/models/a.sql") === "dbt",
    "vscode: the language of a file"
  );
  check(
    JSON.stringify(skippedLayers("yaml", "auto")) === JSON.stringify(["yaml"]) &&
      skippedLayers("yaml", "always").length === 0 &&
      skippedLayers("dbt", "auto").length === 0,
    "vscode: auto leaves YAML's own colours, always paints everything"
  );
  const layered = spansByColor(
    [
      { capture: "property", layer: "yaml", range: { start: { line: 0, character: 0 }, end: { line: 0, character: 4 } } },
      { capture: "type.dbt.model", layer: "dbt", range: { start: { line: 0, character: 6 }, end: { line: 0, character: 9 } } },
    ],
    ["yaml"]
  );
  check(layered.size === 1 && layered.has("symbolIcon.classForeground"), "vscode: a skipped layer is not drawn");
  check(
    JSON.stringify(parseNode("macro:times_five")) === JSON.stringify({ kind: "macro", name: "times_five" }) &&
      parseNode(" orders ").kind === undefined,
    "vscode: a node is [kind:]name"
  );
  const markdown = nodeMarkdown({
    kind: "macro",
    name: "cents",
    params: [{ name: "col" }],
    callers: [{ name: "orders", kind: "model" }],
    calls: [],
  });
  check(
    markdown.includes("`cents(col)`") && markdown.includes("## called by (1)") && markdown.includes("- orders _model_"),
    "vscode: a macro's card as markdown",
    markdown
  );
  const macroInfo = (await exec("dbt.nodeInfo", {
    node: { kind: "macro", name: "times_five" },
  })) as { kind?: string } | null;
  check(macroInfo?.kind === "macro", "vscode: dbt.nodeInfo answers for a macro", macroInfo);

  // ast-grep: the query, a match's label, and the command over the wire
  check(
    JSON.stringify(parseAstGrep("{{ ref($M) }} => {{ ref('x', $M) }}")) ===
      JSON.stringify({ pattern: "{{ ref($M) }}", rewrite: "{{ ref('x', $M) }}", language: "dbt" }) &&
      parseAstGrep("sql: select $$$C from $T")?.language === "sql" &&
      parseAstGrep("  ") === undefined,
    "vscode: an ast-grep query reads [sql:]pattern [=> rewrite]"
  );
  check(
    JSON.stringify(
      astGrepLabel(
        {
          uri: "file:///p/models/a.sql",
          range: { start: { line: 2, character: 5 }, end: { line: 2, character: 19 } },
          text: "{{ ref('a') }}",
          replacement: "{{ ref('x', 'a') }}",
        },
        () => "models/a.sql"
      )
    ) === JSON.stringify({ label: "{{ ref('a') }}  →  {{ ref('x', 'a') }}", description: "models/a.sql:3" }),
    "vscode: a match's quick-pick entry shows its rewrite and place"
  );
  const found = (await exec("dbt.astGrep", {
    uri: document.uri.toString(),
    pattern: "{{ ref($M) }}",
  })) as { matches?: unknown[] } | null;
  check(found?.matches?.length === 4, "vscode: dbt.astGrep finds the model's refs", found?.matches?.length);

  check(
    JSON.stringify(
      unresolvedLabel(
        {
          kind: "ref",
          name: "ghost",
          uri: "file:///p/models/a.sql",
          range: { start: { line: 2, character: 14 }, end: { line: 2, character: 19 } },
          message: "model 'ghost' not found in the project",
        },
        () => "models/a.sql"
      )
    ) ===
      JSON.stringify({
        label: "[ref] ghost",
        description: "models/a.sql:3",
        detail: "model 'ghost' not found in the project",
      }),
    "vscode: an unresolved name's quick-pick entry shows its kind and place"
  );
  const unresolvedResult = (await exec("dbt.unresolved", {
    uri: document.uri.toString(),
  })) as { count?: number; items?: unknown[] } | null;
  check(
    typeof unresolvedResult?.count === "number" && Array.isArray(unresolvedResult?.items),
    "vscode: dbt.unresolved lists the project's unresolved names",
    unresolvedResult
  );

  check(
    JSON.stringify(
      breakingLines({
        summary: "1 breaking change(s): customer_id removed",
        diff: { removed: [{ name: "customer_id", possiblyRenamedTo: "customer_key" }] },
        impacts: [
          {
            column: "customer_id",
            change: "removed",
            severity: "error",
            tests: ["unique"],
            downstream: [{ model: "customers", column: "customer_id", contracted: true }],
          },
        ],
      })
    ) ===
      JSON.stringify([
        "[error] customer_id removed (possibly renamed to customer_key) — tested: unique",
        "  read by customers.customer_id (contracted)",
      ]),
    "vscode: a breaking change and its downstream reads are lines"
  );
  const breakingResult = (await exec("dbt.breakingChanges", {
    uri: document.uri.toString(),
  })) as { status?: string } | null;
  check(
    typeof breakingResult?.status === "string",
    "vscode: dbt.breakingChanges answers a status",
    breakingResult
  );

  check(
    JSON.stringify(
      selectLabel(
        {
          kind: "model",
          name: "customers",
          uri: "file:///p/models/customers.sql",
          uniqueId: "model.p.customers",
          range: { start: { line: 0, character: 0 }, end: { line: 0, character: 0 } },
        },
        () => "models/customers.sql"
      )
    ) ===
      JSON.stringify({
        label: "[model] customers",
        description: "models/customers.sql:1",
        detail: "model.p.customers",
      }),
    "vscode: a selected node's quick-pick entry shows its kind and place"
  );
  const selectResult = (await exec("dbt.select", {
    uri: document.uri.toString(),
    selector: "tag:nightly",
  })) as { count?: number; nodes?: Array<{ name: string }> } | null;
  check(
    typeof selectResult?.count === "number" && selectResult?.nodes?.[0]?.name === "customers",
    "vscode: dbt.select picks the nightly model",
    selectResult
  );

  const plan = {
    changes: 2,
    applied: false,
    files: [{ uri: "file:///p/models/a.yml", created: false, diff: "--- a/a.yml\n+++ b/a.yml\n+      - name: id\n" }],
    models: [
      {
        model: "a",
        added: ["id"],
        inherited: [{ column: "id", from: { kind: "source", source: "raw", table: "t", column: "id" } }],
        stale: ["old"],
        skipped: [{ column: "x", reason: "written in flow style" }],
      },
      { model: "quiet" },
    ],
  };
  check(
    JSON.stringify(syncYamlLines(plan)) ===
      JSON.stringify([
        "# dbt.syncYaml: 2 change(s) in 1 file(s)",
        "# a",
        "  + column id",
        "  ~ id description from source raw.t.id",
        "  ! stale column old (prune removes it)",
        "  ? x: written in flow style",
        "",
        "--- a/a.yml",
        "+++ b/a.yml",
        "+      - name: id",
      ]),
    "vscode: the sync preview lists each model's changes, then the diffs",
    syncYamlLines(plan)
  );
  check(
    syncYamlSummary(plan).startsWith("dbt: 2 change(s) in 1 file(s) — 0 entries, 1 columns"),
    "vscode: the sync summary counts the plan",
    syncYamlSummary(plan)
  );
  const syncResult = (await exec("dbt.syncYaml", {
    uri: "",
    models: ["stg_customers"],
  })) as { applied?: boolean; models?: Array<{ model: string; added: string[] }> } | null;
  check(
    syncResult?.applied === false &&
      syncResult?.models?.[0]?.model === "stg_customers" &&
      syncResult?.models?.[0]?.added?.[0] === "first_name",
    "vscode: dbt.syncYaml plans without applying",
    syncResult
  );

  // the palette commands registered
  const commands = await vscode.commands.getCommands(true);
  check(
    [
      "cogitator.run",
      "cogitator.status",
      "cogitator.restart",
      "cogitator.splitPlan",
      "cogitator.incrementalDiff",
      "cogitator.previewCte",
      "cogitator.incrementality",
      "cogitator.syncYaml",
      "cogitator.syntaxTree",
      "cogitator.highlight",
      "cogitator.astGrep",
      "cogitator.nodeInfo",
      "cogitator.select",
      "cogitator.unresolved",
      "cogitator.breakingChanges",
    ].every((c) =>
      commands.includes(c)
    ),
    "vscode: palette commands registered"
  );

  console.log(failures === 0 ? "ALL PASSED" : `${failures} failure(s)`);
  if (failures > 0) {
    throw new Error(`${failures} failure(s)`);
  }
}
