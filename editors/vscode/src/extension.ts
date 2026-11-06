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

export async function activate(
  context: vscode.ExtensionContext
): Promise<void> {
  const palette: Array<[string, () => Thenable<unknown> | void]> = [
    ["cogitator.run", () => execDbt("dbt.run")],
    ["cogitator.test", () => execDbt("dbt.test")],
    ["cogitator.compile", () => execDbt("dbt.compile")],
    ["cogitator.preview", () => execDbt("dbt.preview")],
    ["cogitator.parse", () => execDbt("dbt.parse")],
    ["cogitator.lineage", () => execDbt("dbt.showLineage")],
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
}

export async function deactivate(): Promise<void> {
  if (client) {
    await client.stop();
    client = undefined;
  }
}
