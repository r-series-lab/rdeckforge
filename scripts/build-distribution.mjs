import { spawnSync } from "node:child_process";
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
const node = process.execPath;
const platform = optionValue("--platform") ?? process.platform;
const dryRun = process.argv.includes("--dry-run");
const generatedDir = resolve(root, "target/release-config");
const generatedConfig = resolve(generatedDir, "tauri.distribution.conf.json");

requireSuccess(
  run(node, [resolve(root, "scripts/release-preflight.mjs"), "--platform", platform]),
);
if (!dryRun) {
  requireSuccess(run(node, [resolve(root, "scripts/clean.mjs"), "build-cache"]));
}

const bundle = { createUpdaterArtifacts: true };
if (platform === "darwin") {
  bundle.macOS = { signingIdentity: process.env.APPLE_SIGNING_IDENTITY };
}
if (platform === "win32") {
  bundle.windows = {
    certificateThumbprint: process.env.RDECKFORGE_WINDOWS_CERTIFICATE_THUMBPRINT,
    digestAlgorithm: "sha256",
    timestampUrl:
      process.env.RDECKFORGE_WINDOWS_TIMESTAMP_URL || "http://timestamp.digicert.com",
  };
}

mkdirSync(generatedDir, { recursive: true });
writeFileSync(generatedConfig, `${JSON.stringify({ bundle }, null, 2)}\n`);

try {
  let buildStatus = 0;
  if (dryRun) {
    console.log(JSON.stringify({ platform, config: { bundle } }, null, 2));
  } else {
    buildStatus = run(node, [
      resolve(root, "scripts/build-release.mjs"),
      "--distribution-config",
      generatedConfig,
    ]);
  }
  if (buildStatus !== 0) {
    process.exitCode = buildStatus;
  }
} finally {
  rmSync(generatedConfig, { force: true });
}

function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: root,
    env: process.env,
    stdio: "inherit",
    shell: false,
  });
  return result.status ?? 1;
}

function requireSuccess(status) {
  if (status !== 0) {
    process.exit(status);
  }
}

function optionValue(name) {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : undefined;
}
