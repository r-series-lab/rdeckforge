import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AppShell } from "@/components/AppShell";
import { PromptPacksPage } from "@/pages/PromptPacksPage";
import { TemplateLibraryPage } from "@/pages/TemplateLibraryPage";
import { ContentPage } from "@/pages/ContentPage";
import { RenderPage, type RenderInspectorState } from "@/pages/RenderPage";
import { HistoryPage } from "@/pages/HistoryPage";
import type { PageKey } from "@/app-modules";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  readWorkflowDraft,
  writeWorkflowDraft,
  type WorkflowDraft,
} from "@/lib/workflow-draft";
import {
  ensureTaskIdentity,
  isTauriRuntime,
  pageForDraft,
  taskInputFromDraft,
} from "@/lib/workflow-tasks";
import { PageErrorBoundary } from "@/components/PageErrorBoundary";
import { SettingsDialog } from "@/components/SettingsDialog";
import { readTheme, saveTheme, type ThemeMode } from "@/lib/theme";

export function App() {
  const [activePage, setActivePage] = useState<PageKey>("render");
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [theme, setTheme] = useState<ThemeMode>(() => readTheme());
  const [workflowDraft, setWorkflowDraft] = useState<WorkflowDraft | null>(() =>
    readWorkflowDraft(),
  );
  const [renderInspector, setRenderInspector] = useState<RenderInspectorState>({
    templateName: "尚未选择",
    templatePath: "",
    inputPath: "",
    outputPath: "",
    status: "idle",
    message: "选择模板和内容后即可预检。",
  });
  useEffect(() => {
    saveTheme(theme);
  }, [theme]);

  const openWorkflowDraft = useCallback((draft: WorkflowDraft, page?: PageKey) => {
    const taskDraft = ensureTaskIdentity(draft);
    setWorkflowDraft(taskDraft);
    writeWorkflowDraft(taskDraft);
    if (isTauriRuntime()) {
      void invoke("save_workflow_task", { task: taskInputFromDraft(taskDraft) }).catch((error) => {
        console.error("failed to persist workflow task", error);
      });
    }
    if (page) {
      setActivePage(page);
    }
  }, []);

  const quickResume = workflowDraft
    ? {
        title:
          workflowDraft.taskName?.trim() ||
          fileName(workflowDraft.outputPath ?? "") ||
          fileName(workflowDraft.contentPath) ||
          "当前任务",
        detail: `${workflowStageLabel(workflowDraft.stage)} · ${
          workflowDraft.format?.toUpperCase() ?? "PPTX"
        }`,
        onOpen: () => openWorkflowDraft(workflowDraft, pageForDraft(workflowDraft)),
      }
    : null;

  return (
    <AppShell
      activePage={activePage}
      onPageChange={setActivePage}
      onOpenSettings={() => setSettingsOpen(true)}
      inspector={activePage === "render" ? <RenderInspector context={renderInspector} /> : null}
      quickResume={quickResume}
    >
      <PageErrorBoundary resetKey={activePage} onReset={() => setActivePage("render")}>
        {activePage === "prompts" ? (
          <PromptPacksPage initialDraft={workflowDraft} onWorkflowDraft={openWorkflowDraft} />
        ) : null}
        {activePage === "templates" ? (
          <TemplateLibraryPage onUseTemplate={(draft) => openWorkflowDraft(draft, "render")} />
        ) : null}
        {activePage === "content" ? (
          <ContentPage initialDraft={workflowDraft} onWorkflowDraft={openWorkflowDraft} />
        ) : null}
        {activePage === "render" ? (
          <RenderPage
            initialDraft={workflowDraft}
            onWorkflowDraft={openWorkflowDraft}
            onInspectorChange={setRenderInspector}
          />
        ) : null}
        {activePage === "history" ? (
          <HistoryPage onNavigate={setActivePage} onWorkflowDraft={openWorkflowDraft} />
        ) : null}
      </PageErrorBoundary>
      <SettingsDialog
        open={settingsOpen}
        theme={theme}
        onThemeChange={setTheme}
        onClose={() => setSettingsOpen(false)}
      />
    </AppShell>
  );
}

