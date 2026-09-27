import { execFileSync } from "node:child_process";
import {
  chmodSync,
  copyFileSync,
  existsSync,
  lstatSync,
  mkdirSync,
  readlinkSync,
  symlinkSync,
  unlinkSync,
} from "node:fs";
import { homedir } from "node:os";
import { delimiter, dirname, join, resolve, win32 } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const defaultProjectRoot = resolve(scriptDir, "..");
const componentNames = ["rdeckforge", "rdeckforge-pptx"];

export function installCli(options = {}) {
  const platform = options.platform ?? process.platform;
  const projectRoot = resolve(options.projectRoot ?? defaultProjectRoot);
  const home = options.home ?? homedir();
  const environment = options.env ?? process.env;
  const executableSuffix = platform === "win32" ? ".exe" : "";
  const installDir = resolve(
    options.installDir ?? defaultInstallDirectory(platform, home, environment),
  );
  const sourceDir = resolveSourceDirectory({
    platform,
    projectRoot,
    appPath: options.appPath,
    sourceDir: options.sourceDir,
  });
  const targetTriple = options.targetTriple ?? environment.RDECKFORGE_TARGET_TRIPLE;
  const sources = componentNames.map((name) => ({
    name,
    source: resolveComponentSource({
      name,
      executableSuffix,
      sourceDir,
      projectRoot,
      targetTriple,
    }),
  }));

  mkdirSync(installDir, { recursive: true });
  const installedFiles = sources.map(({ name, source }) => {
    const destination = join(installDir, `${name}${executableSuffix}`);
    copyFileSync(source, destination);
    if (platform !== "win32") {
      chmodSync(destination, 0o755);
    }
    return destination;
  });

  let commandPath = installedFiles[0];
  let commandLinked = false;
  let pathDirectory = installDir;
  if (platform !== "win32") {
    const linkDir = resolve(options.linkDir ?? join(home, ".local", "bin"));
    mkdirSync(linkDir, { recursive: true });
    commandPath = join(linkDir, "rdeckforge");
    replaceOwnedCommandLink(commandPath, installedFiles[0], installDir);
    commandLinked = true;
    pathDirectory = linkDir;
  }

  const pathConfigured = pathContains(pathDirectory, environment.PATH, platform);
  return {
    commandPath,
    commandLinked,
    installDir,
    installedFiles,
    pathConfigured,
    pathHint: pathConfigured
      ? `${pathDirectory} is already present in PATH`
      : platform === "win32"
        ? `Add ${pathDirectory} to the user PATH, then open a new terminal`
        : `Add ${pathDirectory} to PATH`,
  };
}

export function defaultInstallDirectory(platform, home, environment = process.env) {
  if (platform === "win32") {
    const root =
      environment.LOCALAPPDATA ||
      environment.APPDATA ||
      win32.join(environment.USERPROFILE || home, "AppData", "Local");
    return win32.join(root, "rDeckForge", "bin");
  }
  if (platform === "darwin") {
    return join(home, "Library", "Application Support", "rDeckForge", "bin");
  }
  const dataRoot = environment.XDG_DATA_HOME || join(home, ".local", "share");
  return join(dataRoot, "rDeckForge", "bin");
}

function resolveSourceDirectory({ platform, projectRoot, appPath, sourceDir }) {
  if (sourceDir) {
    return resolve(sourceDir);
  }
  if (appPath) {
    const resolvedApp = resolve(appPath);
    return platform === "darwin" && resolvedApp.endsWith(".app")
      ? join(resolvedApp, "Contents", "MacOS")
      : resolvedApp;
  }
  if (platform === "darwin") {
    const appBinDir = join(
      projectRoot,
      "target",
      "release",
      "bundle",
      "macos",
      "rDeckForge.app",
      "Contents",
      "MacOS",
    );
    if (existsSync(appBinDir)) {
      return appBinDir;
    }
  }
  return join(projectRoot, "src-tauri", "binaries");
}

function resolveComponentSource({
  name,
  executableSuffix,
  sourceDir,
  projectRoot,
  targetTriple,
}) {
  const direct = join(sourceDir, `${name}${executableSuffix}`);
  if (existsSync(direct)) {
    return direct;
  }

  const triple = targetTriple || resolveHostTriple();
  const sidecar = join(
    projectRoot,
    "src-tauri",
    "binaries",
    `${name}-${triple}${executableSuffix}`,
  );
  if (existsSync(sidecar)) {
    return sidecar;
  }
  throw new Error(`missing packaged CLI component: checked ${direct} and ${sidecar}`);
}

function replaceOwnedCommandLink(commandPath, target, installDir) {
  const existing = lstatSync(commandPath, { throwIfNoEntry: false });
  if (existing) {
    if (!existing.isSymbolicLink()) {
      throw new Error(`refusing to replace non-symlink path: ${commandPath}`);
    }
    const currentTarget = resolve(dirname(commandPath), readlinkSync(commandPath));
    if (!currentTarget.startsWith(`${resolve(installDir)}/`) && currentTarget !== target) {
      throw new Error(`refusing to replace command link owned by another install: ${commandPath}`);
    }
    unlinkSync(commandPath);
  }
  symlinkSync(target, commandPath);
}

function pathContains(directory, pathValue, platform) {
  if (!pathValue) {
    return false;
  }
  const separator = platform === "win32" ? ";" : delimiter;
  const expected = normalizePathForComparison(directory, platform);
  return pathValue
    .split(separator)
    .some((entry) => normalizePathForComparison(entry, platform) === expected);
}

function normalizePathForComparison(value, platform) {
  const normalized = value.replace(/^"|"$/gu, "").replace(/[\\/]+$/gu, "");
  return platform === "win32" ? normalized.toLowerCase() : normalized;
}

function resolveHostTriple() {
  const rustcInfo = execFileSync("rustc", ["-vV"], { encoding: "utf8" });
  const targetTriple = rustcInfo.match(/^host:\s+(.+)$/mu)?.[1]?.trim();
  if (!targetTriple) {
    throw new Error("failed to determine the Rust host target triple");
  }
  return targetTriple;
}

function optionValue(name) {
  const index = process.argv.indexOf(name);
  if (index < 0) {
    return undefined;
  }
  const value = process.argv[index + 1];
  if (!value || value.startsWith("--")) {
    throw new Error(`missing value for ${name}`);
  }
  return value;
}

function isMainModule() {
  return Boolean(process.argv[1]) && import.meta.url === pathToFileURL(resolve(process.argv[1])).href;
}

if (isMainModule()) {
  try {
    const data = installCli({
      appPath: optionValue("--app"),
      sourceDir: optionValue("--source-dir"),
      installDir: optionValue("--install-dir"),
      linkDir: optionValue("--link-dir"),
      targetTriple: optionValue("--target"),
    });
    console.log(JSON.stringify({ ok: true, command: "cli.install", data }));
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    console.error(JSON.stringify({ ok: false, command: "cli.install", error: { message } }));
    process.exitCode = 1;
  }
}
