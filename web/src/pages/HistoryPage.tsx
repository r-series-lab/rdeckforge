import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Copy,
  ExternalLink,
  FolderOpen,
  PlayCircle,
  RefreshCw,
  RotateCw,
  Search,
  Trash2,
  X,
} from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { paginate } from "@/lib/pagination";
import { Pagination } from "@/components/Pagination";
import type { PageKey } from "@/app-modules";
import type { WorkflowDraft } from "@/lib/workflow-draft";
import {
  draftFromTask,
  duplicateTaskDraft,
  isTauriRuntime,
  pageForTask,
  taskNextAction,
  taskInputFromDraft,
  taskReadiness,
  type WorkflowTaskRecord,
} from "@/lib/workflow-tasks";

const PAGE_SIZE = 7;

type RenderHistoryRecord = {
  id: string;
  jobId: string;
  createdAt: string;
  renderedAt: string;
  format: string;
  deckRecipe?: string | null;
  templateDir: string;
  contentFile: string;
  markdownFile?: string | null;
  outputFile: string;
  jobFile: string;
  sidecarFile: string;
  plannedPageCount: number;
  visibleSlideCount?: number | null;
  contentCheckStatus?: string | null;
  checkedBindings?: number | null;
  missingBindings: number;
  warnings: string[];
};

type RunState = {
  status: "idle" | "running" | "ok" | "error";
  message: string;
};

type TaskFilter = "all" | "active" | "rendered" | "error";

type HistoryPageProps = {
  onNavigate?: (page: PageKey) => void;
  onWorkflowDraft?: (draft: WorkflowDraft, page?: PageKey) => void;
};

