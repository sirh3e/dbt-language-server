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

export async function activate(
  context: vscode.ExtensionContext
): Promise<CogitatorApi> {
  const palette: Array<[string, () => Thenable<unknown> | void]> = [
    ["cogitator.run", () => execDbt("dbt.run")],
    ["cogitator.test", () => execDbt("dbt.test")],
    ["cogitator.compile", () => execDbt("dbt.compile")],
    ["cogitator.preview", () => execDbt("dbt.preview")],
    ["cogitator.parse", () => execDbt("dbt.parse")],
    ["cogitator.lineage", () => execDbt("dbt.showLineage")],
    ["cogitator.splitPlan", splitPlan],
    ["cogitator.incrementality", incrementality],
    ["cogitator.syntaxTree", syntaxTree],
    ["cogitator.highlight", highlight],
    ["cogitator.astGrep", astGrep],
    ["cogitator.nodeInfo", nodeInfo],
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
