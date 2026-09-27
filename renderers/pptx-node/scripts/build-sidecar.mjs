import { execFileSync } from "node:child_process";
import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { homedir } from "node:os";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repoRoot = resolve(packageRoot, "../..");
const distDir = join(packageRoot, "dist-sidecar");
const bundleFile = join(distDir, "renderer.cjs");
const seaConfigFile = join(distDir, "sea-config.json");
const seaBlobFile = join(distDir, "sea-prep.blob");
const seaNode = resolveSeaNode();
const targetTriple = resolveTargetTriple();
const temporaryEsbuild = prepareEsbuildBinary();
const executableSuffix = process.platform === "win32" ? ".exe" : "";
const sidecarFile = join(
  repoRoot,
  "src-tauri",
  "binaries",
  `rdeckforge-pptx-${targetTriple}${executableSuffix}`,
);

assertNativeTarget(targetTriple);
rmSync(distDir, { recursive: true, force: true });
mkdirSync(distDir, { recursive: true });
mkdirSync(dirname(sidecarFile), { recursive: true });

const { build } = await import("esbuild");
await build({
  entryPoints: [join(packageRoot, "src", "index.ts")],
  outfile: bundleFile,
  bundle: true,
  platform: "node",
  format: "cjs",
  target: "node22",
  legalComments: "none",
  sourcemap: false,
});

writeFileSync(
  seaConfigFile,
  `${JSON.stringify(
    {
      main: "renderer.cjs",
      output: "sea-prep.blob",
      disableExperimentalSEAWarning: true,
      useSnapshot: false,
      useCodeCache: false,
    },
    null,
    2,
  )}\n`,
);

run(seaNode, ["--experimental-sea-config", "sea-config.json"], distDir);
rmSync(sidecarFile, { force: true });
copyFileSync(seaNode, sidecarFile);

if (process.platform !== "win32") {
  chmodSync(sidecarFile, 0o755);
}
if (process.platform === "win32") {
  const signTool = resolveWindowsSignTool();
  run(signTool, ["remove", "/s", sidecarFile]);
}
if (process.platform === "darwin") {
  run("codesign", ["--remove-signature", sidecarFile]);
}

const postjectCli = join(packageRoot, "node_modules", "postject", "dist", "cli.js");
const postjectArgs = [
  postjectCli,
  sidecarFile,
  "NODE_SEA_BLOB",
  seaBlobFile,
  "--sentinel-fuse",
  "NODE_SEA_FUSE_fce680ab2cc467b6e072b8b5df1996b2",
  "--overwrite",
];
if (process.platform === "darwin") {
  postjectArgs.push("--macho-segment-name", "NODE_SEA");
}
run(process.execPath, postjectArgs);

if (process.platform === "darwin") {
  run("codesign", ["--force", "--sign", "-", sidecarFile]);
}
if (temporaryEsbuild) {
  rmSync(temporaryEsbuild, { force: true });
}

console.log(`PPTX standalone sidecar: ${sidecarFile}`);
console.log(`SEA Node runtime: ${seaNode}`);

function resolveSeaNode() {
  const configured = process.env.RDECKFORGE_SEA_NODE?.trim();
  if (configured) {
    if (!hasSeaFuse(configured)) {
      throw new Error(
        `RDECKFORGE_SEA_NODE is not a self-contained SEA-capable Node binary: ${configured}`,
      );
    }
    return configured;
  }

  const candidates = [process.execPath, ...versionManagerNodeCandidates()];
  const selected = candidates.find((candidate) => hasSeaFuse(candidate));
  if (!selected) {
    throw new Error(
      "No self-contained SEA-capable Node binary was found. Install Node 22.20+ with nvm/fnm/Volta or set RDECKFORGE_SEA_NODE. Homebrew's dynamically linked Node launcher cannot be injected.",
    );
  }
  return selected;
}