export function HistoryPage({ onNavigate, onWorkflowDraft }: HistoryPageProps) {
  const [view, setView] = useState<"tasks" | "history">("tasks");
  const [query, setQuery] = useState("");
  const [taskFilter, setTaskFilter] = useState<TaskFilter>("all");
  const [tasks, setTasks] = useState<WorkflowTaskRecord[]>([]);
  const [history, setHistory] = useState<RenderHistoryRecord[]>([]);
  const [selectedTaskId, setSelectedTaskId] = useState("");
  const [currentPage, setCurrentPage] = useState(1);
  const listRef = useRef<HTMLDivElement>(null);
  const [runState, setRunState] = useState<RunState>({
    status: "idle",
    message: "任务和生成记录只保存在本机。",
  });
  const isRunning = runState.status === "running";
  const normalizedQuery = query.trim().toLowerCase();
  const filteredTasks = useMemo(
    () =>
      tasks.filter(
        (task) =>
          taskMatchesFilter(task, taskFilter) &&
          taskMatchesQuery(task, normalizedQuery),
      ),
    [normalizedQuery, taskFilter, tasks],
  );
  const filteredHistory = useMemo(
    () => history.filter((record) => historyMatchesQuery(record, normalizedQuery)),
    [history, normalizedQuery],
  );
  const activeItems = view === "tasks" ? filteredTasks : filteredHistory;
  const sourceItemCount = view === "tasks" ? tasks.length : history.length;
  const hasFilters = normalizedQuery.length > 0 || (view === "tasks" && taskFilter !== "all");
  const taskPagination = useMemo(
    () => paginate(filteredTasks, currentPage, PAGE_SIZE),
    [currentPage, filteredTasks],
  );
  const historyPagination = useMemo(
    () => paginate(filteredHistory, currentPage, PAGE_SIZE),
    [currentPage, filteredHistory],
  );
  const pagination = view === "tasks" ? taskPagination : historyPagination;
  const taskSummary = useMemo(
    () => ({
      active: tasks.filter((task) => task.status !== "rendered" && task.status !== "error").length,
      rendered: tasks.filter((task) => task.status === "rendered").length,
      errors: tasks.filter((task) => task.status === "error").length,
    }),
    [tasks],
  );
  const selectedTask = useMemo(
    () => filteredTasks.find((task) => task.id === selectedTaskId) ?? null,
    [filteredTasks, selectedTaskId],
  );

  useEffect(() => {
    void loadData();
  }, []);

  useEffect(() => {
    setCurrentPage(1);
  }, [normalizedQuery, taskFilter, view]);

  useEffect(() => {
    if (pagination.page !== currentPage) {
      setCurrentPage(pagination.page);
    }
  }, [currentPage, pagination.page]);

  useEffect(() => {
    if (listRef.current) {
      listRef.current.scrollTop = 0;
    }
  }, [pagination.page, view]);

  useEffect(() => {
    if (view !== "tasks") {
      return;
    }
    if (selectedTaskId && filteredTasks.some((task) => task.id === selectedTaskId)) {
      return;
    }
    setSelectedTaskId(filteredTasks[0]?.id ?? "");
  }, [filteredTasks, selectedTaskId, view]);

  async function loadData() {
    if (!isTauriRuntime()) {
      setTasks(browserPreviewTasks);
      setRunState({ status: "idle", message: "桌面应用中会显示本机任务和生成记录。" });
      return;
    }
    await runAction("正在读取任务中心", async () => {
      const [taskRecords, historyRecords] = await Promise.all([
        invoke<WorkflowTaskRecord[]>("list_workflow_tasks", { limit: 50 }),
        invoke<RenderHistoryRecord[]>("list_render_history", { limit: 50 }),
      ]);
      setTasks(taskRecords);
      setHistory(historyRecords);
      setCurrentPage(1);
      return `已载入 ${taskRecords.length} 个任务、${historyRecords.length} 条生成记录`;
    });
  }

  function continueTask(task: WorkflowTaskRecord) {
    onWorkflowDraft?.(draftFromTask(task), pageForTask(task));
  }

  async function duplicateTask(task: WorkflowTaskRecord) {
    await runAction("正在复制任务", async () => {
      const draft = duplicateTaskDraft(task);
      if (!isTauriRuntime()) {
        setTasks((current) => [
          {
            ...task,
            id: draft.taskId!,
            name: draft.taskName!,
            status: draft.status!,
            validationStatus: null,
            updatedAt: new Date().toISOString(),
          },
          ...current,
        ]);
        return `已创建“${draft.taskName}”`;
      }
      await invoke("save_workflow_task", { task: taskInputFromDraft(draft) });
      const records = await invoke<WorkflowTaskRecord[]>("list_workflow_tasks", { limit: 50 });
      setTasks(records);
      setCurrentPage(1);
      return `已创建“${draft.taskName}”`;
    });
  }

  async function deleteTask(task: WorkflowTaskRecord) {
    if (!window.confirm(`删除任务“${task.name}”？\n不会删除模板、内容文件或输出文件。`)) {
      return;
    }
    await runAction("正在删除任务", async () => {
      if (!isTauriRuntime()) {
        setTasks((current) => current.filter((item) => item.id !== task.id));
        return "任务记录已删除，原文件保持不变";
      }
      await invoke("delete_workflow_task", { id: task.id });
      setTasks((current) => current.filter((item) => item.id !== task.id));
      return "任务记录已删除，原文件保持不变";
    });
  }

  async function rerender(record: RenderHistoryRecord) {
    await runAction("正在重新生成", async () => {
      await invoke("rerender_history", { id: record.id });
      const records = await invoke<RenderHistoryRecord[]>("list_render_history", { limit: 50 });
      setHistory(records);
      setCurrentPage(1);
      return `已重新生成 ${record.format.toUpperCase()} 并写入历史`;
    });
  }

  async function openOutput(record: RenderHistoryRecord) {
    await runAction("正在打开文件", async () => {
      await invoke("open_path", { path: record.outputFile });
      return "文件已打开";
    });
  }

  async function revealOutput(record: RenderHistoryRecord) {
    await runAction("正在定位文件", async () => {
      await invoke("reveal_path", { path: record.outputFile });
      return "已在文件管理器中定位";
    });
  }

  async function copyOutputPath(record: RenderHistoryRecord) {
    await runAction("正在复制路径", async () => {
      if (!navigator.clipboard?.writeText) {
        throw new Error("当前 WebView 不支持剪贴板。");
      }
      await navigator.clipboard.writeText(record.outputFile);
      return "输出路径已复制";
    });
  }

  function clearFilters() {
    setQuery("");
    setTaskFilter("all");
  }

  async function runAction(label: string, action: () => Promise<string>) {
    setRunState({ status: "running", message: label });
    try {
      const message = await action();
      setRunState({ status: "ok", message });
    } catch (error) {
      setRunState({
        status: "error",
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }

  return (
    <div className="page-grid workspace-fill-grid history-page-grid">
      <Card className="span-8 task-center-card">
        <CardHeader>
          <CardTitle>任务中心</CardTitle>
          <p className="muted">继续未完成的文档流程，或查看已经生成的文件。</p>
        </CardHeader>
        <CardContent>
          <div className="history-toolbar">
            <div className="task-view-tabs" role="tablist" aria-label="任务中心视图">
              <button
                type="button"
                className={view === "tasks" ? "is-active" : ""}
                onClick={() => setView("tasks")}
              >
                文档任务 <span>{tasks.length}</span>
              </button>
              <button
                type="button"
                className={view === "history" ? "is-active" : ""}
                onClick={() => setView("history")}
              >
                生成历史 <span>{history.length}</span>
              </button>
            </div>
            <label className="task-search">
              <Search size={14} aria-hidden="true" />
              <Input
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                placeholder={view === "tasks" ? "搜索任务、内容或模板路径" : "搜索输出、模板或配方"}
                aria-label={view === "tasks" ? "搜索文档任务" : "搜索生成历史"}
              />
              {query ? (
                <button type="button" onClick={() => setQuery("")} aria-label="清空搜索">
                  <X size={13} aria-hidden="true" />
                </button>
              ) : null}
            </label>
            {view === "tasks" ? (
              <div className="task-filter-tabs" role="group" aria-label="任务状态筛选">
                <button
                  type="button"
                  className={taskFilter === "all" ? "is-active" : ""}
                  onClick={() => setTaskFilter("all")}
                >
                  全部
                </button>
                <button
                  type="button"
                  className={taskFilter === "active" ? "is-active" : ""}
                  onClick={() => setTaskFilter("active")}
                >
                  进行中
                </button>
                <button
                  type="button"
                  className={taskFilter === "rendered" ? "is-active" : ""}
                  onClick={() => setTaskFilter("rendered")}
                >
                  已生成
                </button>
                <button
                  type="button"
                  className={taskFilter === "error" ? "is-active" : ""}
                  onClick={() => setTaskFilter("error")}
                >
                  需处理
                </button>
              </div>
            ) : null}
            <span className={`status-pill ${statusClass(runState.status)}`}>
              {statusLabel(runState.status)}
            </span>
            <span className="task-center-message">{runState.message}</span>
            <Button size="sm" onClick={loadData} disabled={isRunning}>
              <RefreshCw size={14} aria-hidden="true" />
              刷新
            </Button>
          </div>

          {activeItems.length === 0 ? (
            <div className="empty-panel compact-empty">
              <strong>
                {hasFilters
                  ? "没有匹配记录"
                  : view === "tasks"
                    ? "还没有文档任务"
                    : "还没有生成记录"}
              </strong>
              <span>
                {hasFilters
                  ? `当前筛选命中 0 / ${sourceItemCount} 条。`
                  : view === "tasks"
                    ? "从 AI 交接、输出校验或生成页开始后会自动保存任务。"
                    : "生成一次 PPTX、DOCX 或 XLSX 后会自动记录在这里。"}
              </span>
              {hasFilters ? (
                <div className="task-empty-actions">
                  <Button size="sm" onClick={clearFilters}>
                    清除筛选
                  </Button>
                </div>
              ) : view === "tasks" ? (
                <div className="task-empty-actions">
                  <Button size="sm" variant="primary" onClick={() => onNavigate?.("prompts")}>
                    AI 交接
                  </Button>
                  <Button size="sm" onClick={() => onNavigate?.("templates")}>
                    选择模板
                  </Button>
                  <Button size="sm" onClick={() => onNavigate?.("render")}>
                    开始生成
                  </Button>
                </div>
              ) : null}
            </div>
          ) : (
            <>
              <div className="history-list task-center-list" ref={listRef}>
                {view === "tasks"
                  ? taskPagination.items.map((task) => (
                      <TaskRow
                        key={task.id}
                        task={task}
                        selected={selectedTaskId === task.id}
                        disabled={isRunning}
                        onSelect={() => setSelectedTaskId(task.id)}
                        onContinue={continueTask}
                        onDuplicate={duplicateTask}
                        onDelete={deleteTask}
                      />
                    ))
                  : historyPagination.items.map((record) => (
                      <HistoryRow
                        key={record.id}
                        record={record}
                        disabled={isRunning}
                        onRerender={rerender}
                        onOpen={openOutput}
                        onReveal={revealOutput}
                        onCopy={copyOutputPath}
                      />
                    ))}
              </div>
              <Pagination
                label={view === "tasks" ? "文档任务分页" : "生成历史分页"}
                page={pagination.page}
                totalPages={pagination.totalPages}
                startItem={pagination.startItem}
                endItem={pagination.endItem}
                totalItems={pagination.totalItems}
                onPageChange={setCurrentPage}
              />
            </>
          )}
        </CardContent>
      </Card>

      <Card className="span-4">
        <CardHeader>
          <CardTitle>{selectedTask ? "任务详情" : "任务概览"}</CardTitle>
        </CardHeader>
        <CardContent>
          {selectedTask ? (
            <TaskDetail
              task={selectedTask}
              summary={taskSummary}
              historyCount={history.length}
              disabled={isRunning}
              onContinue={continueTask}
              onDuplicate={duplicateTask}
              onDelete={deleteTask}
            />
          ) : (
            <TaskOverview summary={taskSummary} historyCount={history.length} />
          )}
        </CardContent>
      </Card>
    </div>
  );
}

const browserPreviewTasks: WorkflowTaskRecord[] = [
  {
    id: "preview-task-1",
    name: "护理质量改进教学",
    status: "validated",
    stage: "render",
    source: "content-validation",
    templatePath: "/preview/templates/pptx-cards",
    contentPath: "/Documents/content/nursing-quality.json",
    outputPath: "/Documents/output/nursing-quality.pptx",
    format: "pptx",
    recipe: "teaching_deck",
    validationStatus: "pass",
    templateAvailable: true,
    contentAvailable: true,
    outputAvailable: false,
    createdAt: "2026-07-08T01:00:00Z",
    updatedAt: "2026-07-08T02:30:00Z",
  },
  {
    id: "preview-task-2",
    name: "前端概要设计",
    status: "content_ready",
    stage: "validation",
    source: "prompt-ai-output",
    templatePath: "/preview/templates/docx-template",
    contentPath: "/Documents/content/frontend-design.md",
    outputPath: "/Documents/output/frontend-design.docx",
    format: "docx",
    recipe: "standard_document",
    validationStatus: null,
    templateAvailable: true,
    contentAvailable: true,
    outputAvailable: false,
    createdAt: "2026-07-08T00:30:00Z",
    updatedAt: "2026-07-08T01:45:00Z",
  },
];

function TaskRow({
  task,
  selected,
  disabled,
  onSelect,
  onContinue,
  onDuplicate,
  onDelete,
}: {
  task: WorkflowTaskRecord;
  selected: boolean;
  disabled: boolean;
  onSelect: () => void;
  onContinue: (task: WorkflowTaskRecord) => void;
  onDuplicate: (task: WorkflowTaskRecord) => void;
  onDelete: (task: WorkflowTaskRecord) => void;
}) {
  return (
    <article
      className={`history-row task-row${selected ? " is-selected" : ""}`}
      tabIndex={0}
      onClick={onSelect}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          onSelect();
        }
      }}
    >
      <div className="history-row-main">
        <div className="history-row-title">
          <strong>{task.name}</strong>
          <span className={`status-pill ${taskStatusClass(task.status)}`}>
            {taskStatusLabel(task.status)}
          </span>
          <span>{task.format?.toUpperCase() ?? "待选择"}</span>
          <span>{formatDate(task.updatedAt)}</span>
        </div>
        <dl className="definition-list compact task-path-list">
          <div>
            <dt>内容</dt>
            <dd className="path-text">{task.contentPath || "尚未选择"}</dd>
          </div>
          <div>
            <dt>模板</dt>
            <dd className="path-text">{task.templatePath || "尚未选择"}</dd>
          </div>
          {task.outputPath ? (
            <div>
              <dt>输出</dt>
              <dd className="path-text">{task.outputPath}</dd>
            </div>
          ) : null}
        </dl>
      </div>
      <div className="history-row-side task-row-side">
        <div className="task-state-stack">
          <span className="status-pill">{taskStageLabel(task.stage)}</span>
          <span className="history-meta">模板 {task.templateAvailable ? "可用" : "待确认"}</span>
          <span className="history-meta">内容 {task.contentAvailable ? "可用" : "待确认"}</span>
        </div>
        <div className="history-actions">
          <Button size="sm" variant="primary" onClick={() => onContinue(task)} disabled={disabled}>
            <PlayCircle size={14} aria-hidden="true" />
            继续任务
          </Button>
          <Button size="sm" onClick={() => onDuplicate(task)} disabled={disabled}>
            <Copy size={14} aria-hidden="true" />
            复制
          </Button>
          <Button size="sm" variant="ghost" onClick={() => onDelete(task)} disabled={disabled}>
            <Trash2 size={14} aria-hidden="true" />
            删除
          </Button>
        </div>
      </div>
      {task.lastError ? <p className="task-row-error">{task.lastError}</p> : null}
    </article>
  );
}

