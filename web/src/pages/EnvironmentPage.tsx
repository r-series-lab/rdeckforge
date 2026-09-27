import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Copy, Trash2 } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import {
  clearLastUiError,
  formatUiError,
  readLastUiError,
  sourceLabel,
  UI_ERROR_EVENT,
  type UiErrorRecord,
} from "@/lib/ui-error-log";

type DiagnosticStatus = "pass" | "warn" | "fail" | "info";

type EnvironmentComponent = {
  id: string;
  label: string;
  status: DiagnosticStatus;
  required: boolean;
  detected: boolean;
  version?: string | null;
  path?: string | null;
  message: string;
  fixHint?: string | null;
};

type EnvironmentDiagnostics = {
  productName: string;
  version: string;
  checkedAt: string;
  platform: string;
  architecture: string;
  readiness: DiagnosticStatus;
  summary: string;
  components: EnvironmentComponent[];
  notes: string[];
};

type RunState = {
  status: "idle" | "running" | "ok" | "error";
  message: string;
};

export function EnvironmentPage({ embedded = false }: { embedded?: boolean }) {
  const [diagnostics, setDiagnostics] = useState<EnvironmentDiagnostics | null>(null);
  const [lastUiError, setLastUiError] = useState<UiErrorRecord | null>(() => readLastUiError());
  const [runState, setRunState] = useState<RunState>({
    status: "idle",
    message: "点击重新体检可刷新本机依赖状态。",
  });

  useEffect(() => {
    void loadDiagnostics();
  }, []);

  useEffect(() => {
    const refreshUiError = () => setLastUiError(readLastUiError());
    window.addEventListener(UI_ERROR_EVENT, refreshUiError);
    window.addEventListener("storage", refreshUiError);
    return () => {
      window.removeEventListener(UI_ERROR_EVENT, refreshUiError);
      window.removeEventListener("storage", refreshUiError);
    };
  }, []);

  async function loadDiagnostics() {
    setRunState({ status: "running", message: "正在检查本机依赖" });
    try {
      const data = await invoke<EnvironmentDiagnostics>("environment_diagnostics");
      setDiagnostics(data);
      setRunState({ status: "ok", message: data.summary });
    } catch (error) {
      setRunState({
        status: "error",
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }

  async function copyUiError() {
    if (!lastUiError) {
      return;
    }
    try {
      await navigator.clipboard.writeText(formatUiError(lastUiError));
      setRunState({ status: "ok", message: "最近一次界面异常已复制。" });
    } catch (error) {
      setRunState({
        status: "error",
        message: error instanceof Error ? error.message : "复制失败，请展开详情后手动选择。",
      });
    }
  }

  function clearUiError() {
    clearLastUiError();
    setLastUiError(null);
    setRunState({ status: "ok", message: "界面异常记录已清除。" });
  }

  const blockingCount =
    diagnostics?.components.filter((item) => item.required && item.status === "fail").length ?? 0;
  const optionalWarningCount =
    diagnostics?.components.filter(
      (item) => !item.required && (item.status === "warn" || item.status === "fail"),
    ).length ?? 0;

  return (
    <div
      className={`page-grid workspace-fill-grid environment-page${embedded ? " is-embedded" : ""}`}
    >
      <Card className="span-8">
        <CardHeader>
          <CardTitle>环境体检</CardTitle>
          <p className="muted">检查安装后能否完成模板管理、校验和 Office 生成。</p>
        </CardHeader>
        <CardContent>
          {diagnostics ? (
            <>
              <div className="acceptance-summary">
                <span className={`status-pill ${statusClass(diagnostics.readiness)}`}>
                  {statusLabel(diagnostics.readiness)}
                </span>
                <strong>{diagnostics.summary}</strong>
                <span>
                  {diagnostics.productName} {diagnostics.version} / {diagnostics.platform}{" "}
                  {diagnostics.architecture}
                </span>
              </div>
              <div className="selection-summary">
                <div>
                  <span>必需阻断</span>
                  <strong>{blockingCount}</strong>
                </div>
                <div>
                  <span>可选提醒</span>
                  <strong>{optionalWarningCount}</strong>
                </div>
              </div>
              <div className="action-row">
                <Button variant="primary" onClick={loadDiagnostics} disabled={runState.status === "running"}>
                  重新体检
                </Button>
              </div>
              <div className="template-list">
                {diagnostics.components.map((component) => (
                  <div className="template-record" key={component.id}>
                    <div className="template-record-header">
                      <div>
                        <strong>{component.label}</strong>
                        <span>{component.required ? "必需" : "可选"}</span>
                      </div>
                      <span className={`status-pill ${statusClass(component.status)}`}>
                        {statusLabel(component.status)}
                      </span>
                    </div>
                    <dl className="definition-list compact">
                      <div>
                        <dt>状态</dt>
                        <dd>{component.detected ? "已检测到" : "未检测到或无需检测"}</dd>
                      </div>
                      {component.version ? (
                        <div>
                          <dt>版本</dt>
                          <dd>{component.version}</dd>
                        </div>
                      ) : null}
                      {component.path ? (
                        <div>
                          <dt>路径</dt>
                          <dd className="path-text">{component.path}</dd>
                        </div>
                      ) : null}
                      <div>
                        <dt>说明</dt>
                        <dd>{component.message}</dd>
                      </div>
                      {component.fixHint ? (
                        <div>
                          <dt>处理建议</dt>
                          <dd>{component.fixHint}</dd>
                        </div>
                      ) : null}
                    </dl>
                  </div>
                ))}
              </div>
            </>
          ) : (
            <div className="empty-panel compact-empty">
              <strong>还没有体检结果</strong>
              <span>正在读取环境状态。</span>
            </div>
          )}
          <div className="status-line">
            <span className={`status-pill ${runStatusClass(runState.status)}`}>
              {runStatusLabel(runState.status)}
            </span>
            <span>{runState.message}</span>
          </div>
        </CardContent>
      </Card>
      <Card className="span-4">
        <CardHeader>
          <CardTitle>依赖边界</CardTitle>
        </CardHeader>
        <CardContent>
          <dl className="definition-list">
            <div>
              <dt>无外部 AI</dt>
              <dd>应用只生成 prompt、校验内容并渲染文件。</dd>
            </div>
            <div>
              <dt>PPTX</dt>
              <dd>声明式渲染当前需要 Node sidecar；脚本型模板依赖自己的 runtime。</dd>
            </div>
            <div>
              <dt>DOCX / XLSX</dt>
              <dd>内置 OpenXML 写入路径优先，不要求安装 Office 才能生成。</dd>
            </div>
          </dl>
          {diagnostics?.notes.length ? (
            <ul className="check-list">
              {diagnostics.notes.map((note) => (
                <li key={note}>{note}</li>
              ))}
            </ul>
          ) : null}
          <section className="environment-section" aria-labelledby="ui-stability-title">
            <div className="environment-section-header">
              <div>
                <strong id="ui-stability-title">界面稳定性</strong>
                <span>仅在本机保留最近一次界面异常。</span>
              </div>
              <span className={`status-pill ${lastUiError ? "is-danger" : "is-ok"}`}>
                {lastUiError ? "最近异常" : "正常"}
              </span>
            </div>
            {lastUiError ? (
              <div className="ui-error-record">
                <dl className="definition-list compact">
                  <div>
                    <dt>时间</dt>
                    <dd>{formatLocalTime(lastUiError.occurredAt)}</dd>
                  </div>
                  <div>
                    <dt>来源</dt>
                    <dd>{sourceLabel(lastUiError.source)}</dd>
                  </div>
                  <div>
                    <dt>信息</dt>
                    <dd className="path-text">{lastUiError.message}</dd>
                  </div>
                </dl>
                {lastUiError.stack || lastUiError.componentStack ? (
                  <details className="compact-disclosure">
                    <summary>查看技术详情</summary>
                    <pre className="result-preview">{formatUiError(lastUiError)}</pre>
                  </details>
                ) : null}
                <div className="action-row compact-actions">
                  <Button size="sm" onClick={() => void copyUiError()}>
                    <Copy size={14} />
                    复制详情
                  </Button>
                  <Button size="sm" variant="ghost" onClick={clearUiError}>
                    <Trash2 size={14} />
                    清除记录
                  </Button>
                </div>
              </div>
            ) : (
              <p className="muted environment-empty-state">未记录到界面异常。</p>
            )}
          </section>
        </CardContent>
      </Card>
    </div>
  );
}

function formatLocalTime(value: string) {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString("zh-CN", { hour12: false });
}

function statusClass(status: DiagnosticStatus) {
  if (status === "pass") {
    return "is-ok";
  }
  if (status === "warn" || status === "info") {
    return "is-warn";
  }
  return "is-danger";
}

function statusLabel(status: DiagnosticStatus) {
  const labels: Record<DiagnosticStatus, string> = {
    pass: "可用",
    warn: "需确认",
    fail: "阻断",
    info: "信息",
  };
  return labels[status];
}

function runStatusClass(status: RunState["status"]) {
  if (status === "ok") {
    return "is-ok";
  }
  if (status === "error") {
    return "is-danger";
  }
  if (status === "running") {
    return "is-warn";
  }
  return "";
}

function runStatusLabel(status: RunState["status"]) {
  const labels: Record<RunState["status"], string> = {
    idle: "待检查",
    running: "检查中",
    ok: "完成",
    error: "失败",
  };
  return labels[status];
}
