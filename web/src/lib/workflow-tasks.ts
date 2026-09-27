import type { PageKey } from "@/app-modules";
import type {
  WorkflowDraft,
  WorkflowTaskStage,
  WorkflowTaskStatus,
} from "@/lib/workflow-draft";

export type WorkflowTaskRecord = {
  id: string;
  name: string;
  status: WorkflowTaskStatus;
  stage: WorkflowTaskStage;
  source: string;
  templatePath: string;
  contentPath: string;
  outputPath?: string | null;
  format?: string | null;
  recipe?: string | null;
  briefPath?: string | null;
  promptPath?: string | null;
  validationStatus?: string | null;
  lastError?: string | null;
  templateAvailable: boolean;
  contentAvailable: boolean;
  outputAvailable: boolean;
  createdAt: string;
  updatedAt: string;
};

export type WorkflowTaskInput = {
  id: string;
  name: string;
  status: WorkflowTaskStatus;
  stage: WorkflowTaskStage;
  source: string;
  templatePath: string;
  contentPath: string;
  outputPath?: string | null;
  format?: string | null;
  recipe?: string | null;
  briefPath?: string | null;
  promptPath?: string | null;
  validationStatus?: string | null;
  lastError?: string | null;
};

export type TaskReadinessState = "ready" | "warning" | "missing";

export type TaskReadinessItem = {
  key: "template" | "content" | "output" | "brief" | "prompt";
  label: string;
  state: TaskReadinessState;
  detail: string;
};

export type TaskNextAction = {
  page: PageKey;
  label: string;
  detail: string;
};

export function ensureTaskIdentity(draft: WorkflowDraft): WorkflowDraft {
  const taskId = draft.taskId?.trim() || createTaskId();
  return {
    ...draft,
    taskId,
    taskName: draft.taskName?.trim() || deriveTaskName(draft),
    status: draft.status ?? inferTaskStatus(draft),
    stage: draft.stage ?? inferTaskStage(draft),
  };
}

export function taskInputFromDraft(draftValue: WorkflowDraft): WorkflowTaskInput {
  const draft = ensureTaskIdentity(draftValue);
  return {
    id: draft.taskId!,
    name: draft.taskName!,
    status: draft.status!,
    stage: draft.stage!,
    source: draft.source,
    templatePath: draft.templatePath,
    contentPath: draft.contentPath,
    outputPath: draft.outputPath,
    format: draft.format,
    recipe: draft.recipe,
    briefPath: draft.briefPath,
    promptPath: draft.promptPath,
    validationStatus: draft.validationStatus,
    lastError: draft.lastError,
  };
}

export function draftFromTask(task: WorkflowTaskRecord): WorkflowDraft {
  return {
    taskId: task.id,
    taskName: task.name,
    templatePath: task.templatePath,
    contentPath: task.contentPath,
    outputPath: task.outputPath,
    format: task.format,
    recipe: task.recipe,
    briefPath: task.briefPath,
    promptPath: task.promptPath,
    validationStatus: task.validationStatus,
    lastError: task.lastError,
    status: task.status,
    stage: task.stage,
    source: "task-center",
    updatedAt: Date.now(),
  };
}

export function duplicateTaskDraft(task: WorkflowTaskRecord): WorkflowDraft {
  return {
    ...draftFromTask(task),
    taskId: createTaskId(),
    taskName: `${task.name} 副本`,
    status: task.contentPath ? "content_ready" : "draft",
    validationStatus: null,
    lastError: null,
    source: "task-center",
  };
}

export function pageForTask(task: Pick<WorkflowTaskRecord, "stage">): PageKey {
  return pageForStage(task.stage);
}

export function pageForDraft(draft: Pick<WorkflowDraft, "stage">): PageKey {
  return pageForStage(draft.stage);
}

function pageForStage(stage: WorkflowTaskStage | null | undefined): PageKey {
  if (stage === "handoff") {
    return "prompts";
  }
  if (stage === "validation") {
    return "content";
  }
  return "render";
}