function TaskDetail({
  task,
  summary,
  historyCount,
  disabled,
  onContinue,
  onDuplicate,
  onDelete,
}: {
  task: WorkflowTaskRecord;
  summary: { active: number; rendered: number; errors: number };
  historyCount: number;
  disabled: boolean;
  onContinue: (task: WorkflowTaskRecord) => void;
  onDuplicate: (task: WorkflowTaskRecord) => void;
  onDelete: (task: WorkflowTaskRecord) => void;
}) {
  const action = taskNextAction(task);
  const readiness = taskReadiness(task);
  const readyCount = readiness.filter((item) => item.state === "ready").length;

  return (
    <div className="task-detail-panel">
      <section className="task-next-panel">
        <span className={`status-pill ${taskStatusClass(task.status)}`}>
          {taskStatusLabel(task.status)}
        </span>
        <strong>{action.label}</strong>
        <p>{action.detail}</p>
        <Button variant="primary" onClick={() => onContinue(task)} disabled={disabled}>
          <PlayCircle size={14} aria-hidden="true" />
          继续到{pageLabel(action.page)}
        </Button>
      </section>

      <dl className="definition-list task-detail-meta">
        <div>
          <dt>阶段</dt>
          <dd>{taskStageLabel(task.stage)}</dd>
        </div>
        <div>
          <dt>格式</dt>
          <dd>{task.format?.toUpperCase() ?? "待选择"}</dd>
        </div>
        <div>
          <dt>更新</dt>
          <dd>{formatDate(task.updatedAt)}</dd>
        </div>
        <div>
          <dt>结构</dt>
          <dd>{task.recipe || "模板默认"}</dd>
        </div>
      </dl>

      <section className="task-readiness-panel" aria-label="任务就绪度">
        <header>
          <strong>就绪清单</strong>
          <span>
            {readyCount}/{readiness.length}
          </span>
        </header>
        <div className="task-readiness-list">
          {readiness.map((item) => (
            <div className={`task-readiness-item is-${item.state}`} key={item.key}>
              <span aria-hidden="true" />
              <div>
                <strong>{item.label}</strong>
                <small>{item.detail}</small>
              </div>
            </div>
          ))}
        </div>
      </section>

      <dl className="definition-list compact task-detail-paths">
        <div>
          <dt>模板</dt>
          <dd className="path-text">{task.templatePath || "尚未选择"}</dd>
        </div>
        <div>
          <dt>内容</dt>
          <dd className="path-text">{task.contentPath || "尚未保存"}</dd>
        </div>
        <div>
          <dt>输出</dt>
          <dd className="path-text">{task.outputPath || "尚未设置"}</dd>
        </div>
      </dl>

      <div className="task-detail-actions">
        <Button size="sm" onClick={() => onDuplicate(task)} disabled={disabled}>
          <Copy size={14} aria-hidden="true" />
          复制任务
        </Button>
        <Button size="sm" variant="ghost" onClick={() => onDelete(task)} disabled={disabled}>
          <Trash2 size={14} aria-hidden="true" />
          删除记录
        </Button>
      </div>

      <TaskOverview summary={summary} historyCount={historyCount} compact />
    </div>
  );
}

