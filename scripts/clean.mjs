import { spawnSync } from "node:child_process";
import { rmSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
const mode = process.argv[2] ?? "normal";
const dryRun = process.argv.includes("--dry-run");

if (!new Set(["build-cache", "normal", "all"]).has(mode)) {
  throw new Error(`unsupported clean mode: ${mode}`);
}

const cargoArgs = ["clean", "--manifest-path", "./src-tauri/Cargo.toml"];
if (mode === "build-cache") {
  cargoArgs.push("-p", "rdeckforge-core", "-p", "rdeckforge-tauri");
}

const paths = ["web/dist"];
if (mode === "build-cache") {
  paths.push("target/release/bundle");
}
if (mode === "all") {
  paths.push("web/node_modules", "renderers/pptx-node/node_modules");
}

if (dryRun) {
  console.log(JSON.stringify({ mode, cargo: ["cargo", ...cargoArgs], paths }, null, 2));
  process.exit(0);
}

const result = spawnSync("cargo", cargoArgs, {
  cwd: root,
  stdio: "inherit",
  shell: false,
});
if (result.status !== 0) {
  process.exit(result.status ?? 1);
}

for (const path of paths) {
  remove(path);
}

function remove(path) {
  rmSync(resolve(root, path), { recursive: true, force: true });
}
