// Thin VS Code client for the cogitator dbt language server: spawn the
// binary over stdio, route SQL + YAML documents, and expose the dbt.*
// executeCommand surface as palette commands. All intelligence lives in
// the server (specs/neovim-plugin/00-plugin.md §1.6 documents the
// command contract: every dbt.* command takes ONE argument object).

import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
} from "vscode-languageclient/node";
import { LineageAnswer, showLineageView } from "./lineageView";
export { columnPosition, lineageFocus, lineageHtml } from "./lineageView";

let client: LanguageClient | undefined;

/** What activate() returns: other extensions (and the extension-host
 *  test suite) reach the live LanguageClient through it. */
export interface CogitatorApi {
  client(): LanguageClient | undefined;
}

function serverOptions(): ServerOptions {
  const config = vscode.workspace.getConfiguration("cogitator");
  const command = config.get<string>("serverPath", "cogitator");
  const args = config.get<string[]>("serverArguments", []);
  return { command, args };
}

async function startClient(context: vscode.ExtensionContext): Promise<void> {
  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: "file", language: "sql" },
      { scheme: "file", language: "jinja-sql" },
      { scheme: "file", language: "yaml" },
    ],
    synchronize: {
      fileEvents: vscode.workspace.createFileSystemWatcher(
        "**/{dbt_project.yml,cogitator.yml,.cogitator.yml,profiles.yml}"
      ),
    },
  };
  client = new LanguageClient(
    "cogitator",
    "cogitator (dbt)",
    serverOptions(),
    clientOptions
  );
  context.subscriptions.push(client);
  await client.start();
}

/// workspace/executeCommand for a document that need not be the active one.
async function execDbtFor(
  document: vscode.TextDocument,
  command: string,
  extra?: Record<string, unknown>
) {
  if (!client) {
    return;
  }
  return client.sendRequest("workspace/executeCommand", {
    command,
    arguments: [{ uri: document.uri.toString(), ...extra }],
  });
}

/// workspace/executeCommand with the one-argument-object contract.
async function execDbt(command: string, extra?: Record<string, unknown>) {
  const editor = vscode.window.activeTextEditor;
  if (!client || !editor) {
    void vscode.window.showWarningMessage(
      "cogitator: open a dbt model first."
    );
    return;
  }
  const argument: Record<string, unknown> = {
    uri: editor.document.uri.toString(),
    ...extra,
  };
  return client.sendRequest("workspace/executeCommand", {
    command,
    arguments: [argument],
  });
}

/** A dbt.splitPlan answer (specs/ir-pipeline/10 §6). */
export interface SplitPlan {
  model: string;
  sexp: string;
  part?: { name: string; cte: string; text: string } | null;
}

/** The text shown for a split plan: the S-expression, then the new model. */
export function splitText(plan: SplitPlan): string {
  let text = `${plan.sexp}\n`;
  if (plan.part) {
    text += `\n;; ${plan.part.name}.sql\n${plan.part.text.trimEnd()}\n`;
  }
  return text;
}

/** A dbt.incrementalDiff answer: the model rendered with `is_incremental()` false and true. */
export interface IncrementalDiff {
  model: string;
  fullRefresh: string;
  incremental: string;
  diff: string;
  identical: boolean;
}

/** The text shown for an incremental diff: the unified diff, or a note and the SQL. */
export function incrementalDiffText(result: IncrementalDiff): string {
  if (result.identical) {
    return `-- ${result.model}: is_incremental() changes nothing in the rendered SQL\n\n${result.fullRefresh}`;
  }
  return result.diff;
}

async function incrementalDiff(): Promise<void> {
  const result = (await execDbt("dbt.incrementalDiff")) as IncrementalDiff | null | undefined;
  if (!result) {
    return;
  }
  const doc = await vscode.workspace.openTextDocument({
    language: result.identical ? "sql" : "diff",
    content: incrementalDiffText(result),
  });
  await vscode.window.showTextDocument(doc, {
    preview: true,
    viewColumn: vscode.ViewColumn.Beside,
  });
}

/** A dbt.previewCte answer: the rows of one CTE (or the final select). */
export interface CtePreview {
  model: string;
  cte?: string | null;
  columns: string[];
  rows: unknown[][];
  truncated: boolean;
  sql: string;
  source: "compiled" | "rendered";
  uri?: string | null;
  table: string;
}

/** The text shown for a preview: the server's markdown table. */
export function previewCteText(result: CtePreview): string {
  return result.table;
}

/** The rows of the CTE under the cursor; the server opens its markdown
 *  file through window/showDocument, so a document is opened here only
 *  when it could not write one. */
async function previewCte(): Promise<void> {
  const editor = vscode.window.activeTextEditor;
  const position = editor
    ? { line: editor.selection.active.line, character: editor.selection.active.character }
    : undefined;
  const result = (await execDbt("dbt.previewCte", { position })) as CtePreview | null | undefined;
  if (!result || result.uri) {
    return;
  }
  const doc = await vscode.workspace.openTextDocument({
    language: "markdown",
    content: previewCteText(result),
  });
  await vscode.window.showTextDocument(doc, {
    preview: true,
    viewColumn: vscode.ViewColumn.Beside,
  });
}

/** One row of a dbt.incrementality report. */
export interface IncrementalityRow {
  model: string;
  uri: string;
  lastRunSeconds?: number | null;
  view: {
    verdict: string;
    strategy?: string | null;
    summary?: string | null;
    reasons?: Array<{ reason: string }>;
  };
}