function TaskOverview({
  summary,
  historyCount,
  compact = false,
}: {
  summary: { active: number; rendered: number; errors: number };
  historyCount: number;
  compact?: boolean;
}) {
  return (
    <dl className={`definition-list${compact ? " task-overview-compact" : ""}`}>
      <div>
        <dt>进行中</dt>
        <dd>{summary.active} 个</dd>
      </div>
      <div>
        <dt>已生成</dt>
        <dd>{summary.rendered} 个</dd>
      </div>
      <div>
        <dt>需处理</dt>
        <dd>{summary.errors} 个</dd>
      </div>
      <div>
        <dt>生成记录</dt>
        <dd>{historyCount} 条</dd>
      </div>
      <div>
        <dt>数据边界</dt>
        <dd>只保存路径和状态，不复制模板与业务内容。</dd>
      </div>
    </dl>
  );
}

function HistoryRow({
  record,
  disabled,
  onRerender,
  onOpen,
  onReveal,
  onCopy,
}: {
  record: RenderHistoryRecord;
  disabled: boolean;
  onRerender: (record: RenderHistoryRecord) => void;
  onOpen: (record: RenderHistoryRecord) => void;
  onReveal: (record: RenderHistoryRecord) => void;
  onCopy: (record: RenderHistoryRecord) => void;
}) {
  return (
    <article className="history-row">
      <div className="history-row-main">
        <div className="history-row-title">
          <strong>{record.deckRecipe ?? "手动结构"}</strong>
          <span className="status-pill">{record.format.toUpperCase()}</span>
          <span>{formatDate(record.renderedAt)}</span>
        </div>
        <dl className="definition-list compact">
          <div>
            <dt>输出</dt>
            <dd className="path-text">{record.outputFile}</dd>
          </div>
          <div>
            <dt>模板</dt>
            <dd className="path-text">{record.templateDir}</dd>
          </div>
        </dl>
      </div>
      <div className="history-row-side">
        <span className={`status-pill ${record.contentCheckStatus === "passed" ? "is-ok" : "is-warn"}`}>
          {formatCheckStatus(record.contentCheckStatus)}
        </span>
        <span className="history-meta">{formatOutputMetrics(record)}</span>
        <span className="history-meta">
          检查 {record.checkedBindings ?? "-"} / 缺失 {record.missingBindings}
        </span>
      </div>
      <div className="history-actions">
        <Button size="sm" variant="primary" onClick={() => onRerender(record)} disabled={disabled}>
          <RotateCw size={14} aria-hidden="true" />
          重新生成
        </Button>
        <Button size="sm" onClick={() => onOpen(record)} disabled={disabled}>
          <ExternalLink size={14} aria-hidden="true" />
          打开
        </Button>
        <Button size="sm" onClick={() => onReveal(record)} disabled={disabled}>
          <FolderOpen size={14} aria-hidden="true" />
          定位
        </Button>
        <Button size="sm" onClick={() => onCopy(record)} disabled={disabled}>
          <Copy size={14} aria-hidden="true" />
          复制路径
        </Button>
      </div>
    </article>
  );
}

