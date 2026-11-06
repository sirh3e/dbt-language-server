// Launcher for the extension-host suite: downloads a cached VS Code
// build (@vscode/test-electron, under .vscode-test/), copies the jaffle
// fixture (minus target/) into a throwaway workspace, points the
// extension at the cargo-built server via workspace settings, and runs
// src/test/suite.ts inside the extension host. Deterministic by the
// same rules as the nvim/emacs suites (specs/neovim-plugin §8).

import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import { runTests } from "@vscode/test-electron";

function copyFixture(src: string, dst: string): void {
  fs.cpSync(src, dst, {
    recursive: true,
    filter: (from) => path.basename(from) !== "target",
  });
}

async function main(): Promise<void> {
  const extensionRoot = path.resolve(__dirname, "../..");
  const repoRoot = path.resolve(extensionRoot, "../..");

  const exe = process.platform === "win32" ? ".exe" : "";
  const serverBin =
    process.env.COGITATOR_BIN ??
    path.join(
      process.env.CARGO_TARGET_DIR ?? path.join(repoRoot, "target"),
      "debug",
      `cogitator${exe}`
    );
  if (!fs.existsSync(serverBin)) {
    console.error(
      `FAIL vscode: server binary missing at ${serverBin} (cargo build -p cogitator)`
    );
    process.exit(1);
  }

  // per-run sandbox: fixture copy + user-data dir, cleaned on exit
  const sandbox = fs.mkdtempSync(path.join(os.tmpdir(), "cogitator-vscode-"));
  const workspace = path.join(sandbox, "jaffle_shop_duckdb");
  copyFixture(path.join(repoRoot, "testdata", "jaffle_shop_duckdb"), workspace);
  fs.mkdirSync(path.join(workspace, ".vscode"));
  fs.writeFileSync(
    path.join(workspace, ".vscode", "settings.json"),
    JSON.stringify({ "cogitator.serverPath": serverBin })
  );

  try {
    await runTests({
      extensionDevelopmentPath: extensionRoot,
      extensionTestsPath: path.resolve(__dirname, "suite"),
      launchArgs: [
        workspace,
        "--disable-extensions",
        "--disable-workspace-trust",
        "--disable-gpu",
        "--user-data-dir",
        path.join(sandbox, "user-data"),
      ],
      extensionTestsEnv: { COGITATOR_TEST_WORKSPACE: workspace },
    });
  } catch (err) {
    console.error(`FAIL vscode: extension-host run errored — ${err}`);
    process.exitCode = 1;
  } finally {
    fs.rmSync(sandbox, { recursive: true, force: true });
  }
}

void main();