/** A quick-pick label: the verdict, its strategy, the last run and why. */
export function incrementalityLabel(row: IncrementalityRow): string {
  const view = row.view;
  let label = `${row.model} — ${view.verdict}`;
  if (view.strategy) {
    label += ` (${view.strategy})`;
  }
  if (typeof row.lastRunSeconds === "number") {
    label += ` [last run ${row.lastRunSeconds.toFixed(1)}s]`;
  }
  const why =
    view.summary ??
    (view.reasons && view.reasons.length > 0
      ? view.reasons.map((r) => r.reason).join("; ")
      : undefined);
  return why ? `${label}: ${why}` : label;
}

/** The split code action on `uri`, applied through VS Code's providers. */
async function applySplit(uri: vscode.Uri): Promise<void> {
  const actions =
    (await vscode.commands.executeCommand<vscode.CodeAction[]>(
      "vscode.executeCodeActionProvider",
      uri,
      new vscode.Range(0, 0, 0, 0),
      vscode.CodeActionKind.RefactorExtract.value
    )) ?? [];
  const split = actions.find((a) => a.title.startsWith("Split '"));
  if (!split?.edit) {
    void vscode.window.showWarningMessage("cogitator: no split is offered for this model.");
    return;
  }
  await vscode.workspace.applyEdit(split.edit);
}

async function splitPlan(): Promise<void> {
  const source = vscode.window.activeTextEditor?.document.uri;
  const plan = (await execDbt("dbt.splitPlan")) as SplitPlan | null | undefined;
  if (!source || plan === undefined) {
    return;
  }
  if (plan === null) {
    void vscode.window.showInformationMessage(
      "cogitator: nothing to split — this model is not a table, or nothing keeps it one."
    );
    return;
  }
  const doc = await vscode.workspace.openTextDocument({
    language: "clojure",
    content: splitText(plan),
  });
  await vscode.window.showTextDocument(doc, {
    preview: true,
    viewColumn: vscode.ViewColumn.Beside,
  });
  if (plan.part) {
    const choice = await vscode.window.showInformationMessage(
      `Split '${plan.part.cte}' into the incremental model '${plan.part.name}'?`,
      "Apply split"
    );
    if (choice === "Apply split") {
      await applySplit(source);
    }
  }
}

async function incrementality(): Promise<void> {
  const report = (await execDbt("dbt.incrementality")) as
    | { models: IncrementalityRow[]; incremental: number }
    | undefined;
  if (!report) {
    return;
  }
  if (report.models.length === 0) {
    void vscode.window.showInformationMessage("cogitator: no table models in this project.");
    return;
  }
  const pick = await vscode.window.showQuickPick(
    report.models.map((row) => ({ label: incrementalityLabel(row), row })),
    {
      placeHolder: `${report.incremental} of ${report.models.length} table models could be incremental`,
    }
  );
  if (pick) {
    await vscode.window.showTextDocument(vscode.Uri.parse(pick.row.uri));
  }
}

/** One node of a dbt.syntaxTree answer. */
export interface SyntaxNode {
  depth: number;
  kind: string;
  field?: string | null;
  missing?: boolean;
  range: {
    start: { line: number; character: number };
    end: { line: number; character: number };
  };
}

/** One tree line, as :DbtSyntaxTree draws it: depth, field, kind, range. */
export function syntaxLine(node: SyntaxNode): string {
  const r = node.range;
  const kind = node.missing ? `MISSING ${node.kind}` : node.kind;
  return `${"  ".repeat(node.depth)}${node.field ? `${node.field}: ` : ""}${kind} [${
    r.start.line + 1
  }:${r.start.character} - ${r.end.line + 1}:${r.end.character}]`;
}

async function syntaxTree(): Promise<void> {
  const tree = (await execDbt("dbt.syntaxTree")) as
    | { nodes: SyntaxNode[]; truncated: boolean }
    | undefined;
  if (!tree) {
    return;
  }
  let content = tree.nodes.map(syntaxLine).join("\n") + "\n";
  if (tree.truncated) {
    content += "… more nodes than the server sends; the rest are left out\n";
  }
  const doc = await vscode.workspace.openTextDocument({ language: "plaintext", content });
  await vscode.window.showTextDocument(doc, {
    preview: true,
    viewColumn: vscode.ViewColumn.Beside,
  });
}

/** One span of a dbt.highlight answer. */
export interface HighlightSpan {
  range: {
    start: { line: number; character: number };
    end: { line: number; character: number };
  };
  capture: string;
  /** yaml, sql, jinja or dbt (dbt.highlight v2) */
  layer?: string;
}

/** The theme colour of a tree-sitter capture, by its longest known
 *  prefix (`keyword.conditional` → keyword's); undefined for none. */