function taskMatchesFilter(task: WorkflowTaskRecord, filter: TaskFilter) {
  if (filter === "rendered") return task.status === "rendered";
  if (filter === "error") return task.status === "error";
  if (filter === "active") return task.status !== "rendered" && task.status !== "error";
  return true;
}

function taskMatchesQuery(task: WorkflowTaskRecord, query: string) {
  if (!query) return true;
  return includesQuery(
    [
      task.name,
      task.status,
      task.stage,
      task.source,
      task.templatePath,
      task.contentPath,
      task.outputPath,
      task.format,
      task.recipe,
      task.briefPath,
      task.promptPath,
      task.validationStatus,
      task.lastError,
    ],
    query,
  );
}

function historyMatchesQuery(record: RenderHistoryRecord, query: string) {
  if (!query) return true;
  return includesQuery(
    [
      record.id,
      record.jobId,
      record.format,
      record.deckRecipe,
      record.templateDir,
      record.contentFile,
      record.markdownFile,
      record.outputFile,
      record.jobFile,
      record.sidecarFile,
      record.contentCheckStatus,
      ...record.warnings,
    ],
    query,
  );
}

function includesQuery(values: Array<string | number | null | undefined>, query: string) {
  return values.some((value) => String(value ?? "").toLowerCase().includes(query));
}

