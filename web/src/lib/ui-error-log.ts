export const LAST_UI_ERROR_KEY = "rdeckforge:last-ui-error";
export const UI_ERROR_EVENT = "rdeckforge:ui-error";

export type UiErrorSource = "error-boundary" | "window-error" | "unhandled-rejection";

export type UiErrorRecord = {
  message: string;
  stack: string | null;
  componentStack: string | null;
  source: UiErrorSource;
  occurredAt: string;
};

type UiErrorInput = {
  source: UiErrorSource;
  error?: unknown;
  message?: unknown;
  stack?: unknown;
  componentStack?: unknown;
};

const MAX_MESSAGE_LENGTH = 2_000;
const MAX_STACK_LENGTH = 12_000;

export function writeUiError(input: UiErrorInput): UiErrorRecord {
  const errorMessage = input.error instanceof Error ? input.error.message : input.error;
  const errorStack = input.error instanceof Error ? input.error.stack : null;
  const record: UiErrorRecord = {
    message: clippedText(input.message ?? errorMessage, MAX_MESSAGE_LENGTH) || "未知界面错误",
    stack: nullableText(input.stack ?? errorStack, MAX_STACK_LENGTH),
    componentStack: nullableText(input.componentStack, MAX_STACK_LENGTH),
    source: input.source,
    occurredAt: new Date().toISOString(),
  };

  try {
    localStorage.setItem(LAST_UI_ERROR_KEY, JSON.stringify(record));
    notifyUiErrorChange();
  } catch {
    // Error reporting must never become a second application failure.
  }

  return record;
}

export function readLastUiError(): UiErrorRecord | null {
  try {
    const raw = localStorage.getItem(LAST_UI_ERROR_KEY);
    if (!raw) {
      return null;
    }

    const value = JSON.parse(raw) as Partial<UiErrorRecord>;
    const occurredAt = typeof value.occurredAt === "string" ? value.occurredAt : "";
    const message = clippedText(value.message, MAX_MESSAGE_LENGTH);
    if (!message || !occurredAt) {
      return null;
    }

    return {
      message,
      stack: nullableText(value.stack, MAX_STACK_LENGTH),
      componentStack: nullableText(value.componentStack, MAX_STACK_LENGTH),
      source: isUiErrorSource(value.source) ? value.source : "error-boundary",
      occurredAt,
    };
  } catch {
    return null;
  }
}

export function clearLastUiError() {
  try {
    localStorage.removeItem(LAST_UI_ERROR_KEY);
    notifyUiErrorChange();
  } catch {
    // The diagnostics page can still continue when storage is unavailable.
  }
}

export function installGlobalUiErrorCapture() {
  if (typeof window === "undefined") {
    return () => undefined;
  }

  const onWindowError = (event: ErrorEvent) => {
    writeUiError({
      source: "window-error",
      error: event.error,
      message: event.message,
      stack: event.error instanceof Error ? event.error.stack : null,
    });
  };
  const onUnhandledRejection = (event: PromiseRejectionEvent) => {
    writeUiError({
      source: "unhandled-rejection",
      error: event.reason,
      message: event.reason,
    });
  };

  window.addEventListener("error", onWindowError);
  window.addEventListener("unhandledrejection", onUnhandledRejection);

  return () => {
    window.removeEventListener("error", onWindowError);
    window.removeEventListener("unhandledrejection", onUnhandledRejection);
  };
}

export function formatUiError(record: UiErrorRecord) {
  return [
    `[${record.occurredAt}] ${sourceLabel(record.source)}`,
    record.message,
    record.stack,
    record.componentStack ? `React component stack:\n${record.componentStack}` : null,
  ]
    .filter(Boolean)
    .join("\n\n");
}

export function sourceLabel(source: UiErrorSource) {
  const labels: Record<UiErrorSource, string> = {
    "error-boundary": "页面渲染",
    "window-error": "全局脚本",
    "unhandled-rejection": "异步任务",
  };
  return labels[source];
}

function isUiErrorSource(value: unknown): value is UiErrorSource {
  return value === "error-boundary" || value === "window-error" || value === "unhandled-rejection";
}

function nullableText(value: unknown, maxLength: number) {
  const text = clippedText(value, maxLength);
  return text || null;
}

function clippedText(value: unknown, maxLength: number) {
  const text = unknownText(value).trim();
  return text.length > maxLength ? `${text.slice(0, maxLength)}\n...[已截断]` : text;
}

function unknownText(value: unknown): string {
  if (typeof value === "string") {
    return value;
  }
  if (value instanceof Error) {
    return value.message;
  }
  if (value === null || value === undefined) {
    return "";
  }
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}

function notifyUiErrorChange() {
  if (typeof window !== "undefined") {
    window.dispatchEvent(new Event(UI_ERROR_EVENT));
  }
}