export function captureColor(capture: string): string | undefined {
  // a reference the project lacks (type.dbt.reference.unresolved)
  if (capture.endsWith(".unresolved")) {
    return "editorWarning.foreground";
  }
  const colors: Record<string, string> = {
    module: "symbolIcon.moduleForeground",
    label: "symbolIcon.constantForeground",
    attribute: "symbolIcon.keywordForeground",
    "property.dbt.column": "symbolIcon.fieldForeground",
    "function.dbt.test": "symbolIcon.methodForeground",
    "string.special": "symbolIcon.stringForeground",
    comment: "editorLineNumber.foreground",
    string: "debugTokenExpression.string",
    number: "debugTokenExpression.number",
    boolean: "debugTokenExpression.boolean",
    constant: "symbolIcon.constantForeground",
    keyword: "symbolIcon.keywordForeground",
    operator: "symbolIcon.operatorForeground",
    function: "symbolIcon.functionForeground",
    type: "symbolIcon.classForeground",
    variable: "symbolIcon.variableForeground",
    field: "symbolIcon.fieldForeground",
    property: "symbolIcon.propertyForeground",
    tag: "symbolIcon.keywordForeground",
  };
  let best: string | undefined;
  let bestLength = -1;
  for (const prefix of Object.keys(colors)) {
    if ((capture === prefix || capture.startsWith(`${prefix}.`)) && prefix.length > bestLength) {
      best = colors[prefix];
      bestLength = prefix.length;
    }
  }
  return best;
}

/** dbt.highlight's spans grouped by the colour they take, the layers in
 *  `skip` left out. */
export function spansByColor(
  spans: HighlightSpan[],
  skip: string[] = []
): Map<string, HighlightSpan["range"][]> {
  const out = new Map<string, HighlightSpan["range"][]>();
  for (const span of spans) {
    if (skip.includes(span.layer ?? "jinja")) {
      continue;
    }
    const color = captureColor(span.capture);
    if (!color) {
      continue;
    }
    const list = out.get(color) ?? [];
    list.push(span.range);
    out.set(color, list);
  }
  return out;
}

const decorations = new Map<string, vscode.TextEditorDecorationType>();

/** The grammar a file is coloured with: yaml, sql (compiled) or dbt. */
export function languageOf(path: string): "dbt" | "sql" | "yaml" {
  if (/\.ya?ml$/.test(path)) {
    return "yaml";
  }
  return /[/\\]target[/\\](compiled|run)[/\\]/.test(path) ? "sql" : "dbt";
}

/** The layers left to VS Code under a mode: in `auto` its own YAML
 *  grammar keeps YAML's colours; `always` paints every layer. */
export function skippedLayers(language: string, mode: string): string[] {
  return mode === "auto" && language === "yaml" ? ["yaml"] : [];
}

/** Paint `editor` with the server's answer for its text; an answer for an
 *  older version of the document is dropped. */
async function paint(editor: vscode.TextEditor, mode: string): Promise<void> {
  const document = editor.document;
  const language = languageOf(document.uri.fsPath);
  const version = document.version;
  const result = (await execDbtFor(document, "dbt.highlight", {
    text: document.getText(),
    language,
    version,
  })) as { spans: HighlightSpan[] } | undefined;
  if (!result || document.version !== version) {
    return;
  }
  for (const type of decorations.values()) {
    editor.setDecorations(type, []);
  }
  for (const [color, ranges] of spansByColor(result.spans, skippedLayers(language, mode))) {
    let type = decorations.get(color);
    if (!type) {
      type = vscode.window.createTextEditorDecorationType({ color: new vscode.ThemeColor(color) });
      decorations.set(color, type);
    }
    editor.setDecorations(
      type,
      ranges.map(
        (r) => new vscode.Range(r.start.line, r.start.character, r.end.line, r.end.character)
      )
    );
  }
}

/** Colour the active editor as the server highlights it: the Jinja, the
 *  SQL between the tags, YAML with dbt's captures; compiled SQL as SQL. */
async function highlight(): Promise<void> {
  const editor = vscode.window.activeTextEditor;
  if (editor) {
    await paint(editor, "always");
  }
}

/** `cogitator.highlight.onOpen`: off, auto or always. */
function onOpenMode(): string {
  return vscode.workspace.getConfiguration("cogitator").get<string>("highlight.onOpen", "auto");
}

/** Is the document one of the project's SQL or YAML files? */
function isDbtDocument(document: vscode.TextDocument): boolean {
  return (
    document.uri.scheme === "file" &&
    ["sql", "jinja-sql", "yaml"].includes(document.languageId) &&
    vscode.workspace.getWorkspaceFolder(document.uri) !== undefined
  );
}

/** Paint visible dbt editors on open and after edits (debounced). */
function watchHighlights(context: vscode.ExtensionContext): void {
  const timers = new Map<string, NodeJS.Timeout>();
  const paintVisible = (document?: vscode.TextDocument) => {
    const mode = onOpenMode();
    if (mode === "off") {
      return;
    }
    for (const editor of vscode.window.visibleTextEditors) {
      if ((!document || editor.document === document) && isDbtDocument(editor.document)) {
        void paint(editor, mode);
      }
    }
  };
  context.subscriptions.push(
    vscode.window.onDidChangeVisibleTextEditors(() => paintVisible()),
    vscode.workspace.onDidChangeTextDocument((event) => {
      const key = event.document.uri.toString();
      clearTimeout(timers.get(key));
      timers.set(
        key,
        setTimeout(() => paintVisible(event.document), 300)
      );
    })
  );
  paintVisible();
}

/** `[kind:]name` → its parts, kind undefined when not given. */
export function parseNode(text: string): { kind?: string; name: string } {
  const trimmed = text.trim();
  const match = /^([a-z_]+):(.+)$/.exec(trimmed);
  return match ? { kind: match[1], name: match[2].trim() } : { name: trimmed };
}

/** The kinds "dbt: Show node info" tries, in order, when none is given. */
export const NODE_KINDS = [
  "model",
  "macro",
  "test",
  "generic_test",
  "source",
  "exposure",
  "metric",
  "semantic_model",
  "analysis",
];

