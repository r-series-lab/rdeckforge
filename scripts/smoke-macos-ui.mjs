import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

if (process.platform !== "darwin") {
  console.error("smoke:macos-ui 只能在 macOS 上运行。");
  process.exit(2);
}

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
const defaultApp = resolve(root, "target/release/bundle/macos/rDeckForge.app");
const appArgument = argumentValue("--app");
const appPath = resolve(appArgument ?? defaultApp);
const keepOpen = process.argv.includes("--keep-open");
const jxaScript = resolve(root, "scripts/smoke-macos-ui.jxa");

if (!existsSync(appPath)) {
  console.error(`没有找到待测应用：${appPath}`);
  console.error("请先运行 npm run build，或使用 --app /absolute/rDeckForge.app 指定路径。");
  process.exit(2);
}

let exitCode = 0;
try {
  closeExistingInstances();
  execFileSync("open", ["-n", appPath], { stdio: "ignore" });
  sleep(2_000);

  const output = execFileSync("osascript", ["-l", "JavaScript", jxaScript], {
    encoding: "utf8",
    timeout: 90_000,
  }).trim();
  const result = JSON.parse(output);
  if (!result.ok) {
    const visibleElements = Array.isArray(result.visibleElements)
      ? `\n可见元素：${JSON.stringify(result.visibleElements)}`
      : "";
    throw new Error(`${result.error || "原生页面测试失败"}${visibleElements}`);
  }

  console.log(
    JSON.stringify(
      {
        ok: true,
        command: "smoke.macos-ui",
        data: { appPath, pages: result.pages },
      },
      null,
      2,
    ),
  );
} catch (error) {
  exitCode = 1;
  const detail =
    error && typeof error === "object" && "stderr" in error && error.stderr
      ? String(error.stderr).trim()
      : "";
  console.error(
    JSON.stringify(
      {
        ok: false,
        command: "smoke.macos-ui",
        error: [error instanceof Error ? error.message : String(error), detail]
          .filter(Boolean)
          .join("\n"),
      },
      null,
      2,
    ),
  );
} finally {
  if (!keepOpen) {
    try {
      execFileSync("osascript", ["-e", 'tell application id "app.rseries.rdeckforge" to quit'], {
        stdio: "ignore",
      });
    } catch {
      // A failed launch has no application to close.
    }
  }
}

process.exit(exitCode);

function argumentValue(name) {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : null;
}

function sleep(milliseconds) {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, milliseconds);
}

function closeExistingInstances() {
  try {
    execFileSync("osascript", ["-e", 'tell application id "app.rseries.rdeckforge" to quit'], {
      stdio: "ignore",
    });
    sleep(800);
  } catch {
    // No running application is the normal clean-start case.
  }
  try {
    execFileSync("pkill", ["-x", "rdeckforge-tauri"], { stdio: "ignore" });
    sleep(300);
  } catch {
    // pkill returns non-zero when no stale process exists.
  }
}
