import { execFileSync } from "node:child_process";
import { chmodSync, copyFileSync, mkdirSync } from "node:fs";
import { dirname, isAbsolute, join, resolve } from "node:path";
import { homedir } from "node:os";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(scriptDir, "..");
const profile = optionValue("--profile") ?? "release";

if (!new Set(["debug", "release"]).has(profile)) {
  throw new Error(`unsupported CLI build profile: ${profile}`);
}

const rustcInfo = execFileSync("rustc", ["-vV"], {
  cwd: projectRoot,
  encoding: "utf8",
});
const hostTriple = rustcInfo.match(/^host:\s+(.+)$/m)?.[1]?.trim();
if (!hostTriple) {
  throw new Error("failed to determine the Rust host target triple");
}

const configuredTarget =
  process.env.TAURI_ENV_TARGET_TRIPLE ?? process.env.CARGO_BUILD_TARGET ?? process.env.TARGET;
const targetTriple = configuredTarget || hostTriple;
const cargoArgs = ["build", "--bin", "rdeckforge"];
if (profile === "release") {
  cargoArgs.push("--release");
}
if (configuredTarget) {
  cargoArgs.push("--target", targetTriple);
}

const childEnv = sanitizedBuildEnvironment();
execFileSync("cargo", cargoArgs, {
  cwd: projectRoot,
  env: childEnv,
  stdio: "inherit",
});

const executableSuffix = targetTriple.includes("windows") ? ".exe" : "";
const cargoTargetRoot = resolveTargetDirectory(process.env.CARGO_TARGET_DIR);
const source = configuredTarget
  ? join(cargoTargetRoot, targetTriple, profile, `rdeckforge${executableSuffix}`)
  : join(cargoTargetRoot, profile, `rdeckforge${executableSuffix}`);
const destination = join(
  projectRoot,
  "src-tauri",
  "binaries",
  `rdeckforge-${targetTriple}${executableSuffix}`,
);

mkdirSync(dirname(destination), { recursive: true });
copyFileSync(source, destination);
if (!executableSuffix) {
  chmodSync(destination, 0o755);
}

console.log(`CLI sidecar: ${destination}`);

function optionValue(name) {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : undefined;
}

function resolveTargetDirectory(value) {
  if (!value) {
    return join(projectRoot, "target");
  }
  return isAbsolute(value) ? value : resolve(projectRoot, value);
}

function sanitizedBuildEnvironment() {
  const result = { ...process.env };
  for (const key of Object.keys(result)) {
    if (
      key.startsWith("TAURI_") ||
      key.startsWith("CARGO_MANIFEST_") ||
      key.startsWith("CARGO_PKG_") ||
      key === "OUT_DIR"
    ) {
      delete result[key];
    }
  }
  delete result.CARGO_BUILD_TARGET;
  const remapHome = `--remap-path-prefix=${homedir()}=~`;
  result.RUSTFLAGS = [result.RUSTFLAGS, remapHome].filter(Boolean).join(" ");
  return result;
}