interface NodeEntry {
  name: string;
  kind?: string;
}

/** A dbt.nodeInfo card as markdown. */
export function nodeMarkdown(info: Record<string, unknown>): string {
  const kind = (info.kind as string) ?? "model";
  const name = (info.name as string) ?? (info.model as string) ?? "?";
  const lines = [`# ${name}`, "", `*${kind}*`];
  if (typeof info.description === "string" && info.description !== "") {
    lines.push("", info.description);
  }
  const signature = (params: unknown) =>
    `${name}(${((params as Array<string | { name: string; default?: string | null }>) ?? [])
      .map((p) => (typeof p === "string" ? p : p.default ? `${p.name}=${p.default}` : p.name))
      .join(", ")})`;
  const list = (title: string, entries: unknown) => {
    const items = (entries as NodeEntry[]) ?? [];
    lines.push("", `## ${title} (${items.length})`);
    for (const e of items) {
      lines.push(`- ${e.name}${e.kind ? ` _${e.kind}_` : ""}`);
    }
  };
  if (kind === "macro") {
    lines.push("", "`" + signature(info.params) + "`");
    list("called by", info.callers);
    list("calls", info.calls);
  } else if (kind === "generic_test") {
    lines.push("", "`{% test " + signature(info.params) + " %}`");
    const applied = (info.appliedTo as Array<{ model: string; column?: string | null }>) ?? [];
    lines.push("", `## applied to (${applied.length})`);
    for (const a of applied) {
      lines.push(`- ${a.model}${a.column ? "." + a.column : ""}`);
    }
  } else {
    list(kind === "test" ? "tests" : "upstream", info.refs ?? info.upstream);
    if (kind !== "test" && kind !== "analysis") {
      list("downstream", info.downstream);
    }
  }
  return lines.join("\n") + "\n";
}

/** "dbt: Show node info": a node's card (dbt.nodeInfo) as markdown. */
async function nodeInfo(): Promise<void> {
  const input = await vscode.window.showInputBox({
    prompt: "node: [kind:]name, e.g. macro:cents_to_dollars",
  });
  if (!input) {
    return;
  }
  const { kind, name } = parseNode(input);
  const kinds = kind ? [kind] : NODE_KINDS;
  for (const k of kinds) {
    const info = (await execDbt("dbt.nodeInfo", {
      node: { kind: k, name },
      quiet: kinds.length > 1,
    })) as Record<string, unknown> | null | undefined;
    if (info) {
      const doc = await vscode.workspace.openTextDocument({
        language: "markdown",
        content: nodeMarkdown(info),
      });
      await vscode.window.showTextDocument(doc, { preview: true, viewColumn: vscode.ViewColumn.Beside });
      return;
    }
  }
  void vscode.window.showInformationMessage(`cogitator: no node named ${name}`);
}

/** `[sql:]pattern [=> rewrite]` → its parts. */
export function parseAstGrep(
  text: string
): { pattern: string; rewrite?: string; language: "dbt" | "sql" } | undefined {
  let rest = text.trim();
  let language: "dbt" | "sql" = "dbt";
  const sql = /^sql:\s*/.exec(rest);
  if (sql) {
    language = "sql";
    rest = rest.slice(sql[0].length);
  } else {
    rest = rest.replace(/^dbt:\s*/, "");
  }
  const arrow = /\s*=>\s*/.exec(rest);
  const pattern = (arrow ? rest.slice(0, arrow.index) : rest).trim();
  if (!pattern) {
    return undefined;
  }
  const rewrite = arrow ? rest.slice(arrow.index + arrow[0].length).trim() : undefined;
  return rewrite ? { pattern, rewrite, language } : { pattern, language };
}

/** One dbt.astGrep match. */
export interface AstGrepMatch {
  uri: string;
  range: HighlightSpan["range"];
  text: string;
  replacement?: string | null;
}

/** A match as a quick-pick entry: the text, and where it is. */
export function astGrepLabel(match: AstGrepMatch, relative: (uri: string) => string): {
  label: string;
  description: string;
} {
  const text = match.text.replace(/\s*\n\s*/g, " ");
  return {
    label: match.replacement ? `${text}  →  ${match.replacement}` : text,
    description: `${relative(match.uri)}:${match.range.start.line + 1}`,
  };
}

/** Structural search with ast-grep over the project (dbt.astGrep). */
async function astGrep(): Promise<void> {
  const input = await vscode.window.showInputBox({
    prompt: "ast-grep: [sql:]pattern [=> rewrite] — $M is a metavariable",
    value: "{{ ref($M) }}",
  });
  const query = input ? parseAstGrep(input) : undefined;
  if (!query) {
    return;
  }
  const result = (await execDbt("dbt.astGrep", { uri: "", ...query })) as
    | { matches: AstGrepMatch[]; truncated: boolean }
    | undefined;
  if (!result) {
    return;
  }
  if (result.matches.length === 0) {
    void vscode.window.showInformationMessage(`ast-grep: no match of ${query.pattern}`);
    return;
  }
  if (query.rewrite) {
    const answer = await vscode.window.showWarningMessage(
      `Rewrite ${result.matches.length} matches into ${query.rewrite}?`,
      { modal: true },
      "Rewrite"
    );
    if (answer === "Rewrite") {
      await execDbt("dbt.astGrep", { uri: "", ...query, apply: true });
    }
    return;
  }
  const relative = (uri: string) => vscode.workspace.asRelativePath(vscode.Uri.parse(uri));
  const pick = await vscode.window.showQuickPick(
    result.matches.map((m) => ({ ...astGrepLabel(m, relative), match: m })),
    {
      matchOnDescription: true,
      placeHolder: `${result.matches.length} matches of ${query.pattern}${
        result.truncated ? " (the first 500)" : ""
      }`,
    }
  );
  if (pick) {
    const r = pick.match.range;
    await vscode.window.showTextDocument(vscode.Uri.parse(pick.match.uri), {
      selection: new vscode.Range(r.start.line, r.start.character, r.end.line, r.end.character),
    });
  }
}

