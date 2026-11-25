// The lineage view: a webview panel that draws the same page as
// `cogitator lineage-site`. Its script, stylesheet and data all come from
// the server (`dbt.lineageSite`), so the extension holds no copy of the
// page; it only follows the active editor, refreshes on save and opens
// what the page asks for.

import * as vscode from "vscode";

/** The page's assets as `dbt.lineageSite {assets: true}` answers them. */
export interface LineageAssets {
  script: string;
  style: string;
}

/** A `dbt.lineageSite` answer. `site` is the page's data, opaque here. */
export interface LineageAnswer {
  site?: { nodes: Array<{ id: string; uri?: string | null }> } & Record<string, unknown>;
  assets?: LineageAssets;
  counts?: Record<string, number>;
}

/** Asks the server: `assets` for the first answer only. */
export type LineageFetch = (
  uri: string | undefined,
  assets: boolean
) => Promise<LineageAnswer | null | undefined>;

/** The site node id of a model file (`model:<file name>`), if it is SQL. */
export function lineageFocus(uri: string | undefined): string | undefined {
  if (!uri) {
    return undefined;
  }
  const name = decodeURIComponent(uri.split("/").pop() ?? "");
  return name.endsWith(".sql") ? `model:${name.slice(0, -4)}` : undefined;
}

/** The webview's HTML: nothing remote, every script and style carries the
 *  nonce, so the Content-Security-Policy allows them and nothing else. */
export function lineageHtml(nonce: string, cspSource: string, assets: LineageAssets): string {
  const csp = [
    "default-src 'none'",
    `style-src ${cspSource} 'nonce-${nonce}'`,
    `script-src 'nonce-${nonce}'`,
    "img-src data:",
  ].join("; ");
  // the assets are the server's own files; keep them inside their elements
  const script = assets.script.replace(/<\/script/gi, "<\\/script");
  const style = assets.style.replace(/<\/style/gi, "<\\/style");
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="${csp}">
<meta name="viewport" content="width=device-width, initial-scale=1">
<style nonce="${nonce}">${style}</style>
</head>
<body>
<div id="app"></div>
<script nonce="${nonce}">${script}</script>
</body>
</html>`;
}

/** Where a column name first appears as a word in `text` (0-based). */
export function columnPosition(
  text: string,
  column: string | undefined
): { line: number; character: number } | undefined {
  if (!column) {
    return undefined;
  }
  const name = column.split(".").pop() ?? column;
  const escaped = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = new RegExp(`\\b${escaped}\\b`, "i").exec(text);
  if (!match) {
    return undefined;
  }
  const before = text.slice(0, match.index).split("\n");
  return { line: before.length - 1, character: before[before.length - 1].length };
}

function nonce(): string {
  const chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
  let out = "";
  for (let i = 0; i < 32; i++) {
    out += chars.charAt(Math.floor(Math.random() * chars.length));
  }
  return out;
}

let current: vscode.WebviewPanel | undefined;

/** Opens the view, or brings it forward and focuses the active model. */
export async function showLineageView(
  context: vscode.ExtensionContext,
  fetch: LineageFetch
): Promise<void> {
  const active = () => vscode.window.activeTextEditor?.document.uri.toString();
  if (current) {
    current.reveal(vscode.ViewColumn.Beside, true);
    void current.webview.postMessage({ type: "focus", node: lineageFocus(active()) });
    return;
  }
  const first = await fetch(active(), true);
  if (!first || !first.site || !first.assets) {
    void vscode.window.showWarningMessage(
      "cogitator: no dbt project is loaded, so there is no lineage to show."
    );
    return;
  }
  const panel = vscode.window.createWebviewPanel(
    "cogitator.lineage",
    "dbt lineage",
    { viewColumn: vscode.ViewColumn.Beside, preserveFocus: true },
    { enableScripts: true, retainContextWhenHidden: true, localResourceRoots: [] }
  );
  current = panel;
  panel.webview.html = lineageHtml(nonce(), panel.webview.cspSource, first.assets);
  const send = (site: LineageAnswer["site"]) =>
    panel.webview.postMessage({ type: "site", site, focus: lineageFocus(active()) });
  let ready = false;
  const disposables: vscode.Disposable[] = [
    panel.webview.onDidReceiveMessage(async (msg: { type?: string; uri?: string; column?: string }) => {
      if (msg.type === "ready") {
        ready = true;
        void send(first.site);
      } else if (msg.type === "open" && typeof msg.uri === "string") {
        const document = await vscode.workspace.openTextDocument(vscode.Uri.parse(msg.uri));
        const at = columnPosition(document.getText(), msg.column);
        const position = at ? new vscode.Position(at.line, at.character) : new vscode.Position(0, 0);
        await vscode.window.showTextDocument(document, {
          viewColumn: vscode.ViewColumn.One,
          selection: new vscode.Range(position, position),
        });
      }
    }),
    vscode.window.onDidChangeActiveTextEditor((editor) => {
      const node = lineageFocus(editor?.document.uri.toString());
      if (ready && node) {
        void panel.webview.postMessage({ type: "focus", node });
      }
    }),
    vscode.workspace.onDidSaveTextDocument(async (document) => {
      if (!ready || !/\.(sql|ya?ml|csv)$/.test(document.uri.path)) {
        return;
      }
      const answer = await fetch(document.uri.toString(), false);
      if (answer?.site) {
        void send(answer.site);
      }
    }),
  ];
  panel.onDidDispose(() => {
    current = undefined;
    disposables.forEach((d) => d.dispose());
  });
  context.subscriptions.push(panel);
}
