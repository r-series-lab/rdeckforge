import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { ActionableStatus } from "@/components/ActionableStatus";
import { ConfigDialog } from "@/components/ConfigDialog";
import { TemplatePicker } from "@/components/TemplatePicker";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import type { PageKey } from "@/app-modules";
import type { TemplatePackRecord } from "@/lib/template-packs";
import {
  DEFAULT_PROMPT_PACK_PATH,
  ensureContentExtension,
  resolvePromptPackPath,
  suggestedContentOutputPath,
  suggestedPromptOutputPath,
} from "@/lib/prompt-handoff";
import type { WorkflowDraft } from "@/lib/workflow-draft";
import { ensureTaskIdentity, isTauriRuntime } from "@/lib/workflow-tasks";

const defaultPromptPackPath = DEFAULT_PROMPT_PACK_PATH;
const defaultBriefPath = "";
const defaultTemplatePath = "";
const defaultOutputPath = "";
const defaultContentOutputPath = "";

type RunState = {
  status: "idle" | "running" | "ok" | "error";
  message: string;
};

type PromptBuildResult = {
  outputFile: string;
  promptBytes: number;
  promptPackId: string;
  promptPackName: string;
  templateId: string;
  templateName: string;
  inputFormats: string[];
  inputProfile?: string | null;
  inputSchemaLabel?: string | null;
  schemaIncluded: boolean;
  templateContractMode?: string | null;
  contentSkeletonIncluded: boolean;
  templateAuthoringIncluded: boolean;
};

type PromptPacksPageProps = {
  initialDraft?: WorkflowDraft | null;
  onWorkflowDraft?: (draft: WorkflowDraft, page?: PageKey) => void;
};

