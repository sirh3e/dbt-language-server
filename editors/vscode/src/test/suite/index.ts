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

  // the palette commands registered
  const commands = await vscode.commands.getCommands(true);
  check(
    ["cogitator.run", "cogitator.status", "cogitator.restart"].every((c) =>
      commands.includes(c)
    ),
    "vscode: palette commands registered"
  );

  console.log(failures === 0 ? "ALL PASSED" : `${failures} failure(s)`);
  if (failures > 0) {
    throw new Error(`${failures} failure(s)`);
  }
}