function formatDate(value: string) {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString();
}

function statusLabel(status: RunState["status"]) {
  if (status === "running") return "运行中";
  if (status === "ok") return "完成";
  if (status === "error") return "错误";
  return "就绪";
}

function statusClass(status: RunState["status"]) {
  if (status === "ok") return "is-ok";
  if (status === "error") return "is-danger";
  if (status === "running") return "is-warn";
  return "";
}

function taskStatusLabel(status: WorkflowTaskRecord["status"]) {
  if (status === "content_ready") return "内容就绪";
  if (status === "validated") return "已校验";
  if (status === "rendered") return "已生成";
  if (status === "error") return "需处理";
  return "草稿";
}

function taskStatusClass(status: WorkflowTaskRecord["status"]) {
  if (status === "rendered" || status === "validated") return "is-ok";
  if (status === "error") return "is-danger";
  if (status === "content_ready") return "is-warn";
  return "";
}

function taskStageLabel(stage: WorkflowTaskRecord["stage"]) {
  if (stage === "handoff") return "AI 交接";
  if (stage === "validation") return "输出校验";
  return "生成";
}

function pageLabel(page: PageKey) {
  if (page === "prompts") return "交接";
  if (page === "content") return "校验";
  if (page === "templates") return "模板";
  if (page === "history") return "任务";
  return "生成";
}

function formatCheckStatus(status?: string | null) {
  if (status === "passed") return "通过";
  if (status === "failed") return "失败";
  if (status === "passed_with_warnings") return "有提示";
  return "未检查";
}

function formatOutputMetrics(record: RenderHistoryRecord) {
  if (record.format === "pptx") {
    return `计划 ${record.plannedPageCount} / 可见 ${record.visibleSlideCount ?? "-"}`;
  }
  if (record.format === "xlsx") {
    return `工作表 ${record.visibleSlideCount ?? record.plannedPageCount ?? "-"}`;
  }
  return `结构 ${record.plannedPageCount || "-"}`;
}
