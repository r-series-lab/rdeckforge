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
import {
  type TemplatePackRecord,
} from "@/lib/template-packs";
import {
  renderedOutputFile,
  type OfficeRenderResult,
} from "@/lib/office-render-result";
import type { WorkflowDraft } from "@/lib/workflow-draft";
import {
  buildSuggestedOutputPath,
  readOutputPreferences,
} from "@/lib/output-preferences";

const defaultContentPath = "";
const defaultTemplatePath = "";
const defaultOutputPath = "";

const starterContent = `{
  "schemaVersion": "1.0",
  "documentType": "teaching_deck",
  "title": "示例教学课程",
  "speaker": "讲者姓名",
  "date": "2026-06-21",
  "goals": ["说明课程核心概念", "掌握关键流程"],
  "chapters": [
    {
      "id": "chapter-1",
      "label": "第一部分",
      "title": "课程概述",
      "contentTitle": "课程概述",
      "items": [{ "heading": "重点", "body": "聚焦关键知识点。" }]
    }
  ],
  "summary": ["围绕核心流程组织教学内容"],
  "evaluations": [{ "title": "教学评价", "items": ["理论提问"] }]
}`;

type RunState = {
  status: "idle" | "running" | "ok" | "error";
  message: string;
};

type ContentValidation = {
  inputFormat: string;
  inputProfile?: string | null;
  inputSchema?: string | null;
  inputSchemaId?: string | null;
  inputBuiltinSchema?: boolean;
  rootType: string;
  topLevelKeys: string[];
  schemaVersion: string;
  documentType: string;
  title: string;
  speaker?: string | null;
  chapters: number;
  goals: number;
  summaryItems: number;
  evaluations: number;
  warnings: string[];
  schemaErrors: string[];
};

type TemplateValidation = {
  templateId: string;
  name: string;
  format: string;
  entryPath: string;
  pageTemplateCount: number;
  deckRecipeCount: number;
  documentRecipeCount: number;
  workbookRecipeCount: number;
  pageTemplateIds: string[];
  deckRecipeIds: string[];
  documentRecipeIds: string[];
  workbookRecipeIds: string[];
  legacyLayoutCount: number;
  health?: {
    status: string;
    checked: string[];
    warnings: string[];
  };
  warnings: string[];
};

type PlannedPage = {
  pageIndex: number;
  pageTemplate: string;
  sourceSlide: number;
  dataPath: string;
  repeatIndex?: number | null;
};

type ContentWorkspaceValidation = {
  content: ContentValidation;
  template?: TemplateValidation | null;
  renderFormat?: string | null;
  selectedRecipe?: string | null;
  deckRecipe?: string | null;
  documentRecipe?: string | null;
  workbookRecipe?: string | null;
  plannedPages: PlannedPage[];
  bindingWarnings: string[];
  acceptanceSummary: AcceptanceSummary;
};

type AcceptanceSummary = {
  status: "pass" | "warn" | "fail";
  canRender: boolean;
  schemaErrorCount: number;
  contentWarningCount: number;
  templateWarningCount: number;
  bindingWarningCount: number;
  assetWarningCount: number;
  plannedPageCount: number;
  message: string;
};

type OutputPathResolution = {
  resolvedPath: string;
};

type ContentPageProps = {
  initialDraft?: WorkflowDraft | null;
  onWorkflowDraft?: (draft: WorkflowDraft) => void;
};