function RenderInspector({ context }: { context: RenderInspectorState }) {
  const statusTone =
    context.status === "ok"
      ? "is-ok"
      : context.status === "error"
        ? "is-danger"
        : context.status === "running"
          ? "is-warn"
          : "";

  return (
    <div className="inspector-stack">
      <Card className="inspector-card">
        <CardHeader>
          <CardTitle>当前任务</CardTitle>
        </CardHeader>
        <CardContent>
          <dl className="definition-list">
            <div>
              <dt>模板</dt>
              <dd>{context.templateName}</dd>
            </div>
            <div>
              <dt>内容文件</dt>
              <dd className="path-text">{fileName(context.inputPath) || "尚未选择"}</dd>
            </div>
            <div>
              <dt>输出文件</dt>
              <dd className="path-text">{fileName(context.outputPath) || "尚未设置"}</dd>
            </div>
            <div>
              <dt>输出格式</dt>
              <dd>{context.outputFormat?.toUpperCase() ?? "待选择"}</dd>
            </div>
            <div>
              <dt>状态</dt>
              <dd>
                <span className={`status-pill ${statusTone}`}>{renderStatusLabel(context.status)}</span>
              </dd>
            </div>
          </dl>
        </CardContent>
      </Card>
      <Card className="inspector-card">
        <CardHeader>
          <CardTitle>{context.outputFile ? "最新输出" : "预检摘要"}</CardTitle>
        </CardHeader>
        <CardContent>
          <p className="muted inspector-message">{context.message}</p>
          <dl className="definition-list inspector-summary-list">
            <div>
              <dt>模板</dt>
              <dd>{context.templatePath ? "已选择" : "待选择"}</dd>
            </div>
            <div>
              <dt>内容</dt>
              <dd>{context.inputPath ? context.contentFormat ?? "已选择" : "待选择"}</dd>
            </div>
            <div>
              <dt>输出</dt>
              <dd>
                {context.outputPath
                  ? context.outputFormat?.toUpperCase() ?? "路径可用"
                  : "待设置"}
              </dd>
            </div>
          </dl>
          <dl className="definition-list inspector-metrics">
            {typeof context.plannedPageCount === "number" ? (
              <div>
                <dt>计划页数</dt>
                <dd>{context.plannedPageCount}</dd>
              </div>
            ) : null}
            {typeof context.visibleSlideCount === "number" ? (
              <div>
                <dt>实际页数</dt>
                <dd>{context.visibleSlideCount}</dd>
              </div>
            ) : null}
            {typeof context.checkedBindings === "number" ? (
              <div>
                <dt>内容绑定</dt>
                <dd>
                  {context.checkedBindings} 项，缺失 {context.missingBindings ?? 0} 项
                </dd>
              </div>
            ) : null}
            {typeof context.sheetCount === "number" ? (
              <div>
                <dt>工作表</dt>
                <dd>{context.sheetCount}</dd>
              </div>
            ) : null}
            {context.integrityStatus === "passed" ? (
              <div>
                <dt>文件完整性</dt>
                <dd>
                  已通过
                  {typeof context.integrityEntryCount === "number"
                    ? ` · ${context.integrityEntryCount} 个部件`
                    : ""}
                </dd>
              </div>
            ) : null}
          </dl>
        </CardContent>
      </Card>
      <Card className="inspector-card inspector-note">
        <CardHeader>
          <CardTitle>任务信息</CardTitle>
        </CardHeader>
        <CardContent>
          <dl className="definition-list">
            <div>
              <dt>渲染方式</dt>
              <dd>{context.rendererLabel ?? "等待模板"}</dd>
            </div>
            <div>
              <dt>模板库</dt>
              <dd>{context.templateCount ?? 0} 个已链接模板包</dd>
            </div>
            <div>
              <dt>数据边界</dt>
              <dd>仅在本机处理</dd>
            </div>
          </dl>
        </CardContent>
      </Card>
    </div>
  );
}

function fileName(path: string) {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? "";
}

function renderStatusLabel(status: RenderInspectorState["status"]) {
  if (status === "running") return "运行中";
  if (status === "ok") return "已完成";
  if (status === "error") return "需要处理";
  return "就绪";
}

function workflowStageLabel(stage: WorkflowDraft["stage"]) {
  if (stage === "handoff") return "AI 交接";
  if (stage === "validation") return "输出校验";
  return "生成";
}