/** One node of a dbt.select answer. */
export interface SelectedNode {
  uniqueId: string;
  resourceType: string;
  name: string;
  version?: string | null;
  tags: string[];
  uri?: string | null;
  range?: HighlightSpan["range"] | null;
  graphId?: string | null;
}

/** What dbt.select answers. */
export interface SelectionAnswer {
  nodes: SelectedNode[];
  counts: Record<string, number>;
  error?: { message: string; start: number; end: number } | null;
}

/** A selected node as a quick-pick entry: dbt's name, its kind and tags. */
export function selectedLabel(node: SelectedNode): { label: string; description: string } {
  let name = node.name;
  if (node.version) {
    name = `${node.name}.v${node.version}`;
  } else if (node.resourceType === "source" && node.graphId) {
    name = `${node.graphId.replace(/^source:/, "")}.${node.name}`;
  }
  const tags = node.tags.length > 0 ? `  #${node.tags.join(" #")}` : "";
  return { label: name, description: `[${node.resourceType.replace(/_/g, " ")}]${tags}` };
}

const KIND_ORDER = [
  "model", "seed", "snapshot", "source", "test", "unit_test",
  "analysis", "exposure", "metric", "semantic_model",
];

/** "7 · 3 models, 2 tests, 2 seeds" from dbt.select's counts. */
export function selectionSummary(counts: Record<string, number>): string {
  let total = 0;
  const parts: string[] = [];
  for (const kind of KIND_ORDER) {
    const n = counts[kind] ?? 0;
    if (n > 0) {
      total += n;
      const word = kind.replace(/_/g, " ");
      parts.push(`${n} ${word}${n === 1 ? "" : word.endsWith("s") ? "es" : "s"}`);
    }
  }
  return total === 0 ? "0" : `${total} · ${parts.join(", ")}`;
}

/**
 * dbt node selection (dbt.select): dbt's --select syntax, evaluated as
 * typed, as `dbt ls --select … --exclude …` answers it; choosing a node
 * opens it at its place.
 */
async function select(): Promise<void> {
  const pick = vscode.window.createQuickPick<vscode.QuickPickItem & { node: SelectedNode }>();
  pick.placeholder = "tag:nightly  +dim_items  @m  a,b  stg_*  config.materialized:incremental  --exclude x";
  pick.title = "dbt select";
  pick.matchOnDescription = true;
  let asked = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const ask = (text: string) => {
    const mine = ++asked;
    if (text.trim() === "") {
      pick.items = [];
      pick.title = "dbt select";
      return;
    }
    pick.busy = true;
    void execDbt("dbt.select", { select: text }).then((result) => {
      if (mine !== asked) {
        return;
      }
      pick.busy = false;
      const answer = result as SelectionAnswer | undefined;
      if (!answer) {
        return;
      }
      if (answer.error) {
        pick.items = [];
        pick.title = `dbt select · ✗ ${answer.error.message}`;
        return;
      }
      // the server selected them: keep every one, whatever the box says
      pick.items = answer.nodes.map((node) => ({ ...selectedLabel(node), alwaysShow: true, node }));
      pick.title = `dbt select · ${selectionSummary(answer.counts)}`;
    }, (err: unknown) => {
      if (mine === asked) {
        pick.busy = false;
        pick.title = `dbt select · ✗ ${err instanceof Error ? err.message : String(err)}`;
      }
    });
  };
  pick.onDidChangeValue((text) => {
    if (timer) {
      clearTimeout(timer);
    }
    timer = setTimeout(() => ask(text), 150);
  });
  pick.onDidAccept(async () => {
    const node = pick.selectedItems[0]?.node;
    pick.hide();
    if (node?.uri) {
      const r = node.range;
      await vscode.window.showTextDocument(vscode.Uri.parse(node.uri), {
        selection: r ? new vscode.Range(r.start.line, r.start.character, r.end.line, r.end.character) : undefined,
      });
    }
  });
  pick.onDidHide(() => pick.dispose());
  pick.show();
}

/** One dbt.unresolved item. */
export interface UnresolvedItem {
  kind: string;
  name: string;
  uri: string;
  range: HighlightSpan["range"];
  message?: string;
}

/** An unresolved name as a quick-pick entry: what, and where. */
export function unresolvedLabel(item: UnresolvedItem, relative: (uri: string) => string): {
  label: string;
  description: string;
  detail?: string;
} {
  return {
    label: `[${item.kind}] ${item.name}`,
    description: `${relative(item.uri)}:${item.range.start.line + 1}`,
    detail: item.message,
  };
}

