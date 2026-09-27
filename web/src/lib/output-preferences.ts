export type OutputConflictPolicy = "increment" | "overwrite";

export type OutputPreferences = {
  defaultDirectory: string;
  fileNamePattern: string;
  conflictPolicy: OutputConflictPolicy;
  autoOpen: boolean;
};

export type OutputSuggestionSource = "preferences" | "content-directory" | "none";

export type OutputSuggestion = {
  path: string;
  source: OutputSuggestionSource;
};

export const OUTPUT_PREFERENCES_STORAGE_KEY = "rdeckforge.output-preferences.v1";

export const DEFAULT_OUTPUT_PREFERENCES: OutputPreferences = {
  defaultDirectory: "",
  fileNamePattern: "{content}-{template}",
  conflictPolicy: "increment",
  autoOpen: true,
};

type PreferenceStorage = Pick<Storage, "getItem" | "setItem">;

export function readOutputPreferences(storage = browserStorage()): OutputPreferences {
  if (!storage) {
    return { ...DEFAULT_OUTPUT_PREFERENCES };
  }
  try {
    const raw = storage.getItem(OUTPUT_PREFERENCES_STORAGE_KEY);
    if (!raw) {
      return { ...DEFAULT_OUTPUT_PREFERENCES };
    }
    const value = JSON.parse(raw) as Partial<OutputPreferences>;
    return normalizeOutputPreferences(value);
  } catch {
    return { ...DEFAULT_OUTPUT_PREFERENCES };
  }
}

export function saveOutputPreferences(
  preferences: OutputPreferences,
  storage = browserStorage(),
) {
  const normalized = normalizeOutputPreferences(preferences);
  try {
    storage?.setItem(OUTPUT_PREFERENCES_STORAGE_KEY, JSON.stringify(normalized));
  } catch {
    // Local preferences should never block document generation.
  }
  return normalized;
}

export function buildSuggestedOutputPath(
  preferences: OutputPreferences,
  context: {
    contentPath: string;
    templateName: string;
    format: "pptx" | "docx" | "xlsx";
  },
  date = new Date(),
) {
  const directory = preferences.defaultDirectory.trim();
  if (!directory) {
    return "";
  }
  const replacements: Record<string, string> = {
    content: fileStem(context.contentPath) || "document",
    template: context.templateName.trim() || "template",
    date: formatDate(date),
    format: context.format,
  };
  let fileName = (preferences.fileNamePattern.trim() || DEFAULT_OUTPUT_PREFERENCES.fileNamePattern)
    .replace(/\{(content|template|date|format)\}/g, (_, key: string) => replacements[key] ?? "");
  fileName = sanitizeFileName(fileName) || replacements.content;
  return joinPath(directory, `${fileName}.${context.format}`);
}

export function suggestOutputPath(
  preferences: OutputPreferences,
  context: {
    contentPath: string;
    templateName: string;
    format: "pptx" | "docx" | "xlsx";
  },
  date = new Date(),
): OutputSuggestion {
  const preferred = buildSuggestedOutputPath(preferences, context, date);
  if (preferred) {
    return { path: preferred, source: "preferences" };
  }
  const contentDirectory = directoryName(context.contentPath);
  if (!contentDirectory) {
    return { path: "", source: "none" };
  }
  const fallback = buildSuggestedOutputPath(
    { ...preferences, defaultDirectory: contentDirectory },
    context,
    date,
  );
  return fallback
    ? { path: fallback, source: "content-directory" }
    : { path: "", source: "none" };
}

export function outputPatternPreview(pattern: string) {
  return buildSuggestedOutputPath(
    {
      ...DEFAULT_OUTPUT_PREFERENCES,
      defaultDirectory: "/输出目录",
      fileNamePattern: pattern,
    },
    {
      contentPath: "/内容/护理质量改进.json",
      templateName: "现代卡片式",
      format: "pptx",
    },
    new Date(2026, 6, 8),
  ).split("/").pop()!;
}

function normalizeOutputPreferences(value: Partial<OutputPreferences>): OutputPreferences {
  return {
    defaultDirectory:
      typeof value.defaultDirectory === "string" ? value.defaultDirectory.trim() : "",
    fileNamePattern:
      typeof value.fileNamePattern === "string" && value.fileNamePattern.trim()
        ? value.fileNamePattern.trim()
        : DEFAULT_OUTPUT_PREFERENCES.fileNamePattern,
    conflictPolicy: value.conflictPolicy === "overwrite" ? "overwrite" : "increment",
    autoOpen: typeof value.autoOpen === "boolean" ? value.autoOpen : true,
  };
}

function browserStorage(): PreferenceStorage | null {
  return typeof localStorage === "undefined" ? null : localStorage;
}

function fileStem(path: string) {
  const name = path.split(/[\\/]/).filter(Boolean).pop() ?? "";
  return name.replace(/\.(pptx|docx|xlsx|json|md|markdown)$/i, "").trim();
}

function directoryName(path: string) {
  const trimmed = path.trim();
  const slashIndex = Math.max(trimmed.lastIndexOf("/"), trimmed.lastIndexOf("\\"));
  if (slashIndex <= 0) {
    return "";
  }
  return trimmed.slice(0, slashIndex);
}

function sanitizeFileName(value: string) {
  return value
    .replace(/[<>:"/\\|?*\u0000-\u001f]/g, "-")
    .replace(/\s+/g, " ")
    .replace(/[- ]{2,}/g, "-")
    .replace(/^[. -]+|[. -]+$/g, "")
    .slice(0, 120);
}

function joinPath(directory: string, fileName: string) {
  const separator = directory.includes("\\") && !directory.includes("/") ? "\\" : "/";
  return `${directory.replace(/[\\/]+$/, "")}${separator}${fileName}`;
}

function formatDate(date: Date) {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}${month}${day}`;
}
