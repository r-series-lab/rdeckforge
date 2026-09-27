import { execFileSync } from "node:child_process";

const platform = optionValue("--platform") ?? process.platform;
const checks = [
  check(
    "updater-signing-key",
    "更新包签名私钥",
    hasValue("TAURI_SIGNING_PRIVATE_KEY"),
    "设置 TAURI_SIGNING_PRIVATE_KEY；该私钥必须长期备份，不能提交到仓库。",
  ),
];

if (platform === "darwin") {
  const identity = process.env.APPLE_SIGNING_IDENTITY?.trim() ?? "";
  const hasIdentity = identity.length > 0 && identity !== "-";
  const identityAvailable = hasIdentity && appleIdentityAvailable(identity);
  const certificateProvided = hasValue("APPLE_CERTIFICATE");
  checks.push(
    check(
      "macos-developer-id",
      "macOS Developer ID 签名",
      hasIdentity && (identityAvailable || certificateProvided),
      "设置 APPLE_SIGNING_IDENTITY，并在钥匙串安装对应证书，或同时提供 APPLE_CERTIFICATE。",
    ),
    check(
      "macos-notarization",
      "macOS 公证凭据",
      hasAppleIdCredentials() || hasAppStoreConnectCredentials(),
      "提供 APPLE_ID + APPLE_PASSWORD + APPLE_TEAM_ID，或 App Store Connect API 凭据。",
    ),
  );
} else if (platform === "win32") {
  checks.push(
    check(
      "windows-code-signing",
      "Windows 代码签名证书",
      hasValue("RDECKFORGE_WINDOWS_CERTIFICATE_THUMBPRINT"),
      "把证书导入当前用户证书库，并设置 RDECKFORGE_WINDOWS_CERTIFICATE_THUMBPRINT。",
    ),
  );
}

const missing = checks.filter((item) => !item.ok);
const result = {
  ok: missing.length === 0,
  command: "release.preflight",
  ...(missing.length === 0
    ? { data: { platform, checks } }
    : {
        error: {
          code: "release_credentials_missing",
          message: "正式发布所需的签名或公证凭据尚未配置。",
          platform,
          checks,
        },
      }),
};

console.log(JSON.stringify(result, null, 2));
if (missing.length > 0) {
  process.exit(2);
}

function check(id, label, ok, fix) {
  return { id, label, ok, ...(ok ? {} : { fix }) };
}

function hasValue(name) {
  return Boolean(process.env[name]?.trim());
}

function hasAppleIdCredentials() {
  return ["APPLE_ID", "APPLE_PASSWORD", "APPLE_TEAM_ID"].every(hasValue);
}

function hasAppStoreConnectCredentials() {
  const hasKeyLocation = hasValue("APPLE_API_KEY_PATH") || hasValue("API_PRIVATE_KEYS_DIR");
  return hasValue("APPLE_API_KEY") && hasValue("APPLE_API_ISSUER") && hasKeyLocation;
}

function appleIdentityAvailable(identity) {
  if (process.platform !== "darwin") {
    return false;
  }
  try {
    const identities = execFileSync("security", ["find-identity", "-p", "codesigning", "-v"], {
      encoding: "utf8",
    });
    return identities.includes(`\"${identity}\"`);
  } catch {
    return false;
  }
}

function optionValue(name) {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : undefined;
}
