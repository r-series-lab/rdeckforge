import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import {
  ArrowDown,
  Braces,
  CheckCircle2,
  Circle,
  Copy,
  ExternalLink,
  FileText,
  FileOutput,
  FolderOpen,
  LoaderCircle,
  Presentation,
  RotateCcw,
  Settings2,
  Table2,
  TriangleAlert,
} from "lucide-react";
import { ConfigDialog } from "@/components/ConfigDialog";
import { TemplatePicker } from "@/components/TemplatePicker";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  firstRecipe,
  groupTemplateFamilies,
  recipeIdsForTemplate,
  templateDocumentRole,
  templateRoleLabel,
  templateSupportsContent,
  templateVariantSlug,
  type TemplateFamily,
  type TemplatePackRecord,
} from "@/lib/template-packs";
import { writeWorkflowDraft, type WorkflowDraft } from "@/lib/workflow-draft";
import {
  readOutputPreferences,
  suggestOutputPath,
  type OutputSuggestionSource,
} from "@/lib/output-preferences";

const browserPreviewRoot = "/preview";

type OfficeFormat = "pptx" | "docx" | "xlsx";

type RenderForm = {
  template: string;
  recipe: string;
  input: string;
  output: string;
  format: OfficeFormat;
};

type RunState = {
  status: "idle" | "running" | "ok" | "error";
  message: string;
  data?: unknown;
};

type BatchOutput = {
  templateId: string;
  templateName: string;
  outputFile: string;
  status: "ok" | "error";
  message?: string;
};

type OutputPathResolution = {
  requestedPath: string;
  resolvedPath: string;
  conflictPolicy: string;
  renamed: boolean;
};

type OutputPreviewState = {
  status: "idle" | "ok" | "renamed" | "overwrite" | "missing" | "error";
  message: string;
  requestedPath: string;
  resolvedPath: string;
  source: OutputSuggestionSource;
};

type RenderPreflightItem = {
  key: "template" | "content" | "output" | "structure" | "preflight";
  label: string;
  state: "ready" | "warning" | "missing" | "idle";
  detail: string;
  actionLabel: string;
  onAction: () => void;
  disabled?: boolean;
};

type PreflightGuidanceTone = "idle" | "ready" | "warning" | "danger";

type PreflightGuidance = {
  tone: PreflightGuidanceTone;
  title: string;
  detail: string;
  actionLabel: string;
  onAction: () => void;
  disabled?: boolean;
  metrics: Array<{
    label: string;
    value: string;
    tone?: PreflightGuidanceTone;
  }>;
};

type PreflightRepairItem = {
  key: string;
  tone: "warning" | "danger";
  title: string;
  detail: string;
  actionLabel: string;
  onAction: () => void;
};

export type RenderInspectorState = {
  templateName: string;
  templatePath: string;
  inputPath: string;
  outputPath: string;
  status: RunState["status"];
  message: string;
  outputFile?: string | null;
  plannedPageCount?: number;
  visibleSlideCount?: number;
  checkedBindings?: number;
  missingBindings?: number;
  templateCount?: number;
  rendererLabel?: string;
  contentFormat?: string;
  outputFormat?: OfficeFormat;
  sheetCount?: number;
  integrityStatus?: string;
  integrityEntryCount?: number;
  integrityRelationshipCount?: number;
  integrityFileSize?: number;
};

const defaultForm: RenderForm = {
  template: "",
  recipe: "",
  input: "",
  output: "",
  format: "pptx",
};

const browserPreviewTemplate: TemplatePackRecord = {
  id: "browser-preview-template",
  path: `${browserPreviewRoot}/templates/demo-medical-teaching-v1`,
  pathAvailable: true,
  templateId: "demo-teaching-style-classic-v1",
  name: "教学演示 · 经典样式",
  format: "pptx",
  templateType: "declarative",
  entryPath: "template.pptx",
  pipelineStepCount: 0,
  pageTemplateCount: 10,
  deckRecipeCount: 1,
  blockTemplateCount: 0,
  documentRecipeCount: 0,
  sheetTemplateCount: 0,
  workbookRecipeCount: 0,
  pageTemplateIds: ["cover", "goals", "chapter", "summary"],
  deckRecipeIds: ["teaching_deck"],
  blockTemplateIds: [],
  documentRecipeIds: [],
  sheetTemplateIds: [],
  workbookRecipeIds: [],
  legacyLayoutCount: 0,
  inputFormats: ["json", "md"],
  inputSchemaId: "teaching_deck_v1",
  warnings: [],
  linkedAt: "2026-06-29T00:00:00Z",
  lastValidatedAt: "2026-06-29T00:00:00Z",
};

const browserPreviewTemplates: TemplatePackRecord[] = [
  browserPreviewTemplate,
  {
    ...browserPreviewTemplate,
    id: "browser-preview-pptx-cards",
    path: `${browserPreviewRoot}/templates/pptx-cards`,
    templateId: "demo-teaching-style-cards-v1",
    name: "教学演示 · 现代卡片式",
  },
  {
    ...browserPreviewTemplate,
    id: "browser-preview-pptx-editorial",
    path: `${browserPreviewRoot}/templates/pptx-editorial`,
    templateId: "demo-teaching-style-editorial-v1",
    name: "教学演示 · 简洁刊物式",
  },
  {
    ...browserPreviewTemplate,
    id: "browser-preview-docx-template",
    path: `${browserPreviewRoot}/templates/docx-template`,
    templateId: "browser-preview-docx-template",
    name: "Demo Design Document Template",
    format: "docx",
    entryPath: "template.docx",
    pageTemplateCount: 0,
    deckRecipeCount: 0,
    pageTemplateIds: [],
    deckRecipeIds: [],
    blockTemplateCount: 3,
    documentRecipeCount: 1,
    blockTemplateIds: ["heading", "paragraph", "table"],
    documentRecipeIds: ["standard_document"],
  },
  {
    ...browserPreviewTemplate,
    id: "browser-preview-xlsx-template",
    path: `${browserPreviewRoot}/templates/xlsx-template`,
    templateId: "browser-preview-xlsx-template",
    name: "Demo Assessment Workbook Template",
    format: "xlsx",
    entryPath: "template.xlsx",
    pageTemplateCount: 0,
    deckRecipeCount: 0,
    pageTemplateIds: [],
    deckRecipeIds: [],
    sheetTemplateCount: 1,
    workbookRecipeCount: 1,
    sheetTemplateIds: ["assessment"],
    workbookRecipeIds: ["standard_workbook"],
  },
];

type RenderPageProps = {
  initialDraft?: WorkflowDraft | null;
  onWorkflowDraft?: (draft: WorkflowDraft) => void;
  onInspectorChange?: (state: RenderInspectorState) => void;
};

