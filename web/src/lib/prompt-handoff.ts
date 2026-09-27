export const DEFAULT_PROMPT_PACK_PATH = "examples/prompt-packs/template-contract-handoff-v1";

export function resolvePromptPackPath(value: string | null | undefined) {
  const trimmed = value?.trim() ?? "";
  if (!trimmed || /^\d+$/.test(trimmed)) {
    return DEFAULT_PROMPT_PACK_PATH;
  }
  return trimmed;
}

export function suggestedPromptOutputPath(
  currentPath: string,
  briefPath: string,
  templatePath: string,
) {
  const current = currentPath.trim();
  if (current) {
    return current;
  }
  const directory = pathDirectory(briefPath) || pathDirectory(templatePath) || ".";
  const stem = pathStem(briefPath) || "ai-handoff";
  return joinPath(directory, `${stem}.prompt.md`);
}

export function suggestedContentOutputPath(
  currentPath: string,
  briefPath: string,
  templatePath: string,
  extension: string,
) {
  const current = currentPath.trim();
  if (current) {
    return ensureContentExtension(current, extension);
  }
  const directory = pathDirectory(briefPath) || pathDirectory(templatePath) || ".";
  const stem = pathStem(briefPath) || "ai-output";
  return joinPath(directory, `${stem}.content.${extension}`);
}

export function ensureContentExtension(path: string, extension: string) {
  const trimmed = path.trim();
  if (!trimmed) {
    return "";
  }
  if (trimmed.match(/\.(json|md|markdown)$/i)) {
    return trimmed.replace(/\.(json|md|markdown)$/i, `.${extension}`);
  }
  return `${trimmed}.${extension}`;
}

function pathDirectory(path: string) {
  const trimmed = path.trim().replace(/[\\/]+$/g, "");
  const index = Math.max(trimmed.lastIndexOf("/"), trimmed.lastIndexOf("\\"));
  return index > 0 ? trimmed.slice(0, index) : "";
}

function pathStem(path: string) {
  const trimmed = path.trim().replace(/[\\/]+$/g, "");
  const index = Math.max(trimmed.lastIndexOf("/"), trimmed.lastIndexOf("\\"));
  const fileName = index >= 0 ? trimmed.slice(index + 1) : trimmed;
  return fileName.replace(/\.[^.]+$/g, "").trim();
}

function joinPath(directory: string, fileName: string) {
  const separator = directory.includes("\\") && !directory.includes("/") ? "\\" : "/";
  return `${directory.replace(/[\\/]+$/g, "")}${separator}${fileName}`;
}