export function PromptPacksPage({ initialDraft, onWorkflowDraft }: PromptPacksPageProps) {
  const [promptPackPath, setPromptPackPath] = useState(defaultPromptPackPath);
  const [briefPath, setBriefPath] = useState(defaultBriefPath);
  const [templatePath, setTemplatePath] = useState(defaultTemplatePath);
  const [outputPath, setOutputPath] = useState(defaultOutputPath);
  const [contentOutputPath, setContentOutputPath] = useState(defaultContentOutputPath);
  const [configOpen, setConfigOpen] = useState(false);
  const [promptPreview, setPromptPreview] = useState("");
  const [aiOutputText, setAiOutputText] = useState("");
  const [savedContentPath, setSavedContentPath] = useState("");
  const [savedDraft, setSavedDraft] = useState<WorkflowDraft | null>(null);
  const [templates, setTemplates] = useState<TemplatePackRecord[]>([]);
  const [templateMessage, setTemplateMessage] = useState("模板库尚未载入。");
  const [result, setResult] = useState<PromptBuildResult | null>(null);
  const [runState, setRunState] = useState<RunState>({
    status: "idle",
    message: "生成给外部 AI 使用的结构化交接 prompt，应用本身不做推理。",
  });
  const appliedDraftKey = useRef("");
  const isRunning = runState.status === "running";
  const selectedTemplate = useMemo(
    () => templates.find((template) => template.path === templatePath),
    [templatePath, templates],
  );
  const statusActions = useMemo(() => {
    if (isRunning) {
      return [];
    }
    const actions: Array<{ label: string; onClick: () => void }> = [];
    if (!templatePath.trim()) {
      actions.push({ label: "选模板", onClick: () => setConfigOpen(true) });
    }
    if (!briefPath.trim()) {
      actions.push({ label: "选 brief", onClick: () => void chooseBrief() });
    }
    if (!outputPath.trim()) {
      actions.push({ label: "设路径", onClick: () => setConfigOpen(true) });
    }
    if (/prompt pack|提示词包/i.test(runState.message)) {
      actions.push({ label: "重置提示词包", onClick: () => setPromptPackPath(defaultPromptPackPath) });
    }
    return actions.slice(0, 3);
  }, [briefPath, isRunning, outputPath, runState.message, templatePath]);

  useEffect(() => {
    void loadTemplates(true);
  }, []);

  useEffect(() => {
    if (!initialDraft || initialDraft.stage !== "handoff") {
      return;
    }
    const key = [initialDraft.taskId, initialDraft.templatePath, initialDraft.briefPath, initialDraft.promptPath].join("|");
    if (appliedDraftKey.current === key) {
      return;
    }
    appliedDraftKey.current = key;
    setTemplatePath(initialDraft.templatePath);
    setBriefPath(initialDraft.briefPath ?? "");
    setOutputPath(initialDraft.promptPath ?? "");
    setContentOutputPath(initialDraft.contentPath);
    setSavedContentPath(initialDraft.contentPath);
    setSavedDraft(initialDraft);
    if (initialDraft.promptPath) {
      void invoke<string>("read_text_file", { path: initialDraft.promptPath })
        .then(setPromptPreview)
        .catch(() => setPromptPreview(""));
    }
    setRunState({ status: "idle", message: "已恢复交接任务，可以继续生成 prompt 或粘贴 AI 输出。" });
  }, [initialDraft]);

  async function loadTemplates(silent = false) {
    if (!isTauriRuntime()) {
      setTemplates([]);
      setTemplateMessage("桌面应用中会显示已链接模板包。");
      return;
    }
    try {
      const data = await invoke<TemplatePackRecord[]>("list_template_packs");
      setTemplates(data);
      setTemplateMessage(`已链接 ${data.length} 个模板包。`);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setTemplateMessage(message);
      if (!silent) {
        setRunState({ status: "error", message });
      }
    }
  }

  function selectRegisteredTemplate(template: TemplatePackRecord | null) {
    setTemplatePath(template?.path ?? "");
  }

  async function choosePromptPack() {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "选择提示词包目录",
      defaultPath: resolvePromptPackPath(promptPackPath),
    });
    if (typeof selected === "string") {
      setPromptPackPath(selected);
    }
  }

  async function chooseBrief() {
    const selected = await open({
      multiple: false,
      title: "选择需求 brief",
      defaultPath: briefPath,
      filters: [{ name: "Brief", extensions: ["md", "txt"] }],
    });
    if (typeof selected === "string") {
      setBriefPath(selected);
    }
  }

  async function chooseTemplate() {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "选择模板包目录",
      defaultPath: templatePath,
    });
    if (typeof selected === "string") {
      setTemplatePath(selected);
    }
  }

  async function chooseOutput() {
    const selected = await save({
      title: "保存 AI 交接 prompt",
      defaultPath: suggestedPromptOutputPath(outputPath, briefPath, templatePath),
      filters: [{ name: "Markdown", extensions: ["md"] }],
    });
    if (typeof selected === "string") {
      setOutputPath(selected);
    }
  }

  async function chooseContentOutput() {
    const extension = contentExtension(result?.inputFormats ?? selectedTemplate?.inputFormats ?? []);
    const selected = await save({
      title: "保存 AI 输出内容",
      defaultPath: suggestedContentOutputPath(contentOutputPath, briefPath, templatePath, extension),
      filters: [{ name: "内容文件", extensions: ["json", "md", "markdown"] }],
    });
    if (typeof selected === "string") {
      setContentOutputPath(selected);
    }
  }

  async function buildPrompt() {
    setRunState({ status: "running", message: "正在生成 AI 交接 prompt" });
    try {
      const resolvedPackPath = resolvePromptPackPath(promptPackPath);
      const resolvedTemplatePath = templatePath.trim();
      const resolvedBriefPath = briefPath.trim();
      const resolvedOutputPath = suggestedPromptOutputPath(outputPath, resolvedBriefPath, resolvedTemplatePath);
      if (!resolvedTemplatePath) {
        throw new Error("请先选择目标模板。");
      }
      if (!resolvedBriefPath) {
        throw new Error("请选择需求 brief。");
      }
      if (!resolvedOutputPath) {
        throw new Error("请在高级配置里设置 prompt 输出路径。");
      }
      setPromptPackPath(resolvedPackPath);
      setTemplatePath(resolvedTemplatePath);
      setBriefPath(resolvedBriefPath);
      setOutputPath(resolvedOutputPath);
      const data = await invoke<PromptBuildResult>("build_prompt_pack", {
        pack: resolvedPackPath,
        brief: resolvedBriefPath,
        template: resolvedTemplatePath,
        out: resolvedOutputPath,
      });
      setResult(data);
      setOutputPath(data.outputFile);
      const preview = await invoke<string>("read_text_file", { path: data.outputFile });
      setPromptPreview(preview);
      const draft = ensureTaskIdentity({
        taskId: savedDraft?.taskId,
        taskName: savedDraft?.taskName,
        templatePath: resolvedTemplatePath,
        contentPath: savedContentPath,
        outputPath: savedDraft?.outputPath ?? null,
        recipe: firstRecipeForFormat(selectedTemplate),
        format: normalizedOfficeFormat(selectedTemplate?.format),
        briefPath: resolvedBriefPath,
        promptPath: data.outputFile,
        status: "draft",
        stage: "handoff",
        source: "prompt-ai-output",
        updatedAt: Date.now(),
      });
      setSavedDraft(draft);
      onWorkflowDraft?.(draft);
      setRunState({
        status: "ok",
        message: `已生成：${data.promptPackName} -> ${data.templateName}`,
      });
    } catch (error) {
      setResult(null);
      setPromptPreview("");
      setRunState({
        status: "error",
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }

  async function copyPrompt() {
    await runAction("正在复制 prompt", async () => {
      const text =
        promptPreview ||
        (result?.outputFile ? await invoke<string>("read_text_file", { path: result.outputFile }) : "");
      if (!text.trim()) {
        throw new Error("还没有可复制的 prompt，请先生成交接 prompt。");
      }
      if (!navigator.clipboard?.writeText) {
        throw new Error("当前 WebView 不支持剪贴板。");
      }
      await navigator.clipboard.writeText(text);
      return "prompt 已复制，可以粘贴到网页 AI。";
    });
  }

  async function pasteAiOutput() {
    await runAction("正在读取剪贴板", async () => {
      if (!navigator.clipboard?.readText) {
        throw new Error("当前 WebView 不支持读取剪贴板。");
      }
      const text = await navigator.clipboard.readText();
      if (!text.trim()) {
        throw new Error("剪贴板里没有可用文本。");
      }
      setAiOutputText(text);
      return "已粘贴网页 AI 输出。";
    });
  }

  async function saveAiOutput() {
    await runAction("正在保存 AI 输出", async () => {
      if (!aiOutputText.trim()) {
        throw new Error("请先粘贴或填写网页 AI 输出。");
      }
      const extension = contentExtension(result?.inputFormats ?? selectedTemplate?.inputFormats ?? []);
      const path = suggestedContentOutputPath(contentOutputPath, briefPath, templatePath, extension);
      if (!path) {
        throw new Error("请在高级配置里设置 AI 输出保存路径。");
      }
      await invoke("write_text_file", { path, content: aiOutputText });
      const draft = buildWorkflowDraft(
        path,
        templatePath,
        selectedTemplate,
        savedDraft,
        briefPath,
        result?.outputFile ?? outputPath,
      );
      setContentOutputPath(path);
      setSavedContentPath(path);
      setSavedDraft(draft);
      onWorkflowDraft?.(draft);
      return `AI 输出已保存：${path}`;
    });
  }

  function openDraftTarget(page: "content" | "render") {
    const draft = savedDraft ?? buildWorkflowDraft(savedContentPath, templatePath, selectedTemplate);
    if (!draft.contentPath) {
      setRunState({ status: "error", message: "请先保存网页 AI 输出。" });
      return;
    }
    onWorkflowDraft?.(draft, page);
  }

  async function openOutput() {
    if (!result?.outputFile) {
      return;
    }
    await invoke("open_path", { path: result.outputFile });
  }

  async function revealOutput() {
    if (!result?.outputFile) {
      return;
    }
    await invoke("reveal_path", { path: result.outputFile });
  }

  async function openSavedContent() {
    if (!savedContentPath) {
      return;
    }
    await invoke("open_path", { path: savedContentPath });
  }

  async function revealSavedContent() {
    if (!savedContentPath) {
      return;
    }
    await invoke("reveal_path", { path: savedContentPath });
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
    <div className="page-grid workspace-fill-grid">
      <Card className="span-7">
        <CardHeader>
          <CardTitle>AI 交接包</CardTitle>
          <p className="muted">把模板协议和 brief 打包成 prompt，交给外部 AI 生成内容。</p>
        </CardHeader>
        <CardContent>
          <div className="form-stack">
            <div className="form-field">
              <span>目标模板</span>
              <TemplatePicker
                templates={templates}
                value={templatePath}
                disabled={isRunning}
                onSelect={selectRegisteredTemplate}
                onRefresh={() => void loadTemplates(false)}
                onManualPath={() => setConfigOpen(true)}
              />
            </div>
            <label>
              需求 brief
              <div className="field-row">
                <Input value={briefPath} onChange={(event) => setBriefPath(event.target.value)} />
                <Button type="button" onClick={chooseBrief} disabled={isRunning}>
                  选择
                </Button>
              </div>
            </label>
          </div>
          <div className="selection-summary">
            <div>
              <span>Prompt</span>
              <strong className="path-text">{outputPath}</strong>
            </div>
            <div>
              <span>内容保存</span>
              <strong className="path-text">{contentOutputPath}</strong>
            </div>
          </div>
          <div className="action-row">
            <Button onClick={() => setConfigOpen(true)} disabled={isRunning}>
              高级配置
            </Button>
            <Button variant="primary" onClick={buildPrompt} disabled={isRunning}>
              生成交接 prompt
            </Button>
            <Button onClick={copyPrompt} disabled={!promptPreview || isRunning}>
              复制 prompt
            </Button>
            <Button onClick={openOutput} disabled={!result || isRunning}>
              打开
            </Button>
            <Button onClick={revealOutput} disabled={!result || isRunning}>
              定位
            </Button>
          </div>
          <ActionableStatus
            status={runState.status}
            label={statusLabel(runState.status)}
            message={runState.message}
            actions={statusActions}
          >
            <span className="status-pill">{templates.length}</span>
            <span>{templateMessage}</span>
          </ActionableStatus>
          <div className="form-section">
            <div className="section-heading">
              <strong>Prompt 预览</strong>
              <span>确认内容后复制到网页 AI，AI 的回复再粘回下面的输出区。</span>
            </div>
            <Textarea
              className="code-area prompt-preview-area"
              value={promptPreview}
              readOnly
              placeholder="生成交接 prompt 后会在这里预览。"
              spellCheck={false}
            />
          </div>
          <div className="form-section">
            <div className="section-heading">
              <strong>网页 AI 输出回填</strong>
              <span>粘贴网页 AI 返回的 content.md / content.json，保存后可校验或直接生成 Office 文档。</span>
            </div>
            <Textarea
              className="code-area ai-output-area"
              value={aiOutputText}
              onChange={(event) => setAiOutputText(event.target.value)}
              placeholder="把网页 AI 返回的 Markdown 或 JSON 粘贴到这里。"
              spellCheck={false}
            />
            <div className="action-row">
              <Button onClick={pasteAiOutput} disabled={isRunning}>
                从剪贴板粘贴
              </Button>
              <Button variant="primary" onClick={saveAiOutput} disabled={isRunning}>
                保存 AI 输出
              </Button>
              <Button onClick={openSavedContent} disabled={!savedContentPath || isRunning}>
                打开内容
              </Button>
              <Button onClick={revealSavedContent} disabled={!savedContentPath || isRunning}>
                定位内容
              </Button>
              <Button onClick={() => openDraftTarget("content")} disabled={!savedContentPath || isRunning}>
                校验内容
              </Button>
              <Button
                onClick={() => openDraftTarget("render")}
                disabled={!savedContentPath || isRunning}
              >
                进入生成
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>
      <Card className="span-5">
        <CardHeader>
          <CardTitle>交接结果</CardTitle>
        </CardHeader>
        <CardContent>
          {result ? (
            <dl className="definition-list">
              <div>
                <dt>提示词包</dt>
                <dd>{result.promptPackId}</dd>
              </div>
              <div>
                <dt>模板</dt>
                <dd>{result.templateId}</dd>
              </div>
              <div>
                <dt>输入格式</dt>
                <dd>{result.inputFormats.join(", ")}</dd>
              </div>
              <div>
                <dt>内容协议</dt>
                <dd>{result.inputProfile ?? result.inputSchemaLabel ?? "未声明"}</dd>
              </div>
              <div>
                <dt>契约模式</dt>
                <dd>{formatContractMode(result.templateContractMode)}</dd>
              </div>
              <div>
                <dt>内容骨架</dt>
                <dd>{result.contentSkeletonIncluded ? "已注入" : "无声明式骨架"}</dd>
              </div>
              <div>
                <dt>模板写作规范</dt>
                <dd>{result.templateAuthoringIncluded ? "已注入" : "未声明"}</dd>
              </div>
              <div>
                <dt>输出大小</dt>
                <dd>{Math.round(result.promptBytes / 1024)} KB</dd>
              </div>
              <div>
                <dt>输出文件</dt>
                <dd className="path-text">{result.outputFile}</dd>
              </div>
              {savedContentPath ? (
                <div>
                  <dt>内容文件</dt>
                  <dd className="path-text">{savedContentPath}</dd>
                </div>
              ) : null}
            </dl>
          ) : (
            <ul className="check-list">
              <li>AI 负责从 brief 生成 `content.json` / `content.md`。</li>
              <li>rDeckForge 只提供模板协议、约束、校验和 Office 渲染。</li>
              <li>生成后的内容回到“输出校验”或“渲染”页验收。</li>
            </ul>
          )}
        </CardContent>
      </Card>
      <ConfigDialog
        open={configOpen}
        title="交接高级配置"
        description="低频路径和保存位置放在这里，主流程保持清爽。"
        onClose={() => setConfigOpen(false)}
      >
        <div className="form-stack">
          <label>
            提示词包目录
            <div className="field-row">
              <Input
                value={promptPackPath}
                onChange={(event) => setPromptPackPath(event.target.value)}
              />
              <Button type="button" onClick={choosePromptPack} disabled={isRunning}>
                选择
              </Button>
            </div>
          </label>
          <label>
            模板包目录
            <div className="field-row">
              <Input value={templatePath} onChange={(event) => setTemplatePath(event.target.value)} />
              <Button type="button" onClick={chooseTemplate} disabled={isRunning}>
                选择
              </Button>
            </div>
          </label>
          <label>
            输出 prompt
            <div className="field-row">
              <Input value={outputPath} onChange={(event) => setOutputPath(event.target.value)} />
              <Button type="button" onClick={chooseOutput} disabled={isRunning}>
                选择
              </Button>
            </div>
          </label>
          <label>
            保存 AI 输出为
            <div className="field-row">
              <Input
                value={contentOutputPath}
                onChange={(event) => setContentOutputPath(event.target.value)}
              />
              <Button type="button" onClick={chooseContentOutput} disabled={isRunning}>
                选择
              </Button>
            </div>
          </label>
        </div>
      </ConfigDialog>
    </div>
  );
}

function statusLabel(status: RunState["status"]) {
  if (status === "running") {
    return "运行中";
  }
  if (status === "ok") {
    return "完成";
  }
  if (status === "error") {
    return "错误";
  }
  return "就绪";
}

function contentExtension(formats: string[]) {
  const normalized = formats.map((format) => format.toLowerCase());
  if (normalized.includes("md") || normalized.includes("markdown")) {
    return "md";
  }
  return "json";
}

function buildWorkflowDraft(
  contentPath: string,
  templatePath: string,
  template: TemplatePackRecord | undefined,
  currentDraft?: WorkflowDraft | null,
  briefPath?: string,
  promptPath?: string,
): WorkflowDraft {
  const format = normalizedOfficeFormat(template?.format);
  return ensureTaskIdentity({
    taskId: currentDraft?.taskId,
    taskName: currentDraft?.taskName,
    templatePath,
    contentPath,
    recipe: firstRecipeForFormat(template),
    format,
    outputPath: currentDraft?.outputPath ?? null,
    briefPath: briefPath || currentDraft?.briefPath || null,
    promptPath: promptPath || currentDraft?.promptPath || null,
    validationStatus: null,
    lastError: null,
    status: "content_ready",
    stage: "validation",
    source: "prompt-ai-output",
    updatedAt: Date.now(),
  });
}

function firstRecipeForFormat(template: TemplatePackRecord | undefined) {
  if (!template) {
    return "";
  }
  const format = normalizedOfficeFormat(template.format);
  if (format === "docx") {
    return template.documentRecipeIds[0] ?? "";
  }
  if (format === "xlsx") {
    return template.workbookRecipeIds[0] ?? "";
  }
  if (template.templateType === "script" || (template.templateType === "hybrid" && template.rendererType)) {
    return "";
  }
  return template.deckRecipeIds[0] ?? "";
}

function normalizedOfficeFormat(format: string | null | undefined) {
  if (format === "docx" || format === "xlsx") {
    return format;
  }
  return "pptx";
}

function formatContractMode(mode: string | null | undefined) {
  if (mode === "script_renderer") {
    return "脚本渲染输入";
  }
  if (mode === "explicit_pages") {
    return "PPTX 显式页面";
  }
  if (mode === "document_recipe") {
    return "DOCX 文档结构";
  }
  if (mode === "workbook_recipe") {
    return "XLSX 工作簿结构";
  }
  return mode ?? "未声明";
}