export function RenderPage({ initialDraft, onWorkflowDraft, onInspectorChange }: RenderPageProps) {
  const [form, setForm] = useState<RenderForm>(() => formFromDraft(initialDraft, defaultForm));
  const [configOpen, setConfigOpen] = useState(false);
  const [activeStep, setActiveStep] = useState(1);
  const [templates, setTemplates] = useState<TemplatePackRecord[]>([]);
  const [templateMessage, setTemplateMessage] = useState("模板库尚未载入。");
  const [autoOutputPath, setAutoOutputPath] = useState(false);
  const [autoOutputSource, setAutoOutputSource] = useState<OutputSuggestionSource>("none");
  const [outputPreview, setOutputPreview] = useState<OutputPreviewState>({
    status: "idle",
    message: "选择内容后自动建议输出路径。",
    requestedPath: "",
    resolvedPath: "",
    source: "none",
  });
  const [runState, setRunState] = useState<RunState>({
    status: "idle",
    message: "可以先预检，也可以直接生成 Office 文档。",
  });
  const appliedDraftKey = useRef(initialDraft ? draftKey(initialDraft) : "");
  const outputPreviewRequest = useRef(0);

  const result = runState.data as RenderResultEnvelope | undefined;
  const workflowValidation = result?.validation;
  const sidecarData = pptxSidecarData(result);
  const outputFile = renderOutputFile(result);
  const outputIntegrity = renderOutputIntegrity(result);
  const acceptanceSummary =
    result?.acceptanceSummary ??
    workflowValidation?.acceptanceSummary ??
    result?.render?.acceptanceSummary;
  const repairHints = repairHintsFromResult(result);
  const contentCheck = sidecarData?.contentCheck;
  const cleanup = sidecarData?.cleanup;
  const sheetCount = renderSheetCount(result);
  const isRunning = runState.status === "running";
  const templateFamilies = useMemo(() => groupTemplateFamilies(templates), [templates]);
  const selectedTemplate = useMemo(
    () => templates.find((template) => template.path === form.template),
    [form.template, templates],
  );
  const selectedFamily = useMemo(
    () =>
      selectedTemplate
        ? templateFamilies.find((family) =>
            family.templates.some((template) => template.id === selectedTemplate.id),
          )
        : undefined,
    [selectedTemplate, templateFamilies],
  );
  const activeFormat = normalizedOfficeFormat(selectedTemplate?.format ?? form.format);
  const compatibleTemplates = useMemo(
    () =>
      selectedFamily?.templates.filter(
        (template) =>
          template.format === activeFormat &&
          templateDocumentRole(template) === (selectedTemplate ? templateDocumentRole(selectedTemplate) : "") &&
          templateSupportsContent(template, form.input),
      ) ??
      [],
    [activeFormat, form.input, selectedFamily, selectedTemplate],
  );
  const recipeOptions = selectedTemplate ? recipeIdsForTemplate(selectedTemplate) : [];
  const selectedTemplateUsesScript = usesScriptRenderer(selectedTemplate);
  const resultPreview = useMemo(
    () => (runState.data ? JSON.stringify(runState.data, null, 2) : ""),
    [runState.data],
  );
  const batchOutputs = result?.batchOutputs ?? [];
  const renderedUnitCount =
    activeFormat === "pptx"
      ? cleanup?.visibleSlideCount ?? sidecarData?.plannedPageCount ?? acceptanceSummary?.plannedPageCount
      : activeFormat === "xlsx"
        ? sheetCount
        : undefined;
  const inspectorState = useMemo<RenderInspectorState>(
    () => ({
      templateName: selectedTemplate?.name ?? fileName(form.template) ?? "尚未选择",
      templatePath: form.template,
      inputPath: form.input,
      outputPath: form.output,
      status: runState.status,
      message: runState.message,
      outputFile,
      plannedPageCount:
        activeFormat === "pptx"
          ? sidecarData?.plannedPageCount ?? acceptanceSummary?.plannedPageCount ?? undefined
          : undefined,
      visibleSlideCount: activeFormat === "pptx" ? cleanup?.visibleSlideCount : undefined,
      checkedBindings: contentCheck?.checkedBindings,
      missingBindings: contentCheck?.missing.length,
      templateCount: templates.length,
      rendererLabel: `${activeFormat.toUpperCase()} · ${selectedTemplateUsesScript ? "脚本型模板" : "声明式模板"}`,
      contentFormat: contentFormatLabel(form.input),
      outputFormat: activeFormat,
      sheetCount,
      integrityStatus: outputIntegrity?.status,
      integrityEntryCount: outputIntegrity?.entryCount,
      integrityRelationshipCount: outputIntegrity?.relationshipCount,
      integrityFileSize: outputIntegrity?.fileSize,
    }),
    [
      activeFormat,
      acceptanceSummary?.plannedPageCount,
      cleanup?.visibleSlideCount,
      contentCheck?.checkedBindings,
      contentCheck?.missing.length,
      form.input,
      form.output,
      form.template,
      outputFile,
      outputIntegrity?.entryCount,
      outputIntegrity?.fileSize,
      outputIntegrity?.relationshipCount,
      outputIntegrity?.status,
      runState.message,
      runState.status,
      selectedTemplate?.name,
      selectedTemplateUsesScript,
      sheetCount,
      sidecarData?.plannedPageCount,
      templates.length,
    ],
  );

  useEffect(() => {
    void loadTemplates(true);
  }, []);

  useEffect(() => {
    onInspectorChange?.(inspectorState);
  }, [inspectorState, onInspectorChange]);

  useEffect(() => {
    if (!initialDraft) {
      return;
    }
    const key = draftKey(initialDraft);
    if (appliedDraftKey.current === key) {
      return;
    }
    appliedDraftKey.current = key;
    const format = normalizedOfficeFormat(initialDraft.format);
    setForm((current) => ({
      ...current,
      template: initialDraft.templatePath,
      recipe: initialDraft.recipe ?? current.recipe,
      input: initialDraft.contentPath || current.input,
      output: withOfficeExtension(initialDraft.outputPath ?? current.output, format),
      format,
    }));
    setRunState({
      status: "idle",
      message:
        initialDraft.source === "template-library"
          ? `已选择模板，请继续选择内容文件后生成 ${format.toUpperCase()}。`
          : initialDraft.source === "restored-session"
            ? `已恢复上次任务，可以继续预检或生成 ${format.toUpperCase()}。`
            : `已带入刚保存的 AI 输出，可以预检或生成 ${format.toUpperCase()}。`,
    });
  }, [initialDraft]);

  useEffect(() => {
    const timeout = window.setTimeout(() => {
      publishWorkflowDraft({
        status: form.input ? "content_ready" : "draft",
        stage: "render",
        validationStatus: null,
        lastError: null,
      });
    }, 350);
    return () => window.clearTimeout(timeout);
  }, [
    form,
    initialDraft?.briefPath,
    initialDraft?.promptPath,
    initialDraft?.taskId,
    initialDraft?.taskName,
    onWorkflowDraft,
  ]);

  useEffect(() => {
    if (!selectedTemplate) {
      return;
    }
    if (selectedTemplateUsesScript && form.recipe) {
      setForm((current) => ({ ...current, recipe: "" }));
      return;
    }
    if (!selectedTemplateUsesScript && recipeOptions.length > 0 && !recipeOptions.includes(form.recipe)) {
      setForm((current) => ({ ...current, recipe: firstRecipe(selectedTemplate) }));
    }
  }, [form.recipe, recipeOptions, selectedTemplate, selectedTemplateUsesScript]);

  useEffect(() => {
    if (!form.input.trim() || (!form.template.trim() && !selectedTemplate)) {
      return;
    }
    const suggestion = suggestOutputPath(readOutputPreferences(), {
      contentPath: form.input,
      templateName: selectedTemplate?.name ?? fileName(form.template),
      format: activeFormat,
    });
    if (!suggestion.path) {
      return;
    }
    const output = withOfficeExtension(suggestion.path, activeFormat);
    setForm((current) => {
      if (current.output.trim() && !autoOutputPath) {
        return current;
      }
      if (current.output === output && current.format === activeFormat) {
        return current;
      }
      return { ...current, output, format: activeFormat };
    });
    setAutoOutputPath(true);
    setAutoOutputSource(suggestion.source);
  }, [
    activeFormat,
    autoOutputPath,
    form.input,
    form.output,
    form.template,
    selectedTemplate,
  ]);

  useEffect(() => {
    const path = withOfficeExtension(form.output, activeFormat);
    const source = autoOutputPath ? autoOutputSource : "none";
    if (!path.trim()) {
      setOutputPreview({
        status: "missing",
        message: form.input.trim()
          ? "选择模板后会自动建议输出路径。"
          : "选择内容后会自动建议输出路径。",
        requestedPath: "",
        resolvedPath: "",
        source,
      });
      return;
    }
    const preferences = readOutputPreferences();
    const requestId = ++outputPreviewRequest.current;
    void resolveOutputPath(path, preferences.conflictPolicy)
      .then((resolution) => {
        if (requestId !== outputPreviewRequest.current) {
          return;
        }
        const renamed = resolution.renamed;
        const overwrite = resolution.conflictPolicy === "overwrite";
        setOutputPreview({
          status: renamed ? "renamed" : overwrite ? "overwrite" : "ok",
          message: outputPreviewMessage(resolution, source),
          requestedPath: resolution.requestedPath,
          resolvedPath: resolution.resolvedPath,
          source,
        });
      })
      .catch((error) => {
        if (requestId !== outputPreviewRequest.current) {
          return;
        }
        setOutputPreview({
          status: "error",
          message: error instanceof Error ? error.message : String(error),
          requestedPath: path,
          resolvedPath: path,
          source,
        });
      });
  }, [activeFormat, autoOutputPath, autoOutputSource, form.input, form.output, form.template]);

  function updateField(field: Exclude<keyof RenderForm, "format">, value: string) {
    if (field === "output") {
      setAutoOutputPath(false);
      setAutoOutputSource("none");
    }
    setForm((current) => ({ ...current, [field]: value }));
  }

  function updateFormat(value: string) {
    const format = normalizedOfficeFormat(value);
    setForm((current) => ({
      ...current,
      format,
      recipe: selectedTemplate ? firstRecipe(selectedTemplate) : current.recipe,
      output: withOfficeExtension(current.output, format),
    }));
  }

  async function loadTemplates(silent = false) {
    if (!("__TAURI_INTERNALS__" in window)) {
      setTemplates(browserPreviewTemplates);
      setTemplateMessage("预览模式：PPTX、DOCX、XLSX 示例结构已就绪。");
      return;
    }
    try {
      const data = await invoke<TemplatePackRecord[]>("list_template_packs");
      setTemplates(data);
      const formatCounts = countTemplateFormats(data);
      setTemplateMessage(
        `已链接 ${data.length} 个模板包：PPTX ${formatCounts.pptx}、DOCX ${formatCounts.docx}、XLSX ${formatCounts.xlsx}。`,
      );
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setTemplateMessage(message);
      if (!silent) {
        setRunState({ status: "error", message });
      }
    }
  }

  function selectRegisteredTemplate(record: TemplatePackRecord | null) {
    if (!record) {
      setForm((current) => ({ ...current, template: "", recipe: "" }));
      return;
    }
    if (!record.pathAvailable) {
      setRunState({ status: "error", message: "模板目录已失效，请先到模板库重新定位。" });
      return;
    }
    const scriptRenderer = usesScriptRenderer(record);
    const format = normalizedOfficeFormat(record.format);
    const availableRecipes = recipeIdsForTemplate(record);
    setForm((current) => ({
      ...current,
      template: record.path,
      recipe: scriptRenderer
        ? ""
        : availableRecipes.includes(current.recipe)
          ? current.recipe
          : firstRecipe(record),
      output: withOfficeExtension(current.output, format),
      format,
    }));
    setRunState({
      status: "idle",
      message: `已选择 ${format.toUpperCase()} 模板，可以预检或直接生成。`,
    });
  }

  async function chooseTemplateFolder() {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "选择模板包目录",
      defaultPath: form.template,
    });
    if (typeof selected === "string") {
      updateField("template", selected);
    }
  }

  async function chooseContentJson() {
    const selected = await open({
      multiple: false,
      title: "选择内容文件",
      defaultPath: form.input,
      filters: [{ name: "内容文件", extensions: ["json", "md", "markdown"] }],
    });
    if (typeof selected === "string") {
      updateField("input", selected);
    }
  }

  async function chooseOutputOffice() {
    const selected = await save({
      title: `选择输出 ${activeFormat.toUpperCase()}`,
      defaultPath: withOfficeExtension(form.output, activeFormat),
      filters: [{ name: officeFormatName(activeFormat), extensions: [activeFormat] }],
    });
    if (typeof selected === "string") {
      setAutoOutputPath(false);
      updateField("output", selected);
    }
  }

  function applyOutputRule() {
    const preferences = readOutputPreferences();
    const suggestion = suggestOutputPath(preferences, {
      contentPath: form.input,
      templateName: selectedTemplate?.name ?? fileName(form.template),
      format: activeFormat,
    });
    const output = suggestion.path;
    if (!output) {
      setRunState({ status: "error", message: "请先选择内容文件，或在设置中选择默认输出目录。" });
      return;
    }
    setForm((current) => ({ ...current, output, format: activeFormat }));
    setAutoOutputPath(true);
    setAutoOutputSource(suggestion.source);
    setRunState({ status: "idle", message: `已按${outputSourceLabel(suggestion.source)}生成文件路径。` });
  }

  async function dryRun() {
    setRunState({ status: "running", message: `正在预检 ${activeFormat.toUpperCase()} 任务` });
    try {
      const data = await invoke<RenderResultEnvelope>("inspect_office_workflow", {
        input: form.input,
        template: form.template,
        recipe: selectedTemplateUsesScript ? null : form.recipe || null,
      });
      const summary = data.validation?.acceptanceSummary ?? data.acceptanceSummary;
      setRunState({
        status: summary?.canRender ? "ok" : "error",
        message: summary
          ? acceptanceMessage(summary, activeFormat, data.validation?.selectedRecipe ?? data.selectedRecipe ?? form.recipe)
          : "预检完成",
        data,
      });
      publishWorkflowDraft({
        status: summary?.canRender ? "validated" : "error",
        stage: summary?.canRender ? "render" : "validation",
        validationStatus: summary?.status ?? null,
        lastError: summary?.canRender ? null : summary?.message ?? "预检未通过",
      });
    } catch (error) {
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

  async function renderOffice() {
    const preferences = readOutputPreferences();
    const requestedOutput = withOfficeExtension(form.output, activeFormat);
    setRunState({ status: "running", message: `正在生成 ${activeFormat.toUpperCase()}` });
    try {
      const output = await resolveRequestedOutput(requestedOutput, preferences.conflictPolicy);
      setForm((current) => ({ ...current, output }));
      const data = await invoke<RenderResultEnvelope>("run_office_workflow", {
        input: form.input,
        template: form.template,
        recipe: selectedTemplateUsesScript ? null : form.recipe || null,
        output,
      });
      if (data.stage === "validate" && !data.render) {
        const summary = data.validation?.acceptanceSummary;
        setRunState({
          status: "error",
          message: summary?.message ? `预检未通过：${summary.message}` : "预检未通过，请处理修复清单。",
          data,
        });
        publishWorkflowDraft({
          status: "error",
          stage: "validation",
          validationStatus: summary?.status ?? "fail",
          lastError: summary?.message ?? "预检未通过",
        });
        return;
      }
      publishWorkflowDraft({
        outputPath: renderOutputFile(data) ?? output,
        status: "rendered",
        stage: "render",
        validationStatus: data.render?.acceptanceSummary?.status ?? data.validation?.acceptanceSummary.status ?? "pass",
        lastError: null,
      });
      setRunState({ status: "ok", message: renderMessage(`正在生成 ${activeFormat.toUpperCase()}`, data), data });
      if (preferences.autoOpen) {
        void invoke("open_path", { path: renderOutputFile(data) ?? output }).catch(() => undefined);
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

  async function renderTemplateFamily() {
    if (!selectedTemplate || !selectedFamily || compatibleTemplates.length < 2) {
      return;
    }

    const outputs: BatchOutput[] = [];
    setRunState({
      status: "running",
      message: `正在生成 1/${compatibleTemplates.length}：${compatibleTemplates[0].name}`,
    });

    const preferences = readOutputPreferences();
    for (const [index, template] of compatibleTemplates.entries()) {
      const format = normalizedOfficeFormat(template.format);
      const requestedOutput = batchOutputPath(form.output, format, template, selectedFamily);
      let output = requestedOutput;
      setRunState({
        status: "running",
        message: `正在生成 ${index + 1}/${compatibleTemplates.length}：${template.name}`,
        data: { batchOutputs: outputs },
      });
      try {
        output = await resolveRequestedOutput(requestedOutput, preferences.conflictPolicy);
        const data = await invoke<RenderResultEnvelope>("render_accepted_office", {
          input: form.input,
          template: template.path,
          recipe: recipeForBatchTemplate(template, form.recipe),
          output,
        });
        outputs.push({
          templateId: template.templateId,
          templateName: template.name,
          outputFile: renderOutputFile(data) ?? output,
          status: "ok",
        });
      } catch (error) {
        outputs.push({
          templateId: template.templateId,
          templateName: template.name,
          outputFile: output,
          status: "error",
          message: error instanceof Error ? error.message : String(error),
        });
      }
    }

    const successes = outputs.filter((output) => output.status === "ok");
    const failures = outputs.length - successes.length;
    const selectedOutput = successes.find((output) => output.templateId === selectedTemplate.templateId);
    setRunState({
      status: failures > 0 ? "error" : "ok",
      message:
        failures > 0
          ? `批量生成完成：成功 ${successes.length} 个，失败 ${failures} 个`
          : `已生成 ${successes.length} 种兼容样式`,
      data: {
        outputFile: selectedOutput?.outputFile ?? successes[0]?.outputFile,
        batchOutputs: outputs,
      } satisfies RenderResultEnvelope,
    });
    publishWorkflowDraft({
      outputPath: selectedOutput?.outputFile ?? successes[0]?.outputFile ?? form.output,
      status: failures > 0 ? "error" : "rendered",
      stage: "render",
      validationStatus: failures > 0 ? "warn" : "pass",
      lastError: failures > 0 ? `${failures} 个样式生成失败` : null,
    });
    if (preferences.autoOpen && successes.length > 0) {
      void invoke("open_path", { path: selectedOutput?.outputFile ?? successes[0].outputFile }).catch(
        () => undefined,
      );
    }
  }

  async function openOutputFile() {
    if (!outputFile) {
      return;
    }
    await runResultAction("正在打开文件", async () =>
      invoke("open_path", { path: outputFile }),
    );
  }

  async function revealOutputFile() {
    if (!outputFile) {
      return;
    }
    await runResultAction("正在定位文件", async () =>
      invoke("reveal_path", { path: outputFile }),
    );
  }

  async function copyOutputPath() {
    if (!outputFile) {
      return;
    }
    await runResultAction("正在复制路径", async () => {
      if (!navigator.clipboard?.writeText) {
        throw new Error("当前 WebView 不支持剪贴板。");
      }
      await navigator.clipboard.writeText(outputFile);
    });
  }

  async function openBatchOutput(path: string) {
    await runResultAction("正在打开文件", async () => invoke("open_path", { path }));
  }

  function continueSameTemplate() {
    setForm((current) => ({ ...current, input: "", output: "" }));
    setAutoOutputPath(false);
    setAutoOutputSource("none");
    setOutputPreview({
      status: "missing",
      message: "选择内容后会自动建议输出路径。",
      requestedPath: "",
      resolvedPath: "",
      source: "none",
    });
    setRunState({
      status: "idle",
      message: `已保留${selectedTemplate?.name ? `「${selectedTemplate.name}」` : "当前"}模板，请选择下一份内容。`,
    });
    setActiveStep(2);
  }

  async function runAction(label: string, action: () => Promise<unknown>) {
    setRunState({ status: "running", message: label });
    try {
      const data = await action();
      setRunState({ status: "ok", message: renderMessage(label, data), data });
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
      | "outputPath"
      | "status"
      | "stage"
      | "validationStatus"
      | "lastError"
    >>,
  ) {
    if (!initialDraft?.taskId && !form.template.trim() && !form.input.trim() && !form.output.trim()) {
      return;
    }
    const draft: WorkflowDraft = {
      taskId: initialDraft?.taskId,
      taskName: initialDraft?.taskName,
      templatePath: form.template,
      contentPath: form.input,
      recipe: form.recipe || null,
      format: form.format,
      outputPath: form.output || null,
      briefPath: initialDraft?.briefPath ?? null,
      promptPath: initialDraft?.promptPath ?? null,
      validationStatus: initialDraft?.validationStatus ?? null,
      lastError: null,
      status: initialDraft?.status ?? (form.input ? "content_ready" : "draft"),
      stage: "render",
      source: "render-workflow",
      updatedAt: Date.now(),
      ...patch,
    };
    writeWorkflowDraft(draft);
    onWorkflowDraft?.(draft);
  }

  async function runResultAction(label: string, action: () => Promise<unknown>) {
    setRunState((current) => ({ ...current, status: "running", message: label }));
    try {
      await action();
      setRunState((current) => ({ ...current, status: "ok", message: `${label}完成` }));
    } catch (error) {
      setRunState((current) => ({
        ...current,
        status: "error",
        message: error instanceof Error ? error.message : String(error),
      }));
    }
  }

  async function resolveRequestedOutput(path: string, conflictPolicy: string) {
    const resolution = await resolveOutputPath(path, conflictPolicy);
    return resolution.resolvedPath;
  }

  async function resolveOutputPath(path: string, conflictPolicy: string): Promise<OutputPathResolution> {
    if (!("__TAURI_INTERNALS__" in window)) {
      return {
        requestedPath: path,
        resolvedPath: path,
        conflictPolicy: conflictPolicy === "overwrite" ? "overwrite" : "increment",
        renamed: false,
      };
    }
    const resolution = await invoke<OutputPathResolution>("resolve_output_path", {
      path,
      conflictPolicy,
    });
    return resolution;
  }

  const templateReady = Boolean(form.template);
  const contentReady = Boolean(form.input);
  const outputReady = Boolean(form.output);
  const canValidate = templateReady && contentReady;
  const taskReady = templateReady && contentReady && outputReady;
  const structureCount = selectedTemplate
    ? activeFormat === "docx"
      ? selectedTemplate.blockTemplateCount
      : activeFormat === "xlsx"
        ? selectedTemplate.sheetTemplateCount
        : selectedTemplate.pageTemplateCount
    : 0;
  const recipeCount = selectedTemplate
    ? activeFormat === "docx"
      ? selectedTemplate.documentRecipeCount
      : activeFormat === "xlsx"
        ? selectedTemplate.workbookRecipeCount
        : selectedTemplate.deckRecipeCount
    : 0;
  const structureIds = selectedTemplate
    ? activeFormat === "docx"
      ? selectedTemplate.blockTemplateIds
      : activeFormat === "xlsx"
        ? selectedTemplate.sheetTemplateIds
        : selectedTemplate.pageTemplateIds
        : [];
  const preflightItems: RenderPreflightItem[] = [
    {
      key: "template",
      label: "模板",
      state: templateReady ? "ready" : "missing",
      detail: selectedTemplate?.name ?? (form.template ? "手动路径待预检" : "请选择模板包"),
      actionLabel: "选择",
      onAction: () => setActiveStep(1),
    },
    {
      key: "content",
      label: "内容",
      state: contentReady ? "ready" : "missing",
      detail: fileName(form.input) || "请选择 AI 输出文件",
      actionLabel: "选择",
      onAction: () => setActiveStep(2),
    },
    {
      key: "output",
      label: "输出",
      state:
        !outputReady || outputPreview.status === "error"
          ? "warning"
          : outputPreview.status === "renamed" || outputPreview.status === "overwrite"
            ? "warning"
            : "ready",
      detail: outputReady ? outputPreview.message : "可先预检，生成前补齐输出路径",
      actionLabel: contentReady ? "按规则" : "设置",
      onAction: () => {
        if (contentReady) {
          applyOutputRule();
          return;
        }
        setActiveStep(3);
      },
    },
    {
      key: "structure",
      label: "结构",
      state: selectedTemplateUsesScript || form.recipe || recipeOptions.length === 0 ? "ready" : "warning",
      detail: selectedTemplateUsesScript ? "脚本自动处理" : form.recipe || structureFallbackLabel(activeFormat),
      actionLabel: "配置",
      onAction: () => setConfigOpen(true),
    },
    {
      key: "preflight",
      label: "预检",
      state:
        runState.status === "ok" && runState.data
          ? "ready"
          : runState.status === "error"
            ? "missing"
            : "idle",
      detail:
        runState.status === "ok" && runState.data
          ? "已有可用预检/生成结果"
          : runState.status === "error"
            ? "查看错误并重新预检"
            : canValidate
              ? "建议生成前先跑一次"
              : "补齐模板和内容后可预检",
      actionLabel: "预检",
      onAction: () => void dryRun(),
      disabled: !canValidate,
    },
  ];
  const preflightGuidance: PreflightGuidance = (() => {
    const readinessMetrics = [
      {
        label: "模板",
        value: templateReady ? "可用" : "待选择",
        tone: templateReady ? "ready" : "danger",
      },
      {
        label: "内容",
        value: contentReady ? contentFormatLabel(form.input) : "待选择",
        tone: contentReady ? "ready" : "danger",
      },
      {
        label: "输出",
        value: outputReady ? outputPreviewStatusLabel(outputPreview.status) : "待设置",
        tone:
          outputReady && outputPreview.status !== "error"
            ? outputPreview.status === "renamed" || outputPreview.status === "overwrite"
              ? "warning"
              : "ready"
            : "danger",
      },
    ] satisfies PreflightGuidance["metrics"];
    const plannedUnits =
      acceptanceSummary?.plannedPageCount ?? sidecarData?.plannedPageCount ?? renderedUnitCount;
    const checkedBindings = contentCheck?.checkedBindings;
    const missingBindings = contentCheck?.missing.length;
    const resultMetrics = [
      {
        label: renderedUnitLabel(activeFormat),
        value: plannedUnits ? String(plannedUnits) : "待计算",
        tone: plannedUnits ? "ready" : "idle",
      },
      {
        label: "内容绑定",
        value: typeof checkedBindings === "number" ? String(checkedBindings) : "已检查",
        tone: "ready",
      },
      {
        label: "缺失",
        value: typeof missingBindings === "number" ? String(missingBindings) : "0",
        tone: missingBindings ? "danger" : "ready",
      },
      {
        label: "输出",
        value: outputPreviewStatusLabel(outputPreview.status),
        tone:
          outputPreview.status === "renamed" || outputPreview.status === "overwrite"
            ? "warning"
            : outputPreview.status === "error"
              ? "danger"
              : "ready",
      },
    ] satisfies PreflightGuidance["metrics"];

    if (runState.status === "running") {
      return {
        tone: "warning",
        title: "正在处理",
        detail: runState.message,
        actionLabel: "处理中",
        onAction: () => undefined,
        disabled: true,
        metrics: readinessMetrics,
      };
    }
    if (runState.status === "error") {
      const action =
        !templateReady
          ? { label: "选择模板", run: () => setActiveStep(1) }
          : !contentReady
            ? { label: "选择内容", run: () => setActiveStep(2) }
            : !outputReady
              ? { label: "设置输出", run: () => setActiveStep(3) }
              : { label: "回到内容", run: () => setActiveStep(2) };
      return {
        tone: "danger",
        title: acceptanceSummary?.canRender === false ? "预检未通过" : "需要处理问题",
        detail: acceptanceSummary?.message || runState.message,
        actionLabel: action.label,
        onAction: action.run,
        metrics: readinessMetrics,
      };
    }
    if (runState.status === "ok" && runState.data) {
      const hasWarning = acceptanceSummary?.status === "warn" || Boolean(missingBindings);
      return {
        tone: hasWarning ? "warning" : "ready",
        title: hasWarning ? "可生成，但建议确认" : "预检通过",
        detail:
          hasWarning && acceptanceSummary?.message
            ? acceptanceSummary.message
            : hasWarning
              ? "存在提示项，确认后仍可生成。"
              : `内容与模板匹配，可以生成 ${activeFormat.toUpperCase()}。`,
        actionLabel: `生成 ${activeFormat.toUpperCase()}`,
        onAction: () => void renderOffice(),
        disabled: !taskReady || isRunning,
        metrics: resultMetrics,
      };
    }
    if (!templateReady) {
      return {
        tone: "idle",
        title: "先选择模板",
        detail: "模板决定输出格式、结构和内容协议。",
        actionLabel: "选择模板",
        onAction: () => setActiveStep(1),
        metrics: readinessMetrics,
      };
    }
    if (!contentReady) {
      return {
        tone: "idle",
        title: "选择内容文件",
        detail: "选择 AI 输出的 JSON 或 Markdown 后即可预检。",
        actionLabel: "选择内容",
        onAction: () => setActiveStep(2),
        metrics: readinessMetrics,
      };
    }
    if (!outputReady || outputPreview.status === "error") {
      return {
        tone: "warning",
        title: "补齐输出路径",
        detail: outputPreview.message,
        actionLabel: outputReady ? "设置输出" : "按规则生成",
        onAction: outputReady ? () => setActiveStep(3) : applyOutputRule,
        metrics: readinessMetrics,
      };
    }
    return {
      tone:
        outputPreview.status === "renamed" || outputPreview.status === "overwrite"
          ? "warning"
          : "idle",
      title:
        outputPreview.status === "renamed"
          ? "同名文件会自动重命名"
          : outputPreview.status === "overwrite"
            ? "同名文件会覆盖"
            : "建议先预检",
      detail:
        outputPreview.status === "renamed" || outputPreview.status === "overwrite"
          ? outputPreview.message
          : "确认内容结构和模板协议匹配后再生成。",
      actionLabel: "预检任务",
      onAction: () => void dryRun(),
      disabled: !canValidate,
      metrics: readinessMetrics,
    };
  })();
  const preflightRepairItems: PreflightRepairItem[] = (() => {
    const items: PreflightRepairItem[] = [];
    if (!templateReady) {
      items.push({
        key: "template-missing",
        tone: "danger",
        title: "缺少模板",
        detail: "先选择一个模板包，才能校验内容结构。",
        actionLabel: "选择模板",
        onAction: () => setActiveStep(1),
      });
    }
    if (!contentReady) {
      items.push({
        key: "content-missing",
        tone: "danger",
        title: "缺少内容",
        detail: "选择 AI 输出的 JSON 或 Markdown 文件后再预检。",
        actionLabel: "选择内容",
        onAction: () => setActiveStep(2),
      });
    }
    if (!outputReady || outputPreview.status === "error") {
      items.push({
        key: "output-missing",
        tone: outputReady ? "danger" : "warning",
        title: outputReady ? "输出路径不可用" : "输出路径未设置",
        detail: outputPreview.message,
        actionLabel: outputReady ? "设置输出" : "按规则生成",
        onAction: outputReady ? () => setActiveStep(3) : applyOutputRule,
      });
    }
    for (const [index, hint] of repairHints.slice(0, 6).entries()) {
      items.push({
        key: `repair-hint-${index}`,
        tone: hint.severity === "error" ? "danger" : "warning",
        title: repairHintTitle(hint),
        detail: repairHintDetail(hint),
        actionLabel: repairHintActionLabel(hint),
        onAction: () => setActiveStep(repairHintStep(hint)),
      });
    }
    if (repairHints.length > 6) {
      items.push({
        key: "repair-hint-more",
        tone: "warning",
        title: "还有更多修复建议",
        detail: `共 ${repairHints.length} 条建议，已展示前 6 条。`,
        actionLabel: "回到内容",
        onAction: () => setActiveStep(2),
      });
    }
    if (acceptanceSummary && !acceptanceSummary.canRender && repairHints.length === 0) {
      items.push({
        key: "acceptance-failed",
        tone: "danger",
        title: "内容未通过模板协议",
        detail: acceptanceSummary.message || "内容结构与当前模板不匹配。",
        actionLabel: "回到内容",
        onAction: () => setActiveStep(2),
      });
    }
    const missing = contentCheck?.missing ?? [];
    for (const [index, item] of missing.slice(0, 4).entries()) {
      items.push({
        key: `missing-binding-${index}`,
        tone: "danger",
        title: `缺失绑定 ${index + 1}`,
        detail: describeMissingBinding(item),
        actionLabel: "回到内容",
        onAction: () => setActiveStep(2),
      });
    }
    if (missing.length > 4) {
      items.push({
        key: "missing-binding-more",
        tone: "warning",
        title: "还有更多缺失项",
        detail: `共 ${missing.length} 个缺失绑定，已展示前 4 个。`,
        actionLabel: "回到内容",
        onAction: () => setActiveStep(2),
      });
    }
    if (runState.status === "error" && items.length === 0) {
      items.push({
        key: "runtime-error",
        tone: "danger",
        title: "预检执行失败",
        detail: runState.message,
        actionLabel: "重新预检",
        onAction: () => void dryRun(),
      });
    }
    return items;
  })();

  return (
    <div className="render-workbench">
      <aside className="workflow-outline" aria-label="生成流程">
        <div className="workflow-outline-heading">
          <span>生成流程</span>
          <strong>4 步</strong>
        </div>
        {[
          { number: 1, title: "选择模板", detail: selectedTemplate?.name ?? "选择模板包", ready: templateReady },
          { number: 2, title: "内容源", detail: fileName(form.input) || "JSON / Markdown", ready: contentReady },
          { number: 3, title: "输出设置", detail: fileName(form.output) || "路径与文件名", ready: outputReady },
          { number: 4, title: "生成结果", detail: outputFile ? "已生成文件" : "预检并生成", ready: runState.status === "ok" },
        ].map((step) => (
          <button
            type="button"
            key={step.number}
            className={`workflow-outline-step${activeStep === step.number ? " is-active" : ""}`}
            onClick={() => setActiveStep(step.number)}
            aria-current={activeStep === step.number ? "step" : undefined}
          >
            <span className={`workflow-outline-number${step.ready ? " is-ready" : ""}`}>
              {step.ready ? <CheckCircle2 size={15} /> : step.number}
            </span>
            <span className="workflow-outline-copy">
              <strong>{step.title}</strong>
              <small>{step.detail}</small>
            </span>
          </button>
        ))}
        <div className="workflow-outline-actions">
          <Button onClick={dryRun} disabled={isRunning || !canValidate}>
            <CheckCircle2 size={15} />
            预检任务
          </Button>
          <Button variant="primary" onClick={renderOffice} disabled={isRunning || !taskReady}>
            <FileOutput size={16} />
            生成 {activeFormat.toUpperCase()}
          </Button>
          <Button onClick={() => setConfigOpen(true)} disabled={isRunning}>
            <Settings2 size={15} />
            高级配置
          </Button>
        </div>
      </aside>
      <div className="render-workflow">
      <section className={`workflow-section${activeStep === 1 ? " is-active" : " is-hidden"}`}>
        <WorkflowStepHeader
          number={1}
          title="选择模板"
          helper="从模板库选择，或链接本地模板包"
          ready={templateReady}
        />
        <div className="workflow-template-picker">
          <TemplatePicker
            templates={templates}
            value={form.template}
            contentPath={form.input}
            disabled={isRunning}
            emptyLabel="手动选择模板包"
            onSelect={selectRegisteredTemplate}
            onRefresh={() => void loadTemplates(false)}
            onManualPath={chooseTemplateFolder}
          />
          <div className="metadata-row workflow-template-meta">
            <span>{activeFormat.toUpperCase()}</span>
            <span>
              {selectedTemplateUsesScript
                ? "脚本渲染"
                : form.recipe || structureFallbackLabel(activeFormat)}
            </span>
            {selectedFamily?.schemaId ? <span>协议 {selectedFamily.schemaId}</span> : null}
            {selectedTemplate ? <span>用途 {templateRoleLabel(templateDocumentRole(selectedTemplate))}</span> : null}
            {compatibleTemplates.length > 1 ? <span>同用途样式 {compatibleTemplates.length} 种</span> : null}
          </div>
        </div>
        <p className="workflow-footnote">{templateMessage}</p>
      </section>

      <div className="workflow-connector" aria-hidden="true">
        <ArrowDown size={17} />
      </div>

      <section className={`workflow-section${activeStep === 2 ? " is-active" : " is-hidden"}`}>
        <WorkflowStepHeader
          number={2}
          title="内容源"
          helper="选择 AI 生成的 JSON 或 Markdown"
          ready={contentReady}
        />
        <div className="workflow-choice content-choice">
          <div className="content-file-icon" aria-hidden="true">
            <Braces size={30} strokeWidth={1.55} />
          </div>
          <div className="workflow-choice-copy">
            <strong>{fileName(form.input) || "尚未选择内容文件"}</strong>
            <Input
              aria-label="内容文件路径"
              value={form.input}
              onChange={(event) => updateField("input", event.target.value)}
            />
            <div className="metadata-row">
              <span>{contentFormatLabel(form.input)}</span>
              <span>本地文件</span>
            </div>
          </div>
          <div className="workflow-choice-actions">
            <Button type="button" onClick={chooseContentJson} disabled={isRunning}>
              选择文件
            </Button>
          </div>
        </div>
      </section>

      <div className="workflow-connector" aria-hidden="true">
        <ArrowDown size={17} />
      </div>

      <section className={`workflow-section${activeStep === 3 ? " is-active" : " is-hidden"}`}>
        <WorkflowStepHeader
          number={3}
          title="输出设置"
          helper="设置文件路径与名称"
          ready={outputReady}
        />
        <div className="output-setting-row">
          <div className="output-icon" aria-hidden="true">
            <FolderOpen size={22} strokeWidth={1.5} />
          </div>
          <Input
            aria-label={`输出 ${activeFormat.toUpperCase()} 路径`}
            value={form.output}
            onChange={(event) => updateField("output", event.target.value)}
          />
          <div className="output-setting-actions">
            <Button type="button" onClick={applyOutputRule} disabled={isRunning || !contentReady}>
              按规则
            </Button>
            <Button type="button" onClick={chooseOutputOffice} disabled={isRunning}>
              另存为
            </Button>
          </div>
        </div>
        <div className={`output-preview-row is-${outputPreview.status}`}>
          <span className={`status-pill ${outputPreviewStatusClass(outputPreview.status)}`}>
            {outputPreviewStatusLabel(outputPreview.status)}
          </span>
          <div>
            <strong>{outputPreview.message}</strong>
            {outputPreview.resolvedPath && outputPreview.resolvedPath !== form.output ? (
              <small className="path-text">最终写入：{outputPreview.resolvedPath}</small>
            ) : null}
          </div>
          {autoOutputPath ? <em>{outputSourceLabel(outputPreview.source)}</em> : <em>手动路径</em>}
        </div>
      </section>

      <div className="workflow-connector" aria-hidden="true">
        <ArrowDown size={17} />
      </div>

      <section className={`workflow-section workflow-result-section${activeStep === 4 ? " is-active" : " is-hidden"}`}>
        <WorkflowStepHeader
          number={4}
          title="生成结果"
          helper={
            outputFile
              ? "文件已生成，可直接打开或定位"
              : `预检通过后生成 ${activeFormat.toUpperCase()}`
          }
          ready={runState.status === "ok"}
          running={isRunning}
          error={runState.status === "error"}
        />
        <div className={`preflight-panel ${statusClass(runState.status)}`}>
          <div className="preflight-state-icon" aria-hidden="true">
            {runState.status === "running" ? (
              <LoaderCircle className="is-spinning" size={24} />
            ) : runState.status === "error" ? (
              <TriangleAlert size={24} />
            ) : runState.status === "ok" ? (
              <CheckCircle2 size={24} />
            ) : (
              <Circle size={24} />
            )}
          </div>
          <div className="preflight-copy">
            <strong>{statusLabel(runState.status)}</strong>
            <span>{runState.message}</span>
            <div className="preflight-checks">
              <span>模板 {templateReady ? "可用" : "待选择"}</span>
              <span>内容 {contentReady ? "已就绪" : "待选择"}</span>
              <span>输出 {outputReady ? "可写入" : "生成前设置"}</span>
              {outputIntegrity?.status === "passed" ? <span>完整性 已校验</span> : null}
            </div>
          </div>
          <div className="workflow-primary-actions">
            <Button onClick={() => setConfigOpen(true)} disabled={isRunning}>
              <Settings2 size={15} />
              高级配置
            </Button>
            <Button variant="secondary" onClick={dryRun} disabled={isRunning || !canValidate}>
              预检任务
            </Button>
            <Button variant="primary" onClick={renderOffice} disabled={isRunning || !taskReady}>
              <FileOutput size={16} />
              生成 {activeFormat.toUpperCase()}
            </Button>
          </div>
          <div className="preflight-check-list" aria-label="生成前预检清单">
            {preflightItems.map((item) => (
              <button
                type="button"
                className={`preflight-check-item is-${item.state}`}
                key={item.key}
                onClick={item.onAction}
                disabled={isRunning || item.disabled}
              >
                <span aria-hidden="true">
                  {item.state === "ready" ? (
                    <CheckCircle2 size={15} />
                  ) : item.state === "missing" ? (
                    <TriangleAlert size={15} />
                  ) : (
                    <Circle size={15} />
                  )}
                </span>
                <span>
                  <strong>{item.label}</strong>
                  <small>{item.detail}</small>
                </span>
                <em>{item.actionLabel}</em>
              </button>
            ))}
          </div>
        </div>
        {!outputFile ? (
          <section className={`preflight-guidance-panel is-${preflightGuidance.tone}`}>
            <div className="preflight-guidance-copy">
              <span>下一步</span>
              <strong>{preflightGuidance.title}</strong>
              <p>{preflightGuidance.detail}</p>
            </div>
            <dl className="preflight-guidance-metrics">
              {preflightGuidance.metrics.map((item) => (
                <div className={`is-${item.tone ?? "idle"}`} key={`${item.label}-${item.value}`}>
                  <dt>{item.label}</dt>
                  <dd>{item.value}</dd>
                </div>
              ))}
            </dl>
            <Button
              size="sm"
              variant={preflightGuidance.tone === "ready" ? "primary" : "secondary"}
              onClick={preflightGuidance.onAction}
              disabled={preflightGuidance.disabled || isRunning}
            >
              {preflightGuidance.actionLabel}
            </Button>
          </section>
        ) : null}
        {!outputFile && preflightRepairItems.length > 0 ? (
          <section className="preflight-repair-list" aria-label="预检修复清单">
            <header>
              <strong>修复清单</strong>
              <span>{preflightRepairItems.length} 项需要确认</span>
            </header>
            <div>
              {preflightRepairItems.map((item) => (
                <button
                  type="button"
                  className={`preflight-repair-item is-${item.tone}`}
                  key={item.key}
                  onClick={item.onAction}
                  disabled={isRunning}
                >
                  <span aria-hidden="true">
                    {item.tone === "danger" ? (
                      <TriangleAlert size={15} />
                    ) : (
                      <Circle size={15} />
                    )}
                  </span>
                  <span>
                    <strong>{item.title}</strong>
                    <small>{item.detail}</small>
                  </span>
                  <em>{item.actionLabel}</em>
                </button>
              ))}
            </div>
          </section>
        ) : null}
        {outputFile ? (
          <section className="generated-output-card" aria-label="生成完成">
            <div className="generated-output-main">
              <div className="generated-output-icon" aria-hidden="true">
                <CheckCircle2 size={24} />
              </div>
              <div className="generated-output-copy">
                <span>生成完成</span>
                <strong>{fileName(outputFile) || outputFile}</strong>
                <small className="path-text">{outputFile}</small>
              </div>
            </div>
            <div className="generated-output-actions">
              <Button size="sm" variant="primary" onClick={openOutputFile} disabled={isRunning}>
                <ExternalLink size={14} />
                打开
              </Button>
              <Button size="sm" onClick={revealOutputFile} disabled={isRunning}>
                <FolderOpen size={14} />
                定位
              </Button>
              <Button size="sm" onClick={copyOutputPath} disabled={isRunning}>
                <Copy size={14} />
                复制
              </Button>
              <Button size="sm" onClick={continueSameTemplate} disabled={isRunning}>
                <RotateCcw size={14} />
                继续同类
              </Button>
            </div>
            <dl className="generated-output-metrics">
              <div>
                <dt>格式</dt>
                <dd>{activeFormat.toUpperCase()}</dd>
              </div>
              {renderedUnitCount ? (
                <div>
                  <dt>{renderedUnitLabel(activeFormat)}</dt>
                  <dd>{renderedUnitCount}</dd>
                </div>
              ) : null}
              {outputIntegrity?.status === "passed" ? (
                <>
                  <div>
                    <dt>完整性</dt>
                    <dd>已通过</dd>
                  </div>
                  <div>
                    <dt>包结构</dt>
                    <dd>
                      {outputIntegrity.entryCount} / {outputIntegrity.relationshipCount}
                    </dd>
                  </div>
                </>
              ) : (
                <div>
                  <dt>状态</dt>
                  <dd>已写入</dd>
                </div>
              )}
            </dl>
          </section>
        ) : null}
        {batchOutputs.length > 0 ? (
          <div className="batch-output-list">
            {batchOutputs.map((item) => (
              <div className="batch-output-row" key={item.templateId}>
                <span className={`status-pill ${item.status === "ok" ? "is-ok" : "is-danger"}`}>
                  {item.status === "ok" ? "完成" : "失败"}
                </span>
                <div>
                  <strong>{item.templateName}</strong>
                  <span className="path-text">{item.message ?? item.outputFile}</span>
                </div>
                {item.status === "ok" ? (
                  <Button size="sm" onClick={() => openBatchOutput(item.outputFile)} disabled={isRunning}>
                    打开
                  </Button>
                ) : null}
              </div>
            ))}
          </div>
        ) : null}
        {resultPreview ? (
          <details className="compact-disclosure render-details">
            <summary>查看任务 JSON</summary>
            <pre className="result-preview">{resultPreview}</pre>
          </details>
        ) : null}
      </section>
      <section className="workflow-context-panel" aria-label="当前配置摘要">
        <header>
          <strong>当前配置</strong>
          <span>生成前快速核对</span>
        </header>
        <dl>
          <div>
            <dt>模板</dt>
            <dd>{selectedTemplate?.name ?? "尚未选择"}</dd>
          </div>
          <div>
            <dt>内容</dt>
            <dd>{fileName(form.input) || "尚未选择"}</dd>
          </div>
          <div>
            <dt>输出</dt>
            <dd>{fileName(form.output) || "尚未设置"}</dd>
          </div>
          <div>
            <dt>结构</dt>
            <dd>{selectedTemplateUsesScript ? "脚本自动处理" : form.recipe || structureFallbackLabel(activeFormat)}</dd>
          </div>
          <div>
            <dt>输入协议</dt>
            <dd>{selectedFamily?.schemaId ?? selectedTemplate?.inputSchemaId ?? "模板自定义"}</dd>
          </div>
          <div>
            <dt>生成范围</dt>
            <dd>
              {selectedTemplate
                ? `${templateRoleLabel(templateDocumentRole(selectedTemplate))} / ${
                    compatibleTemplates.length > 1 ? `${compatibleTemplates.length} 种同用途样式` : "当前样式"
                  }`
                : "当前样式"}
            </dd>
          </div>
        </dl>
      </section>
      <section className="workflow-preview-panel" aria-label="结构与输出预览">
        <header>
          <div>
            <strong>结构与输出预览</strong>
            <span>模板能力与生成范围</span>
          </div>
          <span className={`status-pill ${selectedTemplate ? "is-ok" : ""}`}>
            {selectedTemplate ? "模板可用" : "等待模板"}
          </span>
        </header>
        {selectedTemplate ? (
          <div className="workflow-preview-content">
            <div className="workflow-preview-format" aria-hidden="true">
              <OfficeFormatIcon format={activeFormat} />
            </div>
            <dl className="workflow-preview-metrics">
              <div>
                <dt>输出格式</dt>
                <dd>{activeFormat.toUpperCase()}</dd>
              </div>
              <div>
                <dt>结构单元</dt>
                <dd>{structureCount} 项</dd>
              </div>
              <div>
                <dt>组合结构</dt>
                <dd>{recipeCount} 项</dd>
              </div>
              <div>
                <dt>输入格式</dt>
                <dd>{selectedTemplate.inputFormats.map((format) => format.toUpperCase()).join(" / ")}</dd>
              </div>
              <div>
                <dt>渲染类型</dt>
                <dd>{selectedTemplateUsesScript ? "脚本型" : "声明式"}</dd>
              </div>
              <div>
                <dt>可生成样式</dt>
                <dd>{compatibleTemplates.length || 1} 种</dd>
              </div>
            </dl>
            <div className="workflow-preview-ids">
              <span>可用结构</span>
              <div>
                {structureIds.length > 0 ? (
                  structureIds.slice(0, 8).map((id) => <code key={id}>{id}</code>)
                ) : (
                  <small>由模板脚本或组合结构自动处理</small>
                )}
              </div>
            </div>
          </div>
        ) : (
          <div className="workflow-preview-empty">
            <Presentation size={24} strokeWidth={1.45} />
            <div>
              <strong>选择模板后显示结构预览</strong>
              <span>这里会展示页面、文档块或工作表结构，以及支持的输入格式和生成范围。</span>
            </div>
          </div>
        )}
      </section>
      </div>
      <ConfigDialog
        open={configOpen}
        title="生成高级配置"
        description="路径、结构和输出位置这些低频项都在这里调整。"
        onClose={() => setConfigOpen(false)}
      >
        <div className="form-stack">
          <label>
            模板包目录
            <div className="field-row">
              <Input
                value={form.template}
                onChange={(event) => updateField("template", event.target.value)}
              />
              <Button type="button" onClick={chooseTemplateFolder} disabled={isRunning}>
                选择
              </Button>
            </div>
          </label>
          <label>
            {structureLabel(activeFormat)}
            {selectedTemplateUsesScript ? (
              <Input value="脚本渲染器自动处理结构与版式" disabled />
            ) : recipeOptions.length > 0 ? (
              <select
                className="ui-select"
                value={form.recipe}
                onChange={(event) => updateField("recipe", event.target.value)}
              >
                {!recipeOptions.includes(form.recipe) && form.recipe ? (
                  <option value={form.recipe}>{form.recipe}</option>
                ) : null}
                {recipeOptions.map((recipe) => (
                  <option value={recipe} key={recipe}>
                    {recipe}
                  </option>
                ))}
              </select>
            ) : (
              <Input
                value={form.recipe}
                onChange={(event) => updateField("recipe", event.target.value)}
              />
            )}
          </label>
          <label>
            输出格式
            <select
              className="ui-select"
              value={activeFormat}
              onChange={(event) => updateFormat(event.target.value)}
              disabled={Boolean(selectedTemplate)}
            >
              <option value="pptx">PPTX</option>
              <option value="docx">DOCX</option>
              <option value="xlsx">XLSX</option>
            </select>
          </label>
          <label>
            输出 {activeFormat.toUpperCase()}
            <div className="field-row">
              <Input value={form.output} onChange={(event) => updateField("output", event.target.value)} />
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

function OfficeFormatIcon({ format }: { format: OfficeFormat }) {
  if (format === "docx") {
    return <FileText size={32} strokeWidth={1.45} />;
  }
  if (format === "xlsx") {
    return <Table2 size={32} strokeWidth={1.45} />;
  }
  return <Presentation size={34} strokeWidth={1.45} />;
}

function WorkflowStepHeader({
  number,
  title,
  helper,
  ready,
  running = false,
  error = false,
}: {
  number: number;
  title: string;
  helper: string;
  ready: boolean;
  running?: boolean;
  error?: boolean;
}) {
  return (
    <header className="workflow-step-header">
      <span className="workflow-step-number">{number}</span>
      <strong>{title}</strong>
      <span>{helper}</span>
      <span
        className={`workflow-step-status${error ? " is-danger" : ready ? " is-ready" : ""}`}
        aria-label={error ? "需要处理" : running ? "运行中" : ready ? "已就绪" : "未完成"}
      >
        {running ? (
          <LoaderCircle className="is-spinning" size={18} />
        ) : error ? (
          <TriangleAlert size={18} />
        ) : ready ? (
          <CheckCircle2 size={18} />
        ) : (
          <Circle size={18} />
        )}
      </span>
    </header>
  );
}

type AcceptanceSummary = {
  status: "pass" | "warn" | "fail";
  canRender: boolean;
  plannedPageCount: number;
  message: string;
};

type WorkflowRepairHint = {
  code?: string;
  target: "content" | "template" | "binding" | "asset" | "output" | string;
  severity: "error" | "warning" | string;
  blocking?: boolean;
  path?: string | null;
  sourcePath?: string | null;
  message: string;
  suggestedAction: string;
};

type ContentWorkspaceValidation = {
  acceptanceSummary: AcceptanceSummary;
  selectedRecipe?: string | null;
  renderFormat?: string | null;
};

type PptxSidecarData = {
  outputFile: string;
  plannedPageCount: number;
  cleanup?: {
    visibleSlideCount: number;
    slidePartCount: number;
  };
  contentCheck?: {
    status: "passed" | "failed";
    checkedPages: number;
    checkedBindings: number;
    missing: unknown[];
  };
};

type RenderResultEnvelope = {
  workflow?: string;
  stage?: "validate" | "rendered" | string;
  validation?: ContentWorkspaceValidation;
  repairHints?: WorkflowRepairHint[];
  render?: RenderResultEnvelope;
  rendererStatus?: string;
  format?: string;
  renderFormat?: string | null;
  selectedRecipe?: string | null;
  outputFile?: string;
  outputIntegrity?: OfficePackageIntegrity;
  plannedPageCount?: number | null;
  sheetCount?: number | null;
  acceptanceSummary?: AcceptanceSummary;
  renderResult?: {
    outputFile?: string;
    outputIntegrity?: OfficePackageIntegrity;
    plannedPageCount?: number | null;
    sheetCount?: number | null;
    sidecar?: {
      ok?: boolean;
      data?: PptxSidecarData;
    };
    renderer?: {
      outputFile?: string;
      sidecar?: {
        ok?: boolean;
        data?: PptxSidecarData;
      };
    };
  };
  sidecar?: {
    ok?: boolean;
    data?: PptxSidecarData;
  };
  batchOutputs?: BatchOutput[];
};

type OfficePackageIntegrity = {
  status: "passed";
  format: string;
  fileSize: number;
  entryCount: number;
  relationshipCount: number;
  requiredEntries: string[];
};

function usesScriptRenderer(template: TemplatePackRecord | undefined) {
  return (
    template?.templateType === "script" ||
    (template?.templateType === "hybrid" && Boolean(template.rendererType))
  );
}

function recipeForBatchTemplate(template: TemplatePackRecord, preferredRecipe: string) {
  if (usesScriptRenderer(template)) {
    return null;
  }
  const recipes = recipeIdsForTemplate(template);
  return (recipes.includes(preferredRecipe) ? preferredRecipe : firstRecipe(template)) || null;
}

function normalizedOfficeFormat(format: string | null | undefined): OfficeFormat {
  if (format === "docx" || format === "xlsx") {
    return format;
  }
  return "pptx";
}

function formFromDraft(draft: WorkflowDraft | null | undefined, fallback: RenderForm): RenderForm {
  if (!draft) {
    return fallback;
  }
  const format = normalizedOfficeFormat(draft.format);
  return {
    template: draft.templatePath,
    recipe: draft.recipe ?? "",
    input: draft.contentPath,
    output: withOfficeExtension(draft.outputPath ?? "", format),
    format,
  };
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

function batchOutputPath(
  path: string,
  format: OfficeFormat,
  template: TemplatePackRecord,
  family: TemplateFamily,
) {
  const normalizedPath = withOfficeExtension(path, format);
  const extensionPattern = new RegExp(`\\.${format}$`, "i");
  const stem = normalizedPath.replace(extensionPattern, "");
  return `${stem}-${templateVariantSlug(template, family)}.${format}`;
}

function pptxSidecarData(result: RenderResultEnvelope | undefined) {
  return (
    result?.sidecar?.data ??
    result?.render?.sidecar?.data ??
    result?.renderResult?.sidecar?.data ??
    result?.render?.renderResult?.sidecar?.data ??
    result?.renderResult?.renderer?.sidecar?.data
  );
}

function renderOutputFile(result: RenderResultEnvelope | undefined) {
  return (
    result?.outputFile ??
    result?.render?.outputFile ??
    result?.sidecar?.data?.outputFile ??
    result?.renderResult?.outputFile ??
    result?.render?.renderResult?.outputFile ??
    result?.renderResult?.renderer?.outputFile ??
    result?.renderResult?.sidecar?.data?.outputFile ??
    result?.renderResult?.renderer?.sidecar?.data?.outputFile ??
    null
  );
}

function renderOutputIntegrity(result: RenderResultEnvelope | undefined) {
  return result?.outputIntegrity ?? result?.render?.outputIntegrity ?? result?.renderResult?.outputIntegrity;
}

function renderSheetCount(result: RenderResultEnvelope | undefined) {
  return result?.sheetCount ?? result?.render?.sheetCount ?? result?.renderResult?.sheetCount ?? undefined;
}

function repairHintsFromResult(result: RenderResultEnvelope | undefined) {
  return result?.repairHints ?? [];
}

function repairHintTitle(hint: WorkflowRepairHint) {
  const target =
    hint.target === "content"
      ? "内容"
      : hint.target === "template"
        ? "模板"
        : hint.target === "asset"
          ? "素材"
          : hint.target === "binding"
            ? "绑定"
            : "任务";
  return `${target}${hint.blocking || hint.severity === "error" ? "需修复" : "需确认"}`;
}

function repairHintDetail(hint: WorkflowRepairHint) {
  const source = hint.sourcePath ?? hint.path ?? hint.code;
  return source ? `${source}：${hint.message}` : hint.message;
}

function repairHintActionLabel(hint: WorkflowRepairHint) {
  if (hint.target === "template" || hint.suggestedAction === "repair_template_pack") {
    return "检查模板";
  }
  if (hint.target === "asset" || hint.suggestedAction === "fix_asset_reference") {
    return "检查素材";
  }
  return "回到内容";
}

function repairHintStep(hint: WorkflowRepairHint) {
  if (hint.target === "template" || hint.suggestedAction === "repair_template_pack") {
    return 1;
  }
  if (hint.target === "output") {
    return 3;
  }
  return 2;
}

function describeMissingBinding(value: unknown) {
  if (typeof value === "string") {
    return value;
  }
  if (!value || typeof value !== "object") {
    return "内容中缺少模板需要的字段。";
  }
  const record = value as Record<string, unknown>;
  const candidates = [
    record.binding,
    record.bindingId,
    record.field,
    record.key,
    record.path,
    record.id,
    record.name,
    record.label,
    record.message,
  ];
  const primary = candidates.find((item) => typeof item === "string" && item.trim());
  if (typeof primary === "string") {
    return primary;
  }
  try {
    return JSON.stringify(value);
  } catch {
    return "内容中缺少模板需要的字段。";
  }
}

function officeFormatName(format: OfficeFormat) {
  if (format === "docx") {
    return "Word";
  }
  if (format === "xlsx") {
    return "Excel";
  }
  return "PowerPoint";
}

function structureLabel(format: OfficeFormat) {
  if (format === "docx") {
    return "文档结构";
  }
  if (format === "xlsx") {
    return "工作簿结构";
  }
  return "演示结构";
}

function renderedUnitLabel(format: OfficeFormat) {
  if (format === "xlsx") {
    return "工作表";
  }
  if (format === "docx") {
    return "文档";
  }
  return "页数";
}

function structureFallbackLabel(format: OfficeFormat) {
  if (format === "docx") {
    return "内容块模板";
  }
  if (format === "xlsx") {
    return "工作表模板";
  }
  return "页面模板";
}

function countTemplateFormats(templates: TemplatePackRecord[]) {
  const initial: Record<OfficeFormat, number> = { pptx: 0, docx: 0, xlsx: 0 };
  return templates.reduce(
    (counts, template) => {
      counts[normalizedOfficeFormat(template.format)] += 1;
      return counts;
    },
    initial,
  );
}

function acceptanceMessage(
  summary: AcceptanceSummary,
  format: OfficeFormat,
  recipe?: string | null,
) {
  if (!summary.canRender || summary.status === "fail") {
    return `预检未通过：${summary.message}`;
  }
  if (summary.status === "warn") {
    return `预检通过但有提示：${summary.message}`;
  }
  if (format === "pptx") {
    return `预检通过：${summary.plannedPageCount} 个计划页`;
  }
  return `预检通过：${format.toUpperCase()}${recipe ? ` / ${recipe}` : ""}`;
}

function renderMessage(label: string, data: unknown) {
  const output = renderOutputFile(data as RenderResultEnvelope | undefined);
  const doneLabel = label.startsWith("正在") ? `已${label.slice(2)}` : `${label}完成`;
  return output ? `${doneLabel}：${output}` : doneLabel;
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

function statusClass(status: RunState["status"]) {
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

function outputSourceLabel(source: OutputSuggestionSource) {
  if (source === "preferences") {
    return "输出规则";
  }
  if (source === "content-directory") {
    return "内容目录";
  }
  return "手动路径";
}

function outputPreviewMessage(
  resolution: OutputPathResolution,
  source: OutputSuggestionSource,
) {
  const sourceLabel = outputSourceLabel(source);
  if (resolution.renamed) {
    return "已有同名文件，将自动改名";
  }
  if (resolution.conflictPolicy === "overwrite") {
    return `已有文件时会覆盖，来源：${sourceLabel}`;
  }
  return `路径可用，来源：${sourceLabel}`;
}

function outputPreviewStatusLabel(status: OutputPreviewState["status"]) {
  if (status === "renamed") return "改名";
  if (status === "overwrite") return "覆盖";
  if (status === "error") return "错误";
  if (status === "missing") return "待设置";
  if (status === "ok") return "可用";
  return "就绪";
}

function outputPreviewStatusClass(status: OutputPreviewState["status"]) {
  if (status === "ok") return "is-ok";
  if (status === "renamed" || status === "overwrite") return "is-warn";
  if (status === "error") return "is-danger";
  return "";
}

function fileName(path: string) {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? "";
}

function contentFormatLabel(path: string) {
  const nameParts = fileName(path).split(".");
  const extension = nameParts[nameParts.length - 1]?.toUpperCase();
  if (extension === "MD" || extension === "MARKDOWN") {
    return "Markdown";
  }
  return extension || "JSON / Markdown";
}