/** Every unresolved ref, source, doc, macro and var (dbt.unresolved). */
async function unresolved(): Promise<void> {
  const result = (await execDbt("dbt.unresolved", { uri: "" })) as
    | { items: UnresolvedItem[]; count: number; truncated: boolean }
    | undefined;
  if (!result) {
    return;
  }
  if (result.count === 0) {
    void vscode.window.showInformationMessage(
      "dbt: nothing unresolved — every ref, source, doc, macro and var resolves"
    );
    return;
  }
  const relative = (uri: string) => vscode.workspace.asRelativePath(vscode.Uri.parse(uri));
  const pick = await vscode.window.showQuickPick(
    result.items.map((u) => ({ ...unresolvedLabel(u, relative), item: u })),
    { matchOnDescription: true, placeHolder: `${result.count} unresolved names` }
  );
  if (pick) {
    const r = pick.item.range;
    await vscode.window.showTextDocument(vscode.Uri.parse(pick.item.uri), {
      selection: new vscode.Range(r.start.line, r.start.character, r.end.line, r.end.character),
    });
  }
}

/** One node of a dbt.modelDependencies answer, as far as the palette reads it. */
export interface DependencyNode {
  name: string;
  kind: string;
  uri?: string | null;
  line: number;
  depth: number;
  config: { materialized?: string | null; incrementalStrategy?: string | null };
}

/** A dependency as a quick-pick entry: what it is, how it is built, how far. */
export function dependencyLabel(node: DependencyNode): {
  label: string;
  description: string;
} {
  const built = [node.config.materialized, node.config.incrementalStrategy]
    .filter((s): s is string => !!s)
    .join(", ");
  return {
    label: `[${node.kind}] ${node.name}`,
    description: [built, node.depth > 1 ? `${node.depth} hops` : ""].filter((s) => s).join(" · "),
  };
}

/** This model and its neighbours (dbt.modelDependencies): pick one to open it. */
async function modelDependencies(): Promise<void> {
  const result = (await execDbt("dbt.modelDependencies")) as
    | {
        model: DependencyNode;
        parents: DependencyNode[];
        children: DependencyNode[];
        upstream: DependencyNode[];
        downstream: DependencyNode[];
      }
    | null
    | undefined;
  if (!result) {
    void vscode.window.showInformationMessage("dbt: no model here");
    return;
  }
  type Entry = vscode.QuickPickItem & { node?: DependencyNode };
  const entries: Entry[] = [];
  const section = (title: string, nodes: DependencyNode[]) => {
    entries.push({ label: title, kind: vscode.QuickPickItemKind.Separator });
    for (const node of nodes) {
      entries.push({ ...dependencyLabel(node), node });
    }
  };
  section("Uses directly", result.parents);
  section("Used directly by", result.children);
  section("Everything upstream", result.upstream);
  section("Everything downstream", result.downstream);
  const pick = await vscode.window.showQuickPick(entries, {
    matchOnDescription: true,
    placeHolder: `${result.model.name}: ${result.upstream.length} upstream, ${result.downstream.length} downstream`,
  });
  if (pick?.node?.uri) {
    const line = pick.node.line;
    await vscode.window.showTextDocument(vscode.Uri.parse(pick.node.uri), {
      selection: new vscode.Range(line, 0, line, 0),
    });
  }
}

/** A dbt.breakingChanges answer, as far as the palette reads it. */
export interface BreakingReport {
  summary: string;
  diff?: { removed?: { name: string; possiblyRenamedTo?: string }[] };
  impacts?: {
    column: string;
    change: string;
    severity: string;
    tests?: string[];
    downstream?: { model: string; column: string; contracted?: boolean; public?: boolean }[];
  }[];
}

/** The lines of a dbt.breakingChanges answer: one per removed or retyped
 * column, then one per downstream column that reads it. */
export function breakingLines(report: BreakingReport): string[] {
  const renamed = new Map(
    (report.diff?.removed ?? []).map((r) => [r.name, r.possiblyRenamedTo] as const)
  );
  const lines: string[] = [];
  for (const impact of report.impacts ?? []) {
    const to = renamed.get(impact.column);
    const tests = impact.tests ?? [];
    lines.push(
      `[${impact.severity}] ${impact.column} ${impact.change}` +
        (to ? ` (possibly renamed to ${to})` : "") +
        (tests.length > 0 ? ` — tested: ${tests.join(", ")}` : "")
    );
    for (const use of impact.downstream ?? []) {
      const tags = [use.contracted ? "contracted" : "", use.public ? "public" : ""].filter(
        (t) => t !== ""
      );
      lines.push(
        `  read by ${use.model}.${use.column}` + (tags.length > 0 ? ` (${tags.join(", ")})` : "")
      );
    }
  }
  return lines;
}

/** This model against its last built schema (dbt.breakingChanges). */
async function breakingChanges(): Promise<void> {
  const report = (await execDbt("dbt.breakingChanges")) as BreakingReport | undefined;
  if (!report) {
    return;
  }
  const lines = breakingLines(report);
  if (lines.length === 0) {
    void vscode.window.showInformationMessage(`dbt: ${report.summary}`);
    return;
  }
  const doc = await vscode.workspace.openTextDocument({
    language: "plaintext",
    content: [report.summary, "", ...lines].join("\n"),
  });
  await vscode.window.showTextDocument(doc, { preview: true });
}

/** dbt.lineageSite: the lineage view, fed by the server (lineageView.ts). */
function lineageView(context: vscode.ExtensionContext): Promise<void> {
  return showLineageView(context, async (uri, assets) => {
    if (!client) {
      return undefined;
    }
    return (await client.sendRequest("workspace/executeCommand", {
      command: "dbt.lineageSite",
      arguments: [{ uri: uri ?? "", assets }],
    })) as LineageAnswer | null;
  });
}