function versionManagerNodeCandidates() {
  const home = homedir();
  const roots = [
    join(home, ".nvm", "versions", "node"),
    join(home, ".fnm", "node-versions"),
    join(home, ".local", "share", "fnm", "node-versions"),
    join(home, ".volta", "tools", "image", "node"),
  ];
  const executableParts =
    process.platform === "win32" ? ["node.exe"] : ["bin", "node"];
  const candidates = [];
  for (const root of roots) {
    if (!existsSync(root)) {
      continue;
    }
    const versions = readdirSync(root, { withFileTypes: true })
      .filter((entry) => entry.isDirectory())
      .map((entry) => entry.name)
      .sort()
      .reverse();
    for (const version of versions) {
      const versionRoot = join(root, version);
      candidates.push(join(versionRoot, ...executableParts));
      candidates.push(join(versionRoot, "installation", ...executableParts));
    }
  }
  return candidates;
}

function prepareEsbuildBinary() {
  if (process.platform !== "darwin") {
    return null;
  }
  const source = join(
    packageRoot,
    "node_modules",
    "@esbuild",
    `darwin-${process.arch}`,
    "bin",
    "esbuild",
  );
  if (!existsSync(source)) {
    return null;
  }
  const target = join(tmpdir(), `rdeckforge-esbuild-${process.arch}-${process.pid}`);
  copyFileSync(source, target);
  chmodSync(target, 0o755);
  process.env.ESBUILD_BINARY_PATH = target;
  return target;
}

function resolveWindowsSignTool() {
  const configured = process.env.RDECKFORGE_SIGNTOOL?.trim();
  if (configured && existsSync(configured)) {
    return configured;
  }

  try {
    const located = execFileSync("where.exe", ["signtool.exe"], { encoding: "utf8" })
      .split(/\r?\n/u)
      .map((value) => value.trim())
      .find(Boolean);
    if (located) {
      return located;
    }
  } catch {
    // Fall through to the Windows SDK search path.
  }

  const programFiles = process.env["ProgramFiles(x86)"] || process.env.ProgramFiles;
  const sdkBin = programFiles ? join(programFiles, "Windows Kits", "10", "bin") : "";
  if (sdkBin && existsSync(sdkBin)) {
    const architecture = process.arch === "arm64" ? "arm64" : "x64";
    const versions = readdirSync(sdkBin, { withFileTypes: true })
      .filter((entry) => entry.isDirectory())
      .map((entry) => entry.name)
      .sort()
      .reverse();
    for (const version of versions) {
      const candidate = join(sdkBin, version, architecture, "signtool.exe");
      if (existsSync(candidate)) {
        return candidate;
      }
    }
  }

  throw new Error(
    "signtool.exe is required to remove Node's original signature before SEA injection. Install the Windows 10/11 SDK or set RDECKFORGE_SIGNTOOL.",
  );
}

function hasSeaFuse(path) {
  if (!existsSync(path)) {
    return false;
  }
  try {
    return readFileSync(path).includes(
      Buffer.from("NODE_SEA_FUSE_fce680ab2cc467b6e072b8b5df1996b2"),
    );
  } catch {
    return false;
  }
}

function resolveTargetTriple() {
  const configured = process.env.TAURI_ENV_TARGET_TRIPLE?.trim();
  if (configured) {
    return configured;
  }
  const rustcInfo = execFileSync("rustc", ["-vV"], { encoding: "utf8" });
  const hostLine = rustcInfo
    .split(/\r?\n/u)
    .find((line) => line.startsWith("host: "));
  if (!hostLine) {
    throw new Error("Unable to determine the Rust target triple from `rustc -vV`.");
  }
  return hostLine.slice("host: ".length).trim();
}

function assertNativeTarget(triple) {
  const platformMatches =
    (process.platform === "darwin" && triple.includes("apple-darwin")) ||
    (process.platform === "win32" && triple.includes("windows")) ||
    (process.platform === "linux" && triple.includes("linux"));
  const archMatches =
    (process.arch === "arm64" && (triple.startsWith("aarch64-") || triple.startsWith("arm64-"))) ||
    (process.arch === "x64" && triple.startsWith("x86_64-"));
  if (!platformMatches || !archMatches) {
    throw new Error(
      `Node SEA sidecars must be built natively. Runtime ${process.platform}/${process.arch} cannot produce ${triple}.`,
    );
  }
}

function run(command, args, cwd = packageRoot) {
  execFileSync(command, args, {
    cwd,
    stdio: "inherit",
  });
}