export function taskNextAction(task: WorkflowTaskRecord): TaskNextAction {
  if (task.stage === "handoff" || (!task.contentPath && (task.briefPath || task.promptPath))) {
    if (task.contentPath) {
      return {
        page: "content",
        label: "校验 AI 输出",
        detail: "内容已经保存，下一步检查协议和模板绑定。",
      };
    }
    if (task.promptPath) {
      return {
        page: "prompts",
        label: "回填 AI 输出",
        detail: "prompt 已生成，粘贴网页 AI 回复后保存内容。",
      };
    }
    return {
      page: "prompts",
      label: "生成交接 prompt",
      detail: "从 brief 和模板契约开始，生成给外部 AI 的交接包。",
    };
  }

  if (task.stage === "validation" || task.status === "error") {
    return {
      page: "content",
      label: task.status === "error" ? "处理校验问题" : "校验内容",
      detail: task.lastError ?? "检查 AI 输出是否符合模板协议。",
    };
  }

  if (task.status === "rendered") {
    return {
      page: "render",
      label: "重新生成",
      detail: "带入同一模板、内容和输出设置再次生成。",
    };
  }

  return {
    page: "render",
    label: task.outputPath ? "继续生成" : "设置输出并生成",
    detail: task.outputPath ? "配置已基本齐全，可先预检再生成。" : "补齐输出路径后即可生成 Office 文件。",
  };
}

export function taskReadiness(task: WorkflowTaskRecord): TaskReadinessItem[] {
  const items: TaskReadinessItem[] = [
    readinessItem("template", "模板", task.templatePath, task.templateAvailable, "模板路径已选择", "模板路径失效", "尚未选择模板"),
    readinessItem("content", "内容", task.contentPath, task.contentAvailable, "内容文件已选择", "内容文件待确认", "尚未保存内容"),
    outputReadiness(task),
  ];

  if (task.stage === "handoff" || task.briefPath || task.promptPath) {
    items.unshift(
      readinessItem("brief", "Brief", task.briefPath ?? "", true, "需求 brief 已选择", "brief 路径待确认", "尚未选择需求 brief"),
    );
    items.push(
      readinessItem("prompt", "Prompt", task.promptPath ?? "", true, "交接 prompt 已生成", "prompt 路径待确认", "尚未生成交接 prompt"),
    );
  }

  return items;
}

export function isTauriRuntime() {
  if (typeof window === "undefined") {
    return false;
  }
  const tauriWindow = window as Window & {
    __TAURI_INTERNALS__?: { invoke?: unknown };
  };
  return typeof tauriWindow.__TAURI_INTERNALS__?.invoke === "function";
}

function readinessItem(
  key: TaskReadinessItem["key"],
  label: string,
  path: string | null | undefined,
  available: boolean,
  readyDetail: string,
  warningDetail: string,
  missingDetail: string,
): TaskReadinessItem {
  if (!path?.trim()) {
    return { key, label, state: "missing", detail: missingDetail };
  }
  return {
    key,
    label,
    state: available ? "ready" : "warning",
    detail: available ? readyDetail : warningDetail,
  };
}

function outputReadiness(task: WorkflowTaskRecord): TaskReadinessItem {
  if (!task.outputPath?.trim()) {
    return {
      key: "output",
      label: "输出",
      state: "missing",
      detail: "尚未设置输出路径",
    };
  }
  if (task.status === "rendered" && !task.outputAvailable) {
    return {
      key: "output",
      label: "输出",
      state: "warning",
      detail: "记录存在，输出文件待确认",
    };
  }
  return {
    key: "output",
    label: "输出",
    state: "ready",
    detail: task.outputAvailable ? "输出文件可用" : "保存路径已设置",
  };
}

function inferTaskStatus(draft: WorkflowDraft): WorkflowTaskStatus {
  if (draft.lastError) {
    return "error";
  }
  if (draft.outputPath) {
    return "draft";
  }
  if (draft.contentPath) {
    return "content_ready";
  }
  return "draft";
}

function inferTaskStage(draft: WorkflowDraft): WorkflowTaskStage {
  if (draft.source === "template-library" || draft.source === "render-workflow") {
    return "render";
  }
  if (!draft.contentPath && (draft.briefPath || draft.promptPath)) {
    return "handoff";
  }
  if (draft.validationStatus && draft.validationStatus !== "pass") {
    return "validation";
  }
  return draft.contentPath ? "render" : "handoff";
}

function deriveTaskName(draft: WorkflowDraft) {
  for (const path of [draft.outputPath, draft.contentPath, draft.templatePath]) {
    const name = fileStem(path ?? "");
    if (name) {
      return name;
    }
  }
  return "未命名任务";
}

function fileStem(path: string) {
  const name = path.split(/[\\/]/).filter(Boolean).pop() ?? "";
  return name.replace(/\.(pptx|docx|xlsx|json|md|markdown)$/i, "").trim();
}

function createTaskId() {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }
  return `task-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;
}
