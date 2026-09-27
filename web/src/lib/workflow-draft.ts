export type WorkflowTaskStatus = "draft" | "content_ready" | "validated" | "rendered" | "error";
export type WorkflowTaskStage = "handoff" | "validation" | "render";

export type WorkflowDraft = {
  taskId?: string | null;
  taskName?: string | null;
  templatePath: string;
  contentPath: string;
  recipe?: string | null;
  format?: string | null;
  outputPath?: string | null;
  briefPath?: string | null;
  promptPath?: string | null;
  validationStatus?: string | null;
  lastError?: string | null;
  status?: WorkflowTaskStatus;
  stage?: WorkflowTaskStage;
  source:
    | "prompt-ai-output"
    | "template-library"
    | "content-validation"
    | "render-workflow"
    | "task-center"
    | "restored-session";
  updatedAt: number;
};

export const WORKFLOW_DRAFT_STORAGE_KEY = "rdeckforge.workflow-draft.v1";

type DraftStorage = Pick<Storage, "getItem" | "setItem" | "removeItem">;

export function readWorkflowDraft(storage = browserStorage()): WorkflowDraft | null {
  if (!storage) {
    return null;
  }
  try {
    const raw = storage.getItem(WORKFLOW_DRAFT_STORAGE_KEY);
    if (!raw) {
      return null;
    }
    const value = JSON.parse(raw) as Partial<WorkflowDraft>;
    if (!hasString(value.templatePath) || !hasString(value.contentPath)) {
      return null;
    }
    const draft: WorkflowDraft = {
      taskId: optionalString(value.taskId),
      taskName: optionalString(value.taskName),
      templatePath: value.templatePath,
      contentPath: value.contentPath,
      recipe: optionalString(value.recipe),
      format: normalizeFormat(value.format),
      outputPath: optionalString(value.outputPath),
      briefPath: optionalString(value.briefPath),
      promptPath: optionalString(value.promptPath),
      validationStatus: optionalString(value.validationStatus),
      lastError: optionalString(value.lastError),
      status: normalizeStatus(value.status),
      stage: normalizeStage(value.stage),
      source: "restored-session",
      updatedAt: typeof value.updatedAt === "number" ? value.updatedAt : Date.now(),
    };
    return hasWorkflowValues(draft) ? draft : null;
  } catch {
    return null;
  }
}

export function writeWorkflowDraft(
  draft: WorkflowDraft,
  storage = browserStorage(),
): void {
  if (!storage) {
    return;
  }
  try {
    if (!hasWorkflowValues(draft)) {
      storage.removeItem(WORKFLOW_DRAFT_STORAGE_KEY);
      return;
    }
    storage.setItem(
      WORKFLOW_DRAFT_STORAGE_KEY,
      JSON.stringify({
        ...draft,
        format: normalizeFormat(draft.format),
        source: "restored-session",
      } satisfies WorkflowDraft),
    );
  } catch {
    // Workflow persistence should never block document generation.
  }
}

export function clearWorkflowDraft(storage = browserStorage()): void {
  try {
    storage?.removeItem(WORKFLOW_DRAFT_STORAGE_KEY);
  } catch {
    // Ignore unavailable or quota-limited browser storage.
  }
}

function browserStorage(): DraftStorage | null {
  return typeof localStorage === "undefined" ? null : localStorage;
}

function hasWorkflowValues(draft: Pick<WorkflowDraft, "templatePath" | "contentPath" | "outputPath">) {
  return Boolean(draft.templatePath.trim() || draft.contentPath.trim() || draft.outputPath?.trim());
}

function hasString(value: unknown): value is string {
  return typeof value === "string";
}

function optionalString(value: unknown): string | null {
  return typeof value === "string" ? value : null;
}

function normalizeFormat(value: unknown): "pptx" | "docx" | "xlsx" {
  return value === "docx" || value === "xlsx" ? value : "pptx";
}

function normalizeStatus(value: unknown): WorkflowTaskStatus {
  if (
    value === "content_ready" ||
    value === "validated" ||
    value === "rendered" ||
    value === "error"
  ) {
    return value;
  }
  return "draft";
}

function normalizeStage(value: unknown): WorkflowTaskStage {
  if (value === "handoff" || value === "validation") {
    return value;
  }
  return "render";
}
