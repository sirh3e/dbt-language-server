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
  return { client: () => client };
}

export async function deactivate(): Promise<void> {
  if (client) {
    await client.stop();
    client = undefined;
  }
}
