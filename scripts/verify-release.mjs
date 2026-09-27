import { execFileSync, spawnSync } from "node:child_process";
import { existsSync, readdirSync, statSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
const targetTriple = resolveTargetTriple();
const executableSuffix = targetTriple.includes("windows") ? ".exe" : "";
const expectedSidecars = [
  resolve(root, "src-tauri/binaries", `rdeckforge-${targetTriple}${executableSuffix}`),
  resolve(root, "src-tauri/binaries", `rdeckforge-pptx-${targetTriple}${executableSuffix}`),
];
const missingSidecars = expectedSidecars.filter((path) => !isNonEmptyFile(path));
const bundles = releaseBundles(targetTriple);
const bundleSetComplete = hasRequiredBundles(targetTriple, bundles);
const updaterRequired = process.env.RDECKFORGE_VERIFY_UPDATER === "1";
const updaterArtifacts = updaterRequired ? releaseUpdaterArtifacts(targetTriple) : [];
const updaterSetComplete = !updaterRequired || hasRequiredUpdaterArtifacts(targetTriple, updaterArtifacts);
const distributionRequired = process.env.RDECKFORGE_VERIFY_DISTRIBUTION === "1";
const distributionChecks = distributionRequired
  ? verifyDistributionSignatures(targetTriple, expectedSidecars, bundles)
  : [];
const distributionSetComplete = distributionChecks.every((check) => check.ok);
const releaseArtifactsMissing =
  missingSidecars.length > 0 || !bundleSetComplete || !updaterSetComplete;

if (
  releaseArtifactsMissing ||
  !distributionSetComplete
) {
  console.error(
    JSON.stringify(
      {
        ok: false,
        command: "release.verify",
        error: {
          code: releaseArtifactsMissing
            ? "missing_release_artifact"
            : "distribution_validation_failed",
          message: releaseArtifactsMissing
            ? "Release build did not produce every required sidecar and platform bundle."
            : "Release artifacts exist but did not pass platform trust validation.",
          missingSidecars,
          bundles,
          updaterArtifacts,
          distributionChecks,
          targetTriple,
        },
      },
      null,
      2,
    ),
  );
  process.exit(1);
}

function hasRequiredBundles(triple, bundles) {
  if (triple.includes("apple-darwin")) {
    return (
      bundles.some((path) => path.endsWith(".app")) &&
      bundles.some((path) => path.toLowerCase().endsWith(".dmg"))
    );
  }
  return bundles.length > 0;
}

console.log(
  JSON.stringify(
    {
      ok: true,
      command: "release.verify",
      data: {
        targetTriple,
        sidecars: expectedSidecars.map(fileSummary),
        bundles: bundles.map(fileSummary),
        updaterArtifacts: updaterArtifacts.map(fileSummary),
        distributionChecks,
      },
    },
    null,
    2,
  ),
);

function resolveTargetTriple() {
  const configured =
    process.env.TAURI_ENV_TARGET_TRIPLE?.trim() ||
    process.env.CARGO_BUILD_TARGET?.trim() ||
    process.env.TARGET?.trim();
  if (configured) {
    return configured;
  }
  const rustcInfo = execFileSync("rustc", ["-vV"], { cwd: root, encoding: "utf8" });
  const hostTriple = rustcInfo.match(/^host:\s+(.+)$/m)?.[1]?.trim();
  if (!hostTriple) {
    throw new Error("failed to determine the Rust host target triple");
  }
  return hostTriple;
}

function releaseBundles(triple) {
  const bundleRoot = resolve(root, "target/release/bundle");
  if (triple.includes("apple-darwin")) {
    return [
      resolve(bundleRoot, "macos/rDeckForge.app"),
      ...filesWithExtension(resolve(bundleRoot, "dmg"), ".dmg"),
    ].filter(existsSync);
  }
  if (triple.includes("windows")) {
    return [
      ...filesWithExtension(resolve(bundleRoot, "nsis"), ".exe"),
      ...filesWithExtension(resolve(bundleRoot, "msi"), ".msi"),
    ];
  }
  if (triple.includes("linux")) {
    return [
      ...filesWithExtension(resolve(bundleRoot, "appimage"), ".AppImage"),
      ...filesWithExtension(resolve(bundleRoot, "deb"), ".deb"),
      ...filesWithExtension(resolve(bundleRoot, "rpm"), ".rpm"),
    ];
  }
  return [];
}

function releaseUpdaterArtifacts(triple) {
  const bundleRoot = resolve(root, "target/release/bundle");
  if (triple.includes("apple-darwin")) {
    return [
      ...filesWithExtension(resolve(bundleRoot, "macos"), ".tar.gz"),
      ...filesWithExtension(resolve(bundleRoot, "macos"), ".sig"),
    ];
  }
  if (triple.includes("windows")) {
    return [
      ...filesWithExtension(resolve(bundleRoot, "nsis"), ".sig"),
      ...filesWithExtension(resolve(bundleRoot, "msi"), ".sig"),
    ];
  }
  if (triple.includes("linux")) {
    return [
      ...filesWithExtension(resolve(bundleRoot, "appimage"), ".sig"),
    ];
  }
  return [];
}

function hasRequiredUpdaterArtifacts(triple, artifacts) {
  if (triple.includes("apple-darwin")) {
    return (
      artifacts.some((path) => path.endsWith(".tar.gz")) &&
      artifacts.some((path) => path.endsWith(".tar.gz.sig"))
    );
  }
  return artifacts.some((path) => path.endsWith(".sig"));
}

function verifyDistributionSignatures(triple, sidecars, releaseFiles) {
  if (triple.includes("apple-darwin")) {
    const app = releaseFiles.find((path) => path.endsWith(".app"));
    if (!app) {
      return [{ id: "macos-signature", ok: false, message: "macOS app bundle is missing." }];
    }
    return [
      commandCheck("macos-signature", "Nested code signatures", "codesign", [
        "--verify",
        "--deep",
        "--strict",
        "--verbose=2",
        app,
      ]),
      macDeveloperIdCheck(app),
      commandCheck("macos-gatekeeper", "Gatekeeper assessment", "spctl", [
        "-a",
        "-vv",
        "--type",
        "execute",
        app,
      ]),
      commandCheck("macos-staple", "Notarization ticket", "xcrun", [
        "stapler",
        "validate",
        app,
      ]),
    ];
  }
  if (triple.includes("windows")) {
    return [...sidecars, ...releaseFiles].map((path) =>
      commandCheck("windows-authenticode", path, "powershell.exe", [
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        "$signature = Get-AuthenticodeSignature -LiteralPath $args[0]; if ($signature.Status -ne 'Valid') { Write-Error ($signature.Status.ToString() + ': ' + $signature.StatusMessage); exit 1 }",
        path,
      ]),
    );
  }
  return [];
}

function macDeveloperIdCheck(app) {
  const result = spawnSync("codesign", ["-dvvv", app], { encoding: "utf8" });
  const details = `${result.stdout ?? ""}\n${result.stderr ?? ""}`;
  const hasAuthority = details.includes("Authority=Developer ID Application:");
  const teamIdentifier = details.match(/^TeamIdentifier=(.+)$/m)?.[1]?.trim();
  const ok = result.status === 0 && hasAuthority && teamIdentifier && teamIdentifier !== "not set";
  return {
    id: "macos-developer-id",
    label: "Developer ID identity",
    ok: Boolean(ok),
    ...(ok
      ? {}
      : { message: "Bundle is not signed with a Developer ID Application identity and Team ID." }),
  };
}

function commandCheck(id, label, command, args) {
  try {
    execFileSync(command, args, { encoding: "utf8", stdio: "pipe" });
    return { id, label, ok: true };
  } catch (error) {
    const stderr = error?.stderr?.toString?.().trim();
    return {
      id,
      label,
      ok: false,
      message: stderr || (error instanceof Error ? error.message : String(error)),
    };
  }
}

function filesWithExtension(directory, extension) {
  if (!existsSync(directory)) {
    return [];
  }
  return readdirSync(directory)
    .filter((name) => name.toLowerCase().endsWith(extension.toLowerCase()))
    .map((name) => resolve(directory, name))
    .filter(isNonEmptyFile);
}

function isNonEmptyFile(path) {
  return existsSync(path) && statSync(path).isFile() && statSync(path).size > 0;
}

function fileSummary(path) {
  const stat = statSync(path);
  return { path, bytes: stat.isDirectory() ? directorySize(path) : stat.size };
}

function directorySize(path) {
  return readdirSync(path, { withFileTypes: true }).reduce((total, entry) => {
    const child = resolve(path, entry.name);
    return total + (entry.isDirectory() ? directorySize(child) : statSync(child).size);
  }, 0);
}