/** A column whose description dbt.syncYaml inherits. */
export interface SyncInherited {
  column: string;
  from: { kind: string; name?: string; source?: string; table?: string; column: string };
}

/** One model of a dbt.syncYaml plan. */
export interface SyncModel {
  model: string;
  createdEntry?: boolean;
  added?: string[];
  inherited?: SyncInherited[];
  typed?: Array<{ column: string; dataType: string }>;
  stale?: string[];
  pruned?: string[];
  reordered?: boolean;
  skipped?: Array<{ column?: string | null; reason: string }>;
}

/** A dbt.syncYaml answer. */
export interface SyncPlanResult {
  changes: number;
  applied: boolean;
  files: Array<{ uri: string; created: boolean; diff: string }>;
  models: SyncModel[];
}

function syncFrom(from: SyncInherited["from"]): string {
  return from.kind === "source"
    ? `source ${from.source}.${from.table}.${from.column}`
    : `${from.name}.${from.column}`;
}

/** The preview of a dbt.syncYaml plan: each model's changes, then the diffs. */
export function syncYamlLines(result: SyncPlanResult): string[] {
  const lines = [`# dbt.syncYaml: ${result.changes} change(s) in ${result.files.length} file(s)`];
  for (const model of result.models) {
    const pruned = new Set(model.pruned ?? []);
    const body = [
      ...(model.createdEntry ? ["  + entry"] : []),
      ...(model.added ?? []).map((c) => `  + column ${c}`),
      ...(model.inherited ?? []).map((i) => `  ~ ${i.column} description from ${syncFrom(i.from)}`),
      ...(model.typed ?? []).map((t) => `  ~ ${t.column} data_type ${t.dataType}`),
      ...(model.pruned ?? []).map((c) => `  - column ${c}`),
      ...(model.reordered ? ["  ~ columns reordered"] : []),
      ...(model.stale ?? [])
        .filter((c) => !pruned.has(c))
        .map((c) => `  ! stale column ${c} (prune removes it)`),
      ...(model.skipped ?? []).map((s) => `  ? ${s.column ? `${s.column}: ` : ""}${s.reason}`),
    ];
    if (body.length > 0) {
      lines.push(`# ${model.model}`, ...body);
    }
  }
  for (const file of result.files) {
    lines.push("", ...file.diff.replace(/\n$/, "").split("\n"));
  }
  return lines;
}

/** One line on a dbt.syncYaml plan, for the confirmation. */
export function syncYamlSummary(result: SyncPlanResult): string {
  const sum = (f: (m: SyncModel) => number) => result.models.reduce((n, m) => n + f(m), 0);
  return (
    `dbt: ${result.changes} change(s) in ${result.files.length} file(s) — ` +
    `${sum((m) => (m.createdEntry ? 1 : 0))} entries, ${sum((m) => m.added?.length ?? 0)} columns, ` +
    `${sum((m) => m.inherited?.length ?? 0)} descriptions inherited, ` +
    `${sum((m) => m.stale?.length ?? 0)} stale, ${sum((m) => m.skipped?.length ?? 0)} skipped`
  );
}

/** Bring the schema YAML in line with the SQL (dbt.syncYaml): plan, show, apply. */
async function syncYaml(): Promise<void> {
  const scope = await vscode.window.showQuickPick(
    [
      { label: "Whole project", uri: "" },
      { label: "This model", uri: vscode.window.activeTextEditor?.document.uri.toString() ?? "" },
    ],
    { placeHolder: "Sync the schema YAML of…" }
  );
  if (!scope) {
    return;
  }
  const flags = await vscode.window.showQuickPick(
    [
      { label: "prune", description: "remove columns the models no longer produce", key: "prune" },
      { label: "inherit types", description: "fill data_type from proven types", key: "inheritTypes" },
      { label: "reorder", description: "order columns as the SQL does", key: "reorder" },
    ],
    { canPickMany: true, placeHolder: "Options (none by default)" }
  );
  if (!flags) {
    return;
  }
  const argument: Record<string, unknown> = { uri: scope.uri };
  for (const flag of flags) {
    argument[flag.key] = true;
  }
  const result = (await execDbt("dbt.syncYaml", argument)) as SyncPlanResult | undefined;
  if (!result) {
    return;
  }
  if (result.changes === 0) {
    void vscode.window.showInformationMessage("dbt: schema YAML is in sync — nothing to change");
    return;
  }
  let choice = await vscode.window.showInformationMessage(syncYamlSummary(result), "Apply", "Show plan");
  if (choice === "Show plan") {
    const doc = await vscode.workspace.openTextDocument({
      language: "diff",
      content: syncYamlLines(result).join("\n"),
    });
    await vscode.window.showTextDocument(doc, { preview: true });
    choice = await vscode.window.showInformationMessage(syncYamlSummary(result), "Apply");
  }
  if (choice === "Apply") {
    await execDbt("dbt.syncYaml", { ...argument, apply: true });
  }
}

/** A dbt.renameColumn answer, as far as the palette reads it. */
export interface ColumnRenameResult {
  model: string;
  column: string;
  newName: string;
  direction: string;
  origin: { model: string; column: string; chain: string[]; stop: string };
  models: string[];
  edit: {
    documentChanges: Array<{
      textDocument: { uri: string };
      edits: Array<{ range: { start: { line: number; character: number } }; newText: string }>;
    }>;
  };
}