export function ContentPage({ initialDraft, onWorkflowDraft }: ContentPageProps) {
  const [contentPath, setContentPath] = useState(defaultContentPath);
  const [templatePath, setTemplatePath] = useState(defaultTemplatePath);
  const [recipe, setRecipe] = useState("teaching_deck");
  const [outputPath, setOutputPath] = useState(defaultOutputPath);
  const [configOpen, setConfigOpen] = useState(false);
  const [contentText, setContentText] = useState(starterContent);
  const [templates, setTemplates] = useState<TemplatePackRecord[]>([]);
  const [validation, setValidation] = useState<ContentWorkspaceValidation | null>(null);
  const [renderResult, setRenderResult] = useState<OfficeRenderResult | null>(null);
  const [dirty, setDirty] = useState(false);
  const [runState, setRunState] = useState<RunState>({
    status: "idle",
    message: "选择 AI 生成的 JSON / Markdown 输出并校验。",
  });
  const validationRequest = useRef(0);
  const appliedDraftKey = useRef("");

  const isRunning = runState.status === "running";
  const selectedTemplate = useMemo(
    () => templates.find((template) => template.path === templatePath),
    [templatePath, templates],
  );
  const activeFormat = validation?.renderFormat ?? selectedTemplate?.format ?? "pptx";
  const recipeOptions =
    recipeOptionsForTemplate(selectedTemplate, activeFormat) ??
    recipeOptionsForValidation(validation?.template, activeFormat);
  const warnings = [
    ...safeArray(validation?.content.warnings),
    ...safeArray(validation?.content.schemaErrors),
    ...safeArray(validation?.template?.warnings),
    ...safeArray(validation?.bindingWarnings),
  ];
  const statusActions = useMemo(() => {
    if (isRunning) {
      return [];
    }
    const actions: Array<{ label: string; onClick: () => void; disabled?: boolean }> = [];
    if (!templatePath.trim()) {
      actions.push({ label: "选模板", onClick: () => setConfigOpen(true) });
    }
    if (!contentPath.trim()) {
      actions.push({ label: "选内容", onClick: () => void chooseContentJson() });
    }
    if (dirty) {
      actions.push({ label: "保存微调", onClick: () => void saveContentFile(), disabled: !contentPath.trim() });
    }
    if (!outputPath.trim()) {
      actions.push({ label: "设输出", onClick: () => setConfigOpen(true) });
    }
    if (validation?.acceptanceSummary.canRender) {
      actions.push({ label: "生成文件", onClick: () => void renderAcceptedOffice() });
    }
    return actions.slice(0, 3);
  }, [contentPath, dirty, isRunning, outputPath, templatePath, validation?.acceptanceSummary.canRender]);

  useEffect(() => {
    void loadTemplates(true);
  }, []);

  useEffect(() => {
    if (!initialDraft) {
      return;
    }
    const key = draftKey(initialDraft);
    if (appliedDraftKey.current === key) {
      return;
    }
    appliedDraftKey.current = key;
    setTemplatePath(initialDraft.templatePath);
    setRecipe(initialDraft.recipe ?? "");
    setOutputPath((current) =>
      initialDraft.outputPath
        ? withOfficeExtension(initialDraft.outputPath, initialDraft.format ?? "pptx")
        : withOfficeExtension(current, initialDraft.format ?? "pptx"),
    );
    if (initialDraft.contentPath) {
      void loadContentFile(initialDraft.contentPath, false);
    } else {
      setContentPath("");
      setContentText(starterContent);
      setDirty(false);
      setValidation(null);
      setRunState({
        status: "idle",
        message: "已选择模板，请继续选择 AI 输出文件或粘贴内容。",
      });
    }
  }, [initialDraft]);

  useEffect(() => {
    if (!contentText.trim() || (!dirty && !contentPath.trim())) {
      setValidation(null);
      return;
    }
    const timeout = window.setTimeout(() => {
      void validateEditor("auto");
    }, 700);
    return () => window.clearTimeout(timeout);
  }, [contentText, templatePath, recipe]);

  function updateContent(value: string) {
    setContentText(value);
    setDirty(true);
  }

  async function loadTemplates(silent = false) {
    try {
      const data = await invoke<TemplatePackRecord[]>("list_template_packs");
      setTemplates(data);
    } catch (error) {
      if (!silent) {
        setRunState({
          status: "error",
          message: error instanceof Error ? error.message : String(error),
        });
      }
    }
  }

  function selectRegisteredTemplate(record: TemplatePackRecord | null) {
    if (!record) {
      setTemplatePath("");
      setValidation(null);
      return;
    }
    setTemplatePath(record.path);
    const options = recipeOptionsForTemplate(record, record.format) ?? [];
    setRecipe((current) => (options.includes(current) ? current : firstRecipeForFormat(record)));
    setOutputPath((current) => withOfficeExtension(current, record.format));
  }

  async function chooseTemplateFolder() {
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

  async function chooseContentJson() {
    const selected = await open({
      multiple: false,
      title: "选择内容文件",
      defaultPath: contentPath,
      filters: [{ name: "Content Source", extensions: ["json", "md", "markdown"] }],
    });
    if (typeof selected === "string") {
      await loadContentFile(selected, false);
    }
  }

  async function chooseOutputOffice() {
    const format = normalizedOfficeFormat(activeFormat);
    const selected = await save({
      title: `选择输出 ${format.toUpperCase()}`,
      defaultPath: withOfficeExtension(outputPath, format),
      filters: [{ name: officeFormatName(format), extensions: [format] }],
    });
    if (typeof selected === "string") {
      setOutputPath(selected);
    }
  }

  function applyOutputRule() {
    const format = normalizedOfficeFormat(activeFormat);
    const output = buildSuggestedOutputPath(readOutputPreferences(), {
      contentPath,
      templateName: selectedTemplate?.name ?? fileName(templatePath),
      format,
    });
    if (!output) {
      setRunState({ status: "error", message: "请先在设置 → 输出中选择默认输出目录。" });
      return;
    }
    setOutputPath(output);
    setRunState({ status: "idle", message: "已按输出规则生成文件路径。" });
  }

  async function loadContentFile(path = contentPath, silent = false) {
    if (!silent) {
      setRunState({ status: "running", message: "正在读取内容文件" });
    }
    try {
      const raw = await invoke<string>("read_text_file", { path });
      setContentPath(path);
      setContentText(raw);
      setDirty(false);
      setRenderResult(null);
      if (!silent) {
        setRunState({ status: "ok", message: "内容文件已载入" });
      }
    } catch (error) {
      if (!silent) {
        setRunState({
          status: "error",
          message: error instanceof Error ? error.message : String(error),
        });
      }
    }
  }

  async function saveContentFile(path = contentPath) {
    setRunState({ status: "running", message: "正在保存内容文件" });
    try {
      await invoke("write_text_file", { path, content: contentText });
      setContentPath(path);
      setDirty(false);
      setRenderResult(null);
      publishWorkflowDraft({
        contentPath: path,
        status: "content_ready",
        stage: "validation",
        validationStatus: null,
        lastError: null,
      });
      setRunState({ status: "ok", message: "内容文件已保存" });
    } catch (error) {
      setRunState({
        status: "error",
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }

  async function saveContentAs() {
    const selected = await save({
      title: "保存内容文件",
      defaultPath: contentPath,
      filters: [{ name: "内容文件", extensions: ["json", "md", "markdown"] }],
    });
    if (typeof selected === "string") {
      await saveContentFile(selected);
    }
  }

  async function validateEditor(source: "manual" | "auto") {
    const requestId = ++validationRequest.current;
    if (source === "manual") {
      setRunState({ status: "running", message: "正在校验内容和模板绑定" });
    }
    try {
      const data = dirty || !contentPath.trim()
        ? await invoke<ContentWorkspaceValidation>("validate_content_text", {
            content: contentText,
            inputFormat: inferInputFormat(contentPath),
            template: templatePath || null,
            recipe: recipe || null,
            profile: null,
          })
        : await invoke<ContentWorkspaceValidation>("validate_content_file", {
            input: contentPath,
            template: templatePath || null,
            recipe: recipe || null,
            profile: null,
          });
      if (requestId !== validationRequest.current) {
        return;
      }
      setValidation(data);
      setRenderResult(null);
      if (data.selectedRecipe) {
        setRecipe(data.selectedRecipe);
      }
      if (data.renderFormat) {
        setOutputPath((current) => withOfficeExtension(current, data.renderFormat ?? "pptx"));
      }
      setRunState({
        status: data.acceptanceSummary.status === "fail" ? "error" : "ok",
        message:
          source === "manual"
            ? acceptanceMessage(data.acceptanceSummary, data.renderFormat, data.selectedRecipe)
            : data.acceptanceSummary.status === "fail"
              ? `自动校验失败：${data.acceptanceSummary.schemaErrorCount} 个协议问题`
              : autoValidationMessage(data),
      });
      publishWorkflowDraft({
        recipe: data.selectedRecipe ?? recipe,
        format: data.renderFormat ?? activeFormat,
        outputPath: withOfficeExtension(outputPath, data.renderFormat ?? activeFormat),
        status: data.acceptanceSummary.status === "fail" ? "error" : "validated",
        stage: data.acceptanceSummary.status === "fail" ? "validation" : "render",
        validationStatus: data.acceptanceSummary.status,
        lastError: data.acceptanceSummary.status === "fail" ? data.acceptanceSummary.message : null,
      });
    } catch (error) {
      if (requestId !== validationRequest.current) {
        return;
      }
      setValidation(null);
      setRunState({
        status: "error",
        message: error instanceof Error ? error.message : String(error),
      });
      publishWorkflowDraft({
        status: "error",
        stage: "validation",
        validationStatus: "fail",
        lastError: error instanceof Error ? error.message : String(error),
      });
    }
  }

  async function renderAcceptedOffice() {
    if (dirty) {
      setRunState({ status: "error", message: "当前内容有未保存微调，请先保存再生成。" });
      return;
    }
    if (!validation?.acceptanceSummary.canRender) {
      setRunState({
        status: "error",
        message: validation
          ? "验收结果仍有阻断问题，请先处理协议、绑定或资源路径。"
          : "请先校验 AI 输出。",
      });
      return;
    }
    const format = normalizedOfficeFormat(validation.renderFormat ?? activeFormat);
    const selectedRecipe = (validation.selectedRecipe ?? recipe) || null;
    const requestedOutput = withOfficeExtension(outputPath, format);
    const preferences = readOutputPreferences();
    setRunState({ status: "running", message: `正在根据已验收输出生成 ${format.toUpperCase()}` });
    try {
      const output = await resolveRequestedOutput(requestedOutput, preferences.conflictPolicy);
      setOutputPath(output);
      const data = await invoke<OfficeRenderResult>("render_accepted_office", {
        input: contentPath,
        template: templatePath,
        recipe: selectedRecipe,
        output,
      });
      setRenderResult(data);
      publishWorkflowDraft({
        recipe: selectedRecipe,
        format,
        outputPath: renderedOutputFile(data) ?? output,
        status: "rendered",
        stage: "render",
        validationStatus: validation.acceptanceSummary.status,
        lastError: null,
      });
      setRunState({
        status: "ok",
        message: `已生成 ${format.toUpperCase()}：${renderedOutputFile(data) ?? output}`,
      });
      if (preferences.autoOpen) {
        void invoke("open_path", { path: renderedOutputFile(data) ?? output }).catch(() => undefined);
      }
    } catch (error) {
      setRunState({
        status: "error",
        message: error instanceof Error ? error.message : String(error),
      });
      publishWorkflowDraft({
        status: "error",
        stage: "render",
        lastError: error instanceof Error ? error.message : String(error),
      });
    }
  }

  function publishWorkflowDraft(
    patch: Partial<Pick<WorkflowDraft,
      | "contentPath"
      | "recipe"
      | "format"
      | "outputPath"
      | "status"
      | "stage"
      | "validationStatus"
      | "lastError"
    >>,
  ) {
    if (!initialDraft?.taskId && !templatePath.trim() && !contentPath.trim()) {
      return;
    }
    onWorkflowDraft?.({
      taskId: initialDraft?.taskId,
      taskName: initialDraft?.taskName,
      templatePath,
      contentPath,
      recipe: recipe || null,
      format: activeFormat,
      outputPath: outputPath || null,
      briefPath: initialDraft?.briefPath ?? null,
      promptPath: initialDraft?.promptPath ?? null,
      validationStatus: initialDraft?.validationStatus ?? null,
      lastError: null,
      status: initialDraft?.status ?? "content_ready",
      stage: "validation",
      source: "content-validation",
      updatedAt: Date.now(),
      ...patch,
    });
  }

  async function openRenderedPptx() {
    const path = renderedOutputFile(renderResult);
    if (path) {
      await invoke("open_path", { path });
    }
  }

  async function revealRenderedPptx() {
    const path = renderedOutputFile(renderResult);
    if (path) {
      await invoke("reveal_path", { path });
    }
  }

  async function resolveRequestedOutput(path: string, conflictPolicy: string) {
    if (!("__TAURI_INTERNALS__" in window)) {
      return path;
    }
    const resolution = await invoke<OutputPathResolution>("resolve_output_path", {
      path,
      conflictPolicy,
    });
    return resolution.resolvedPath;
  }

  return (
    <div className="page-grid workspace-fill-grid">
      <Card className="span-8">
        <CardHeader>
          <CardTitle>AI 输出校验</CardTitle>
          <p className="muted">验收外部 AI 生成的 JSON 或 Markdown，按模板协议预检页面绑定。</p>
        </CardHeader>
        <CardContent>
          <div className="form-stack">
            <div className="form-field">
              <span>模板</span>
              <TemplatePicker
                templates={templates}
                value={templatePath}
                contentPath={contentPath}
                disabled={isRunning}
                onSelect={selectRegisteredTemplate}
                onRefresh={() => void loadTemplates(false)}
                onManualPath={() => setConfigOpen(true)}
              />
            </div>
            <label>
              内容
              <div className="field-row">
                <Input
                  value={contentPath}
                  onChange={(event) => setContentPath(event.target.value)}
                />
                <Button type="button" onClick={chooseContentJson} disabled={isRunning}>
                  选择
                </Button>
              </div>
            </label>
          </div>
          <div className="selection-summary">
            <div>
              <span>结构</span>
              <strong>{recipe || validation?.selectedRecipe || "未选择"}</strong>
            </div>
            <div>
              <span>输出</span>
              <strong className="path-text">{outputPath}</strong>
            </div>
          </div>
          <Textarea
            className="code-area content-editor-area"
            value={contentText}
            onChange={(event) => updateContent(event.target.value)}
            spellCheck={false}
          />
          <div className="action-row">
            <Button onClick={() => setConfigOpen(true)} disabled={isRunning}>
              高级配置
            </Button>
            <Button variant="primary" onClick={() => void validateEditor("manual")} disabled={isRunning}>
              校验输出
            </Button>
            <Button onClick={() => void loadContentFile()} disabled={isRunning}>
              重新载入
            </Button>
            <Button onClick={() => void saveContentFile()} disabled={isRunning}>
              保存微调
            </Button>
            <Button onClick={saveContentAs} disabled={isRunning}>
              另存为
            </Button>
            <Button
              variant="primary"
              onClick={renderAcceptedOffice}
              disabled={isRunning || !validation?.acceptanceSummary.canRender}
            >
              生成 {normalizedOfficeFormat(activeFormat).toUpperCase()}
            </Button>
          </div>
          <ActionableStatus
            status={runState.status}
            label={statusLabel(runState.status)}
            message={runState.message}
            actions={statusActions}
          >
            {dirty ? <span className="status-pill is-warn">未保存</span> : null}
          </ActionableStatus>
        </CardContent>
      </Card>
      <Card className="span-4">
        <CardHeader>
          <CardTitle>校验结果</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="status-stack">
            {validation ? (
              <>
                <div className="acceptance-summary">
                  <span
                    className={`status-pill ${acceptanceStatusClass(
                      validation.acceptanceSummary.status,
                    )}`}
                  >
                    {acceptanceStatusLabel(validation.acceptanceSummary.status)}
                  </span>
                  <strong>
                    {acceptanceMessage(
                      validation.acceptanceSummary,
                      validation.renderFormat,
                      validation.selectedRecipe,
                    )}
                  </strong>
                  <span>{validation.acceptanceSummary.canRender ? "可以生成 Office 文件" : "仍有阻断项"}</span>
                </div>
                <dl className="definition-list content-metrics">
                  <div>
                    <dt>协议问题</dt>
                    <dd>{validation.acceptanceSummary.schemaErrorCount}</dd>
                  </div>
                  <div>
                    <dt>绑定提示</dt>
                    <dd>{validation.acceptanceSummary.bindingWarningCount}</dd>
                  </div>
                  <div>
                    <dt>资源提示</dt>
                    <dd>{validation.acceptanceSummary.assetWarningCount}</dd>
                  </div>
                  <div>
                    <dt>标题</dt>
                    <dd>{validation.content.title}</dd>
                  </div>
                  <div>
                    <dt>输入</dt>
                    <dd>
                      {validation.content.inputFormat}
                      {validation.content.inputSchemaId
                        ? ` / ${validation.content.inputSchemaId}`
                        : ""}
                    </dd>
                  </div>
                  <div>
                    <dt>协议</dt>
                    <dd>
                      {validation.content.schemaVersion} / {validation.content.documentType}
                    </dd>
                  </div>
                  <div>
                    <dt>内容数量</dt>
                    <dd>
                      {validation.content.goals} 个目标，{validation.content.chapters} 个章节，{" "}
                      {validation.content.evaluations} 个评价
                    </dd>
                  </div>
                  <div>
                    <dt>模板</dt>
                    <dd>
                      {validation.template?.name ?? "未选择模板"}
                      {validation.renderFormat ? ` / ${validation.renderFormat.toUpperCase()}` : ""}
                    </dd>
                  </div>
                  <div>
                    <dt>结构</dt>
                    <dd>{validation.selectedRecipe ?? "-"}</dd>
                  </div>
                  <div>
                    <dt>{validation.renderFormat === "pptx" ? "计划页" : "输出结构"}</dt>
                    <dd>
                      {validation.renderFormat === "pptx"
                        ? validation.acceptanceSummary.plannedPageCount
                        : validation.selectedRecipe ?? "-"}
                    </dd>
                  </div>
                  {renderedOutputFile(renderResult) ? (
                    <div>
                      <dt>生成文件</dt>
                      <dd className="path-text">{renderedOutputFile(renderResult)}</dd>
                    </div>
                  ) : null}
                </dl>
                {renderedOutputFile(renderResult) ? (
                  <div className="output-actions">
                    <Button size="sm" variant="primary" onClick={openRenderedPptx} disabled={isRunning}>
                      打开
                    </Button>
                    <Button size="sm" onClick={revealRenderedPptx} disabled={isRunning}>
                      定位
                    </Button>
                  </div>
                ) : null}
                {warnings.length > 0 ? (
                  <ul className="warning-list">
                    {warnings.map((warning) => (
                      <li key={warning}>{warning}</li>
                    ))}
                  </ul>
                ) : (
                  <span className="status-pill is-ok">无警告</span>
                )}
                <div className="planned-page-list">
                  {safeArray(validation.plannedPages).map((page) => (
                    <div className="planned-page-row" key={`${page.pageIndex}-${page.pageTemplate}`}>
                      <strong>{page.pageIndex}</strong>
                      <span>{page.pageTemplate}</span>
                      <small>{page.dataPath}</small>
                    </div>
                  ))}
                </div>
              </>
            ) : (
              <div className="empty-panel compact-empty">
                <strong>还没有可用内容</strong>
                <span>点击校验，或编辑后等待自动校验。</span>
              </div>
            )}
          </div>
        </CardContent>
      </Card>
      <ConfigDialog
        open={configOpen}
        title="校验高级配置"
        description="模板路径、结构和输出位置在这里调整。"
        onClose={() => setConfigOpen(false)}
      >
        <div className="form-stack">
          <label>
            模板包目录
            <div className="field-row">
              <Input
                value={templatePath}
                onChange={(event) => setTemplatePath(event.target.value)}
              />
              <Button type="button" onClick={chooseTemplateFolder} disabled={isRunning}>
                选择
              </Button>
            </div>
          </label>
          <label>
            模板结构
            {recipeOptions.length > 0 ? (
              <select
                className="ui-select"
                value={recipe}
                onChange={(event) => setRecipe(event.target.value)}
              >
                {!recipeOptions.includes(recipe) && recipe ? (
                  <option value={recipe}>{recipe}</option>
                ) : null}
                {recipeOptions.map((item) => (
                  <option value={item} key={item}>
                    {item}
                  </option>
                ))}
              </select>
            ) : (
              <Input value={recipe} onChange={(event) => setRecipe(event.target.value)} />
            )}
          </label>
          <label>
            验收后输出 {normalizedOfficeFormat(activeFormat).toUpperCase()}
            <div className="field-row">
              <Input value={outputPath} onChange={(event) => setOutputPath(event.target.value)} />
              <Button type="button" onClick={applyOutputRule} disabled={isRunning || !contentPath}>
                按规则
              </Button>
              <Button type="button" onClick={chooseOutputOffice} disabled={isRunning}>
                另存为
              </Button>
            </div>
          </label>
        </div>
      </ConfigDialog>
    </div>
  );
}

function inferInputFormat(path: string) {
  const extension = path.split(".").pop()?.toLowerCase();
  if (extension === "md" || extension === "markdown") {
    return "md";
  }
  return "json";
}

function fileName(path: string) {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? "";
}

function draftKey(draft: WorkflowDraft) {
  return [
    draft.taskId ?? "",
    draft.templatePath,
    draft.contentPath,
    draft.recipe ?? "",
    draft.format ?? "",
    draft.outputPath ?? "",
  ].join("|");
}

function recipeOptionsForTemplate(record: TemplatePackRecord | undefined, format: string) {
  if (!record) {
    return undefined;
  }
  const normalized = normalizedOfficeFormat(format);
  if (normalized === "docx") {
    return safeArray(record.documentRecipeIds);
  }
  if (normalized === "xlsx") {
    return safeArray(record.workbookRecipeIds);
  }
  return safeArray(record.deckRecipeIds);
}

function recipeOptionsForValidation(template: TemplateValidation | undefined | null, format: string) {
  if (!template) {
    return [];
  }
  const normalized = normalizedOfficeFormat(format);
  if (normalized === "docx") {
    return safeArray(template.documentRecipeIds);
  }
  if (normalized === "xlsx") {
    return safeArray(template.workbookRecipeIds);
  }
  return safeArray(template.deckRecipeIds);
}

function firstRecipeForFormat(record: TemplatePackRecord) {
  return recipeOptionsForTemplate(record, record.format)?.[0] ?? "";
}

function normalizedOfficeFormat(format: string | null | undefined) {
  if (format === "docx" || format === "xlsx") {
    return format;
  }
  return "pptx";
}

function officeFormatName(format: string) {
  if (format === "docx") {
    return "Word";
  }
  if (format === "xlsx") {
    return "Excel";
  }
  return "PowerPoint";
}

function withOfficeExtension(path: string, format: string) {
  if (!path.trim()) {
    return "";
  }
  const normalized = normalizedOfficeFormat(format);
  if (path.match(/\.(pptx|docx|xlsx)$/i)) {
    return path.replace(/\.(pptx|docx|xlsx)$/i, `.${normalized}`);
  }
  return `${path}.${normalized}`;
}

function safeArray<T>(value: T[] | null | undefined): T[] {
  return Array.isArray(value) ? value : [];
}

function autoValidationMessage(validation: ContentWorkspaceValidation) {
  const format = normalizedOfficeFormat(validation.renderFormat);
  if (format === "pptx") {
    return `自动校验：${validation.acceptanceSummary.plannedPageCount} 个计划页`;
  }
  return `自动校验：${format.toUpperCase()} / ${validation.selectedRecipe ?? "-"}`;
}

function acceptanceMessage(
  summary: AcceptanceSummary,
  formatValue?: string | null,
  recipe?: string | null,
) {
  const format = normalizedOfficeFormat(formatValue);
  if (summary.status === "fail") {
    return `验收失败：${summary.schemaErrorCount} 个协议问题`;
  }
  if (summary.canRender && summary.status === "warn") {
    if (format !== "pptx") {
      return `可生成但需复核：${format.toUpperCase()} / ${recipe ?? "-"}`;
    }
    return `可生成但需复核：${summary.plannedPageCount} 页，${summary.bindingWarningCount} 个绑定提示`;
  }
  if (summary.canRender) {
    if (format !== "pptx") {
      return `验收通过：${format.toUpperCase()} / ${recipe ?? "-"}`;
    }
    return `验收通过：${summary.plannedPageCount} 页`;
  }
  return "验收未通过：缺少模板结构或存在阻断项";
}

function acceptanceStatusLabel(status: AcceptanceSummary["status"]) {
  if (status === "pass") {
    return "通过";
  }
  if (status === "warn") {
    return "提示";
  }
  return "失败";
}

function acceptanceStatusClass(status: AcceptanceSummary["status"]) {
  if (status === "pass") {
    return "is-ok";
  }
  if (status === "warn") {
    return "is-warn";
  }
  return "is-danger";
}

function statusLabel(status: RunState["status"]) {
  if (status === "running") {
    return "运行中";
  }
  if (status === "ok") {
    return "有效";
  }
  if (status === "error") {
    return "错误";
  }
  return "就绪";
}
