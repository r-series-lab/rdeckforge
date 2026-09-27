import { execFileSync, spawnSync } from "node:child_process";
import { existsSync, readdirSync, rmSync, statSync } from "node:fs";
import { basename, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
const srcTauri = resolve(root, "src-tauri");
const bundleRoot = resolve(root, "target/release/bundle");
const macosBundle = resolve(bundleRoot, "macos");
const appPath = resolve(macosBundle, "rDeckForge.app");
const verifyScript = resolve(root, "scripts/verify-release.mjs");
const npmCommand = process.platform === "win32" ? "npm.cmd" : "npm";
const distributionConfig = optionValue("--distribution-config");
const buildArgs = [
  "--prefix",
  "../web",
  "exec",
  "tauri",
  "--",
  "build",
];
if (distributionConfig) {
  buildArgs.push("--config", distributionConfig);
}

const build = spawnSync(
  npmCommand,
  buildArgs,
  { cwd: srcTauri, stdio: "inherit" },
);

if (build.status === 0) {
  process.exit(verifyRelease());
}
if (process.platform !== "darwin" || !existsSync(appPath)) {
  process.exit(build.status ?? 1);
}

const rwImage = newestReadWriteDmg(macosBundle);
if (!rwImage) {
  process.exit(build.status ?? 1);
}
const dmgName = basename(rwImage).replace(/^rw\.\d+\./, "");
const dmgPath = resolve(bundleRoot, "dmg", dmgName);

try {
  detachImage(rwImage);
  rmSync(dmgPath, { force: true });
  execFileSync(
    "hdiutil",
    ["convert", rwImage, "-format", "UDZO", "-imagekey", "zlib-level=9", "-o", dmgPath],
    { stdio: "inherit" },
  );
  execFileSync("hdiutil", ["verify", dmgPath], { stdio: "inherit" });
  rmSync(rwImage, { force: true });
  console.log(`Recovered DMG bundle: ${dmgPath}`);
  process.exit(verifyRelease());
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(build.status ?? 1);
}

function verifyRelease() {
  const result = spawnSync(process.execPath, [verifyScript], {
    cwd: root,
    env: {
      ...process.env,
      RDECKFORGE_VERIFY_UPDATER: distributionConfig ? "1" : "0",
      RDECKFORGE_VERIFY_DISTRIBUTION: distributionConfig ? "1" : "0",
    },
    stdio: "inherit",
  });
  return result.status ?? 1;
}

function optionValue(name) {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : undefined;
}

function newestReadWriteDmg(directory) {
  if (!existsSync(directory)) {
    return null;
  }
  return readdirSync(directory)
    .filter((name) => /^rw\..+\.dmg$/.test(name))
    .map((name) => resolve(directory, name))
    .sort((left, right) => statSync(right).mtimeMs - statSync(left).mtimeMs)[0] ?? null;
}

function detachImage(imagePath) {
  const info = execFileSync("hdiutil", ["info"], { encoding: "utf8" });
  const block = info
    .split(/^={20,}$/m)
    .find((candidate) => candidate.includes(`image-path      : ${imagePath}`));
  const device = block?.match(/^\/dev\/(disk\d+)\b/m)?.[1];
  if (!device) {
    return;
  }
  for (let attempt = 1; attempt <= 10; attempt += 1) {
    const result = spawnSync("hdiutil", ["detach", `/dev/${device}`], { stdio: "inherit" });
    if (result.status === 0) {
      return;
    }
    sleep(Math.min(attempt * 1_000, 5_000));
  }
  throw new Error(`failed to detach /dev/${device} after 10 attempts`);
}

function sleep(milliseconds) {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, milliseconds);
}