/** One line on a dbt.renameColumn answer, for the confirmation. */
export function renameColumnSummary(result: ColumnRenameResult): string {
  const changes = result.edit.documentChanges ?? [];
  const edits = changes.reduce((n, c) => n + (c.edits?.length ?? 0), 0);
  return `dbt: rename ${result.origin.model}.${result.origin.column} → ${result.newName}: ${edits} edit(s) in ${changes.length} file(s), models ${result.models.join(", ")}`;
}

/** The preview of a dbt.renameColumn answer: where it starts, then each edit. */
export function renameColumnLines(result: ColumnRenameResult): string[] {
  const lines = [
    `# dbt.renameColumn: ${result.model}.${result.column} → ${result.newName} (${result.direction})`,
    `# from ${result.origin.model}.${result.origin.column}: ${result.origin.stop}`,
  ];
  for (const change of result.edit.documentChanges ?? []) {
    lines.push(`${change.textDocument.uri}: ${change.edits.length} edit(s)`);
    for (const edit of change.edits) {
      const at = edit.range.start;
      lines.push(`  ${at.line + 1}:${at.character + 1} ${edit.newText}`);
    }
  }
  return lines;
}

/** Rename the model column at the cursor up- and downstream (dbt.renameColumn). */
async function renameColumn(): Promise<void> {
  const editor = vscode.window.activeTextEditor;
  if (!editor) {
    void vscode.window.showWarningMessage("cogitator: open a dbt model first.");
    return;
  }
  const at = editor.selection.active;
  const word = editor.document.getText(editor.document.getWordRangeAtPosition(at));
  const newName = await vscode.window.showInputBox({
    prompt: `Rename column ${word} to`,
    value: word,
  });
  if (!newName || newName === word) {
    return;
  }
  const direction = await vscode.window.showQuickPick(
    [
      { label: "Up- and downstream", description: "from the model that defines it", key: "both" },
      { label: "Downstream only", description: "this model and its consumers", key: "down" },
    ],
    { placeHolder: `Rename ${word} → ${newName}…` }
  );
  if (!direction) {
    return;
  }
  const argument = {
    position: { line: at.line, character: at.character },
    newName,
    direction: direction.key,
  };
  let result: ColumnRenameResult | undefined | null;
  try {
    result = (await execDbt("dbt.renameColumn", argument)) as ColumnRenameResult | undefined | null;
  } catch (err) {
    void vscode.window.showErrorMessage(`dbt: ${(err as Error).message}`);
    return;
  }
  if (!result) {
    void vscode.window.showWarningMessage("dbt: no model column at the cursor");
    return;
  }
  let choice = await vscode.window.showInformationMessage(renameColumnSummary(result), "Apply", "Show edits");
  if (choice === "Show edits") {
    const doc = await vscode.workspace.openTextDocument({
      language: "markdown",
      content: renameColumnLines(result).join("\n"),
    });
    await vscode.window.showTextDocument(doc, { preview: true });
    choice = await vscode.window.showInformationMessage(renameColumnSummary(result), "Apply");
  }
  if (choice === "Apply") {
    await execDbt("dbt.renameColumn", { ...argument, apply: true });
  }
}

export async function activate(
  context: vscode.ExtensionContext
): Promise<CogitatorApi> {
  const palette: Array<[string, () => Thenable<unknown> | void]> = [
    ["cogitator.run", () => execDbt("dbt.run")],
    ["cogitator.test", () => execDbt("dbt.test")],
    ["cogitator.compile", () => execDbt("dbt.compile")],
    ["cogitator.preview", () => execDbt("dbt.preview")],
    ["cogitator.previewCte", previewCte],
    ["cogitator.parse", () => execDbt("dbt.parse")],
    ["cogitator.lineage", () => execDbt("dbt.showLineage")],
    ["cogitator.splitPlan", splitPlan],
    ["cogitator.incrementalDiff", incrementalDiff],
    ["cogitator.incrementality", incrementality],
    ["cogitator.syntaxTree", syntaxTree],
    ["cogitator.highlight", highlight],
    ["cogitator.astGrep", astGrep],
    ["cogitator.nodeInfo", nodeInfo],
    ["cogitator.lineageView", () => lineageView(context)],
    ["cogitator.syncYaml", syncYaml],
    ["cogitator.unresolved", unresolved],
    ["cogitator.select", select],
    ["cogitator.breakingChanges", breakingChanges],
    ["cogitator.modelDependencies", modelDependencies],
    ["cogitator.renameColumn", renameColumn],
    [
      "cogitator.status",
      async () => {
        const status = await execDbt("dbt.status");
        if (status !== undefined) {
          const doc = await vscode.workspace.openTextDocument({
            language: "json",
            content: JSON.stringify(status, null, 2),
          });
          await vscode.window.showTextDocument(doc, { preview: true });
        }
      },
    ],
    [
      "cogitator.restart",
      async () => {
        if (client) {
          await client.restart();
        }
      },
    ],
  ];
  for (const [name, handler] of palette) {
    context.subscriptions.push(
      vscode.commands.registerCommand(name, handler)
    );
  }
  await startClient(context);
  watchHighlights(context);
  return { client: () => client };
}

export async function deactivate(): Promise<void> {
  if (client) {
    await client.stop();
    client = undefined;
  }
}
