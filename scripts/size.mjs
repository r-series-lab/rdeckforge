import { existsSync, lstatSync, readdirSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
const paths = ["target", "web/node_modules", "web/dist"];

for (const relativePath of paths) {
  const absolutePath = resolve(root, relativePath);
  if (existsSync(absolutePath)) {
    console.log(`${formatBytes(directorySize(absolutePath))}\t${relativePath}`);
  }
}

function directorySize(path) {
  const stat = lstatSync(path);
  if (!stat.isDirectory() || stat.isSymbolicLink()) {
    return stat.size;
  }
  return readdirSync(path).reduce(
    (total, entry) => total + directorySize(resolve(path, entry)),
    0,
  );
}

function formatBytes(bytes) {
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value >= 10 || unit === 0 ? value.toFixed(0) : value.toFixed(1)}${units[unit]}`;
}
