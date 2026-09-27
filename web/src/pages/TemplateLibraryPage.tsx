import { useEffect, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import {
  Activity,
  Archive,
  ChevronDown,
  Download,
  FileText,
  FolderOpen,
  Play,
  Search,
  Upload,
} from "lucide-react";
import { ConfigDialog } from "@/components/ConfigDialog";
import { TemplateThumbnail } from "@/components/TemplateThumbnail";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { Pagination } from "@/components/Pagination";
import { paginate } from "@/lib/pagination";
import {
  filterTemplateFamilies,
  firstRecipe,
  groupTemplateFamilies,
  templatePrimaryPath,
  templateDocumentRole,
  templateRoleLabel,
  templateStructureGroups,
  templateVariantLabel,
  type TemplatePackRecord,
} from "@/lib/template-packs";
import {
  templateAcceptanceIssues,
  templateAcceptanceStatus,
  type TemplateReadinessReport,
  type TemplateTestReport,
  type TemplateValidationSummary,
} from "@/lib/template-acceptance";
import type { WorkflowDraft } from "@/lib/workflow-draft";

const defaultScriptPath = "";
const defaultScriptAdapterDir = "";
const TEMPLATE_FAMILY_PAGE_SIZE = 5;

type RunState = {
  status: "idle" | "running" | "ok" | "error";
  message: string;
};

type OfficeTemplateFormat = "pptx" | "docx" | "xlsx";

type OfficeTemplatePackDraft = {
  packDir: string;
  templateFile: string;
  manifestFile: string;
  inspection: {
    slideCount?: number;
    placeholderCount?: number;
    sheetCount?: number;
    namedRangeCount?: number;
    warnings: string[];
    slides?: Array<{
      sourceSlide: number;
      shapeCount: number;
      bindableShapeCount: number;
    }>;
    placeholders?: Array<{ bindable: boolean }>;
    namedRanges?: Array<{ bindable: boolean }>;
  };
  validation: {
    warnings: string[];
  };
};

type TemplateContractResult = {
  outputFile?: string | null;
  templateName: string;
};

type TemplateContentSkeletonResult = {
  outputFile?: string | null;
  templateName: string;
  mode: string;
  warnings: string[];
};

type ScriptTemplateAdapter = {
  packDir: string;
  manifestFile: string;
  scriptFile: string;
  validation: {
    warnings: string[];
  };
};

type TemplateArchiveExportResult = {
  archiveFile: string;
  templateId: string;
  fileCount: number;
  totalBytes: number;
};

type TemplateArchiveImportResult = {
  archiveFile: string;
  packDir: string;
  fileCount: number;
  totalBytes: number;
  template: TemplatePackRecord;
};

type TemplateLibraryPageProps = {
  onUseTemplate?: (draft: WorkflowDraft) => void;
};

type TemplateAcceptanceResult = {
  recordId: string;
  readiness: TemplateReadinessReport;
  validation: TemplateValidationSummary;
  tests: TemplateTestReport | null;
};

type TemplateDetailSectionKey = "structure" | "acceptance" | "management";

export function TemplateLibraryPage({ onUseTemplate }: TemplateLibraryPageProps) {
  const [templatePath, setTemplatePath] = useState("");
  const [configOpen, setConfigOpen] = useState(false);
  const [archiveImportOpen, setArchiveImportOpen] = useState(false);
  const [archivePath, setArchivePath] = useState("");
  const [archiveRoot, setArchiveRoot] = useState("");
  const [archiveForce, setArchiveForce] = useState(false);
  const [lastArchiveExport, setLastArchiveExport] = useState<{
    templateId: string;
    path: string;
  } | null>(null);
  const [templateAcceptance, setTemplateAcceptance] =
    useState<TemplateAcceptanceResult | null>(null);
  const [draftFormat, setDraftFormat] = useState<OfficeTemplateFormat>("pptx");
  const [officeTemplatePath, setOfficeTemplatePath] = useState("");
  const [draftDir, setDraftDir] = useState("");
  const [scriptPath, setScriptPath] = useState(defaultScriptPath);
  const [scriptAdapterDir, setScriptAdapterDir] = useState(defaultScriptAdapterDir);
  const [scriptTemplateId, setScriptTemplateId] = useState("custom-script-template");
  const [scriptTemplateName, setScriptTemplateName] = useState("自定义脚本模板");
  const [scriptFormat, setScriptFormat] = useState("pptx");
  const [scriptInputFormat, setScriptInputFormat] = useState("md");
  const [scriptRuntime, setScriptRuntime] = useState("");
  const [templates, setTemplates] = useState<TemplatePackRecord[]>([]);
  const [selectedTemplateId, setSelectedTemplateId] = useState("");
  const [templateQuery, setTemplateQuery] = useState("");
  const [formatFilter, setFormatFilter] = useState("all");
  const [currentPage, setCurrentPage] = useState(1);
  const [openDetailSections, setOpenDetailSections] = useState<
    Record<TemplateDetailSectionKey, boolean>
  >({
    structure: true,
    acceptance: true,
    management: false,
  });
  const templateListRef = useRef<HTMLDivElement>(null);
  const [runState, setRunState] = useState<RunState>({
    status: "idle",
    message: "链接本地模板包，模板文件不会进入应用包。",
  });
  const isRunning = runState.status === "running";
  const templateFamilies = useMemo(() => groupTemplateFamilies(templates), [templates]);
  const filteredTemplateFamilies = useMemo(
    () => filterTemplateFamilies(templateFamilies, templateQuery, formatFilter),
    [formatFilter, templateFamilies, templateQuery],
  );
  const pagination = useMemo(
    () => paginate(filteredTemplateFamilies, currentPage, TEMPLATE_FAMILY_PAGE_SIZE),
    [currentPage, filteredTemplateFamilies],
  );
  const selectedTemplate = useMemo(
    () => templates.find((template) => template.id === selectedTemplateId) ?? templates[0],
    [selectedTemplateId, templates],
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
  const selectedStructureGroups = useMemo(
    () => (selectedTemplate ? templateStructureGroups(selectedTemplate) : []),
    [selectedTemplate],
  );
  const selectedStructureItemCount = selectedStructureGroups.reduce(
    (total, group) => total + group.ids.length,
    0,
  );
  const selectedPrimaryPath = selectedTemplate ? templatePrimaryPath(selectedTemplate) : null;
  const selectedAcceptance =
    selectedTemplate && templateAcceptance?.recordId === selectedTemplate.id
      ? templateAcceptance
      : null;
  const selectedAcceptanceIssues = selectedAcceptance
    ? templateAcceptanceIssues(selectedAcceptance.readiness)
    : [];
  const selectedTestReport = selectedAcceptance?.tests ?? null;
  const selectedAcceptanceStatus = templateAcceptanceStatus(
    selectedAcceptance?.readiness ?? null,
    selectedTestReport,
  );
  const templateCountLabel = useMemo(
    () => {
      const unavailableCount = templates.filter((template) => !template.pathAvailable).length;
      return `已链接 ${templates.length} 个模板包，共 ${templateFamilies.length} 个系列${
        unavailableCount > 0 ? `，${unavailableCount} 个路径失效` : ""
      }`;
    },
    [templateFamilies.length, templates],
  );

  useEffect(() => {
    void loadTemplates();
  }, []);

  useEffect(() => {
    if (templates.length > 0 && !templates.some((template) => template.id === selectedTemplateId)) {
      setSelectedTemplateId(templates[0].id);
    }
  }, [selectedTemplateId, templates]);

  useEffect(() => {
    setCurrentPage(1);
  }, [formatFilter, templateQuery]);

  useEffect(() => {
    if (pagination.page !== currentPage) {
      setCurrentPage(pagination.page);
    }
  }, [currentPage, pagination.page]);

  useEffect(() => {
    if (templateListRef.current) {
      templateListRef.current.scrollTop = 0;
    }
  }, [pagination.page]);

  async function loadTemplates() {
    await runAction("正在读取模板库", async () => {
      const data = await invoke<TemplatePackRecord[]>("list_template_packs");
      setTemplates(data);
      setSelectedTemplateId((current) =>
        data.some((template) => template.id === current) ? current : data[0]?.id ?? "",
      );
      return `已载入 ${data.length} 个模板包`;
    });
  }

  function toggleDetailSection(section: TemplateDetailSectionKey) {
    setOpenDetailSections((current) => ({ ...current, [section]: !current[section] }));
  }

  async function chooseOfficeTemplateFile() {
    const formatLabel = draftFormat.toUpperCase();
    const selected = await open({
      multiple: false,
      title: `选择 ${formatLabel} 模板文件`,
      defaultPath: officeTemplatePath || undefined,
      filters: [{ name: formatLabel, extensions: [draftFormat] }],
    });
    if (typeof selected === "string") {
      setOfficeTemplatePath(selected);
      setDraftDir(buildDraftDir(selected));
    }
  }

  function changeDraftFormat(format: OfficeTemplateFormat) {
    setDraftFormat(format);
    if (!officeTemplatePath.toLowerCase().endsWith(`.${format}`)) {
      setOfficeTemplatePath("");
      setDraftDir(buildDraftDir(`template.${format}`));
    }
  }

  async function chooseDraftFolder() {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "选择草稿模板包目录",
      defaultPath: draftDir,
    });
    if (typeof selected === "string") {
      setDraftDir(selected);
    }
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

  async function chooseTemplateArchive() {
    const selected = await open({
      multiple: false,
      title: "选择 rDeckForge 模板包",
      defaultPath: archivePath || undefined,
      filters: [{ name: "rDeckForge 模板包", extensions: ["rdeckpack"] }],
    });
    if (typeof selected === "string") {
      setArchivePath(selected);
    }
  }

  async function chooseArchiveRoot() {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "选择外部模板根目录",
      defaultPath: archiveRoot || undefined,
    });
    if (typeof selected === "string") {
      setArchiveRoot(selected);
    }
  }

  async function chooseScriptFile() {
    const selected = await open({
      multiple: false,
      title: "选择脚本渲染器",
      defaultPath: scriptPath,
      filters: [
        { name: "Python", extensions: ["py"] },
        { name: "脚本", extensions: ["py", "js", "mjs", "cjs", "sh", "ps1"] },
      ],
    });
    if (typeof selected === "string") {
      setScriptPath(selected);
    }
  }

  async function chooseScriptAdapterFolder() {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "选择脚本型模板包目录",
      defaultPath: scriptAdapterDir,
    });
    if (typeof selected === "string") {
      setScriptAdapterDir(selected);
    }
  }

  async function createOfficeDraft() {
    const formatLabel = draftFormat.toUpperCase();
    await runAction(`正在生成 ${formatLabel} 草稿模板包`, async () => {
      const command = {
        pptx: "create_pptx_template_pack_draft",
        docx: "create_docx_template_pack_draft",
        xlsx: "create_xlsx_template_pack_draft",
      }[draftFormat];
      const draft = await invoke<OfficeTemplatePackDraft>(command, {
        input: officeTemplatePath,
        outDir: draftDir,
      });
      const record = await invoke<TemplatePackRecord>("link_template_pack", {
        template: draft.packDir,
      });
      setTemplatePath(draft.packDir);
      setTemplates((current) => upsertTemplate(current, record));
      setSelectedTemplateId(record.id);
      const summary = officeDraftSummary(draftFormat, draft.inspection);
      const warningCount = draft.inspection.warnings.length + draft.validation.warnings.length;
      const suffix = warningCount > 0 ? `，${warningCount} 条提示待确认` : "";
      return `已生成并链接：${record.name}（${summary}）${suffix}`;
    });
  }

  async function createScriptAdapter(force = false) {
    await runAction(force ? "正在覆盖脚本型模板包" : "正在创建脚本型模板包", async () => {
      const adapter = await invoke<ScriptTemplateAdapter>("create_script_template_adapter", {
        script: scriptPath,
        outDir: scriptAdapterDir,
        id: scriptTemplateId,
        name: scriptTemplateName,
        format: scriptFormat,
        inputFormat: scriptInputFormat,
        runtime: scriptRuntime,
        force,
      });
      const record = await invoke<TemplatePackRecord>("link_template_pack", {
        template: adapter.packDir,
      });
      setTemplatePath(adapter.packDir);
      setTemplates((current) => upsertTemplate(current, record));
      setSelectedTemplateId(record.id);
      const suffix =
        adapter.validation.warnings.length > 0
          ? `，${adapter.validation.warnings.length} 条提示待确认`
          : "";
      return `已创建并链接：${record.name}${suffix}`;
    });
  }

  async function linkTemplate() {
    await runAction("正在链接模板包", async () => {
      const record = await invoke<TemplatePackRecord>("link_template_pack", {
        template: templatePath,
      });
      setTemplates((current) => upsertTemplate(current, record));
      setSelectedTemplateId(record.id);
      return `${record.name} 已链接`;
    });
  }

  async function importTemplateArchive() {
    await runAction("正在导入模板包", async () => {
      const result = await invoke<TemplateArchiveImportResult>("import_template_pack_archive", {
        archive: archivePath,
        outDir: archiveRoot,
        force: archiveForce,
      });
      setTemplates((current) => upsertTemplate(current, result.template));
      setSelectedTemplateId(result.template.id);
      setTemplatePath(result.packDir);
      setArchiveImportOpen(false);
      return `已导入并链接 ${result.template.name}，共 ${result.fileCount} 个文件`;
    });
  }

  async function exportTemplateArchive(template: TemplatePackRecord) {
    const output = await save({
      title: "导出 rDeckForge 模板包",
      defaultPath: `${template.templateId}.rdeckpack`,
      filters: [{ name: "rDeckForge 模板包", extensions: ["rdeckpack"] }],
    });
    if (!output) {
      return;
    }
    await runAction("正在导出模板包", async () => {
      const result = await invoke<TemplateArchiveExportResult>("export_template_pack_archive", {
        template: template.path,
        output,
        force: true,
      });
      setLastArchiveExport({ templateId: template.id, path: result.archiveFile });
      return `已导出 ${result.fileCount} 个文件：${result.archiveFile}`;
    });
  }

  async function revealArchive(path: string) {
    await runAction("正在定位模板包", async () => {
      await invoke("reveal_path", { path });
      return "已在文件管理器中定位导出的模板包";
    });
  }

  async function exportContract(template: TemplatePackRecord) {
    await runAction("正在导出模板契约", async () => {
      const out = templateArtifactPath(template, "template-contract.json");
      const result = await invoke<TemplateContractResult>("export_template_contract", {
        template: template.path,
        out,
      });
      return `${result.templateName} 契约已导出：${result.outputFile ?? out}`;
    });
  }

  async function exportSkeleton(template: TemplatePackRecord, mode: "minimal" | "all") {
    await runAction("正在导出内容骨架", async () => {
      const out = templateArtifactPath(template, `content.skeleton.${mode}.json`);
      const recipe = defaultRecipeForSkeleton(template);
      const result = await invoke<TemplateContentSkeletonResult>("export_content_skeleton", {
        template: template.path,
        out,
        mode,
        recipe,
      });
      const suffix =
        result.warnings.length > 0 ? `，${result.warnings.length} 条提示待确认` : "";
      return `${result.templateName} ${result.mode} 骨架已导出：${
        result.outputFile ?? out
      }${suffix}`;
    });
  }

  async function openTemplateFolder(path: string) {
    await runAction("正在打开模板目录", async () => {
      await invoke("open_path", { path });
      return "已交给系统打开模板目录";
    });
  }

  async function openTemplateFile(template: TemplatePackRecord) {
    const path = templatePrimaryPath(template);
    if (!path) {
      setRunState({ status: "error", message: "该模板没有可直接打开的 Office 文件或脚本入口。" });
      return;
    }
    await runAction("正在打开模板文件", async () => {
      await invoke("open_path", { path });
      return "已交给系统打开模板文件";
    });
  }

  function useTemplate(template: TemplatePackRecord) {
    onUseTemplate?.({
      templatePath: template.path,
      contentPath: "",
      recipe: firstRecipe(template),
      format: template.format,
      outputPath: null,
      source: "template-library",
      updatedAt: Date.now(),
    });
  }

  async function refreshTemplate(id: string) {
    await runAction("正在重新校验模板包", async () => {
      const record = await invoke<TemplatePackRecord>("refresh_template_pack", { id });
      setTemplates((current) => upsertTemplate(current, record));
      setTemplateAcceptance((current) => (current?.recordId === id ? null : current));
      return `${record.name} 已更新`;
    });
  }

  async function relocateTemplate(template: TemplatePackRecord) {
    const selected = await open({
      directory: true,
      multiple: false,
      title: `重新定位 ${template.name}`,
      defaultPath: template.pathAvailable ? template.path : undefined,
    });
    if (typeof selected !== "string") {
      return;
    }
    await runAction("正在重新定位模板包", async () => {
      const record = await invoke<TemplatePackRecord>("relink_template_pack", {
        id: template.id,
        template: selected,
      });
      setTemplates((current) => upsertTemplate(current, record));
      setTemplateAcceptance(null);
      return `${record.name} 已重新定位`;
    });
  }

  async function checkTemplateAcceptance(template: TemplatePackRecord) {
    await runAction("正在检查模板可用性", async () => {
      const [readiness, validation] = await Promise.all([
        invoke<TemplateReadinessReport>("diagnose_template_readiness", {
          template: template.path,
        }),
        invoke<TemplateValidationSummary>("validate_template_pack", {
          template: template.path,
        }),
      ]);
      setTemplateAcceptance({
        recordId: template.id,
        readiness,
        validation,
        tests: null,
      });
      const tests =
        validation.testCaseCount > 0 ? `，包含 ${validation.testCaseCount} 项自测` : "，未配置自测";
      return `${readiness.summary}${tests}`;
    });
  }

  async function runTemplateAcceptanceTests(template: TemplatePackRecord) {
    await runAction("正在运行模板自测", async () => {
      const report = await invoke<TemplateTestReport>("test_template_pack", {
        template: template.path,
        case: null,
        outDir: null,
      });
      setTemplateAcceptance((current) =>
        current?.recordId === template.id ? { ...current, tests: report } : current,
      );
      if (!report.passed) {
        throw new Error(`模板自测失败：通过 ${report.passedCount} 项，失败 ${report.failedCount} 项`);
      }
      return `模板自测通过：${report.passedCount}/${report.selectedCount} 项`;
    });
  }

  async function forgetTemplate(id: string) {
    await runAction("正在移除模板记录", async () => {
      await invoke("forget_template_pack", { id });
      setTemplates((current) => current.filter((item) => item.id !== id));
      return "已从本地模板库移除";
    });
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
    <div className="page-grid workspace-fill-grid template-library-page-grid">
      <Card className="span-7 template-library-card">
        <CardHeader>
          <CardTitle>模板库</CardTitle>
          <p className="muted">链接外部模板包，模板文件始终留在你选择的本地目录。</p>
        </CardHeader>
        <CardContent>
          <div className="template-link-bar">
            <div className="field-row">
              <Input
                value={templatePath}
                onChange={(event) => setTemplatePath(event.target.value)}
                placeholder="选择包含 template.manifest.json 的目录"
              />
              <Button type="button" onClick={chooseTemplateFolder} disabled={isRunning}>
                选择
              </Button>
              <Button variant="primary" onClick={linkTemplate} disabled={isRunning}>
                链接
              </Button>
            </div>
            <div className="action-row">
              <Button onClick={() => setArchiveImportOpen(true)} disabled={isRunning}>
                <Upload size={14} />
                导入模板包
              </Button>
              <Button onClick={() => setConfigOpen(true)} disabled={isRunning}>
                高级创建
              </Button>
              <Button onClick={loadTemplates} disabled={isRunning}>
                刷新
              </Button>
            </div>
          </div>
          <div className="status-line template-library-status">
            <span className={`status-pill ${statusClass(runState.status)}`}>
              {statusLabel(runState.status)}
            </span>
            <span>{runState.message}</span>
          </div>
          <div className="template-library-toolbar">
            <label className="search-field">
              <Search size={14} aria-hidden="true" />
              <Input
                value={templateQuery}
                onChange={(event) => setTemplateQuery(event.target.value)}
                placeholder="搜索名称、ID 或输入协议"
              />
            </label>
            <select
              className="ui-select"
              value={formatFilter}
              onChange={(event) => setFormatFilter(event.target.value)}
              aria-label="按输出格式筛选"
            >
              <option value="all">全部格式</option>
              <option value="pptx">PPTX</option>
              <option value="docx">DOCX</option>
              <option value="xlsx">XLSX</option>
            </select>
            <span className="template-library-count">{templateCountLabel}</span>
          </div>
          <div className="template-list template-select-list" ref={templateListRef}>
            {templates.length === 0 ? (
              <div className="template-drop">
                <strong>还没有链接模板包</strong>
                <span>链接已有目录，或从 Office 文件创建草稿模板包。</span>
              </div>
            ) : filteredTemplateFamilies.length === 0 ? (
              <div className="template-drop">
                <strong>没有匹配的模板</strong>
                <span>调整搜索词或格式筛选。</span>
              </div>
            ) : (
              pagination.items.map((family) => (
                <section className="template-family" key={family.key}>
                  <header className="template-family-header">
                    <div>
                      <strong>{family.name}</strong>
                      <span>{family.schemaId ? `协议 ${family.schemaId}` : "独立模板"}</span>
                    </div>
                    <span>{family.roles.map(templateRoleLabel).join(" / ")}</span>
                  </header>
                  <div className="template-family-records">
                    {family.templates.map((template) => (
                      <button
                        type="button"
                        className={`template-select-row ${
                          selectedTemplate?.id === template.id ? "is-selected" : ""
                        }`}
                        key={template.id}
                        onClick={() => setSelectedTemplateId(template.id)}
                      >
                        <TemplateThumbnail
                          template={template}
                          className="template-select-thumbnail"
                          iconSize={20}
                        />
                        <span className="template-select-copy">
                          <strong>{templateVariantLabel(template, family)}</strong>
                          <span>{formatTemplateStructure(template)}</span>
                        </span>
                        <span className="template-select-meta">
                          <b>{template.format.toUpperCase()}</b>
                          <span>{templateRoleLabel(templateDocumentRole(template))}</span>
                          <span
                            className={
                              !template.pathAvailable
                                ? "is-danger"
                                : template.warnings.length > 0
                                  ? "is-warn"
                                  : "is-ok"
                            }
                          >
                            {!template.pathAvailable
                              ? "路径失效"
                              : template.warnings.length > 0
                                ? "有提示"
                                : "可用"}
                          </span>
                        </span>
                      </button>
                    ))}
                  </div>
                </section>
              ))
            )}
          </div>
          <Pagination
            label="模板系列分页"
            page={pagination.page}
            totalPages={pagination.totalPages}
            startItem={pagination.startItem}
            endItem={pagination.endItem}
            totalItems={pagination.totalItems}
            onPageChange={setCurrentPage}
          />
        </CardContent>
      </Card>
      <Card className="span-5 template-detail-card">
        <CardHeader>
          <CardTitle>模板预览</CardTitle>
          <p className="muted">查看结构与输入协议，不加载或保存业务内容。</p>
        </CardHeader>
        <CardContent>
          {selectedTemplate ? (
            <div className="template-detail-stack">
              <div className="template-detail-hero">
                <TemplateThumbnail
                  template={selectedTemplate}
                  className="template-detail-thumbnail"
                  iconSize={28}
                />
                <div>
                  <strong>{selectedTemplate.name}</strong>
                  <span>{selectedTemplate.templateId}</span>
                </div>
                <span
                  className={`status-pill ${
                    !selectedTemplate.pathAvailable
                      ? "is-danger"
                      : selectedTemplate.warnings.length > 0
                        ? "is-warn"
                        : "is-ok"
                  }`}
                >
                  {!selectedTemplate.pathAvailable
                    ? "路径失效"
                    : selectedTemplate.warnings.length > 0
                      ? "有提示"
                      : "可用"}
                </span>
              </div>
              {!selectedTemplate.pathAvailable ? (
                <div className="template-path-alert">
                  <strong>模板目录不存在</strong>
                  <span>模板可能已移动或改名。重新定位后会保留当前记录和历史关系。</span>
                </div>
              ) : null}
              <div className="template-detail-primary-actions">
                <Button
                  variant="primary"
                  onClick={() => useTemplate(selectedTemplate)}
                  disabled={isRunning || !selectedTemplate.pathAvailable}
                >
                  <Play size={15} />
                  使用此模板
                </Button>
                <Button onClick={() => relocateTemplate(selectedTemplate)} disabled={isRunning}>
                  <FolderOpen size={15} />
                  重新定位
                </Button>
                <Button
                  onClick={() => openTemplateFile(selectedTemplate)}
                  disabled={isRunning || !selectedTemplate.pathAvailable || !selectedPrimaryPath}
                >
                  <FileText size={15} />
                  打开模板文件
                </Button>
              </div>
              {selectedFamily && selectedFamily.templates.length > 1 ? (
                <div className="template-style-comparison" aria-label="模板样式对比">
                  <header>
                    <div>
                      <strong>样式对比</strong>
                      <span>{selectedFamily.templates.length} 个同族模板</span>
                    </div>
                  </header>
                  <div className="template-style-strip">
                    {selectedFamily.templates.map((template) => (
                      <button
                        type="button"
                        key={template.id}
                        className={`template-style-card${
                          selectedTemplate.id === template.id ? " is-selected" : ""
                        }`}
                        onClick={() => setSelectedTemplateId(template.id)}
                        disabled={!template.pathAvailable}
                      >
                        <TemplateThumbnail
                          template={template}
                          className="template-style-thumbnail"
                          iconSize={20}
                        />
                        <span>
                          <strong>{templateVariantLabel(template, selectedFamily)}</strong>
                          <small>
                            {templateRoleLabel(templateDocumentRole(template))} · {formatTemplateStructure(template)}
                          </small>
                        </span>
                      </button>
                    ))}
                  </div>
                </div>
              ) : null}
              <dl className="definition-list compact template-detail-facts">
                <div>
                  <dt>类型</dt>
                  <dd>{templateTypeLabel(selectedTemplate.templateType)} · {selectedTemplate.format.toUpperCase()}</dd>
                </div>
                <div>
                  <dt>文档用途</dt>
                  <dd>{templateRoleLabel(templateDocumentRole(selectedTemplate))}</dd>
                </div>
                <div>
                  <dt>输入协议</dt>
                  <dd>{formatInputContract(selectedTemplate)}</dd>
                </div>
                <div>
                  <dt>渲染方式</dt>
                  <dd>{formatRenderer(selectedTemplate)}</dd>
                </div>
                <div>
                  <dt>最近校验</dt>
                  <dd>{formatDate(selectedTemplate.lastValidatedAt)}</dd>
                </div>
                <div>
                  <dt>本地路径</dt>
                  <dd className="path-text">{selectedTemplate.path}</dd>
                </div>
              </dl>
              <TemplateDetailSection
                id="template-structure-section"
                title="结构与提示"
                meta={
                  selectedStructureGroups.length +
                  " 组 · " +
                  selectedStructureItemCount +
                  " 项" +
                  (selectedTemplate.warnings.length > 0
                    ? " · " + selectedTemplate.warnings.length + " 条提示"
                    : "")
                }
                open={openDetailSections.structure}
                onToggle={() => toggleDetailSection("structure")}
              >
                <div className="template-structure-preview">
                  {selectedStructureGroups.map((group) => (
                    <section key={group.label}>
                      <header>
                        <strong>{group.label}</strong>
                        <span>{group.ids.length} 项</span>
                      </header>
                      <div className="template-id-list">
                        {group.ids.length > 0 ? (
                          group.ids.map((id) => <code key={id}>{id}</code>)
                        ) : (
                          <span>由脚本或模板包自行处理</span>
                        )}
                      </div>
                    </section>
                  ))}
                </div>
                {selectedTemplate.warnings.length > 0 ? (
                  <ul className="warning-list">
                    {selectedTemplate.warnings.map((warning) => (
                      <li key={warning}>{warning}</li>
                    ))}
                  </ul>
                ) : null}
              </TemplateDetailSection>
              <TemplateDetailSection
                id="template-acceptance-section"
                title="模板验收"
                meta="文件、引擎、运行时和模板内自测"
                badge={
                  <span className={`status-pill ${selectedAcceptanceStatus.className}`}>
                    {selectedAcceptanceStatus.label}
                  </span>
                }
                open={openDetailSections.acceptance}
                onToggle={() => toggleDetailSection("acceptance")}
              >
                <div className="template-acceptance-actions">
                  <Button
                    size="sm"
                    variant={selectedAcceptance ? "secondary" : "primary"}
                    onClick={() => checkTemplateAcceptance(selectedTemplate)}
                    disabled={isRunning || !selectedTemplate.pathAvailable}
                  >
                    <Activity size={14} />
                    {selectedAcceptance ? "重新检查" : "检查可用性"}
                  </Button>
                  {selectedAcceptance && selectedAcceptance.validation.testCaseCount > 0 ? (
                    <Button
                      size="sm"
                      onClick={() => runTemplateAcceptanceTests(selectedTemplate)}
                      disabled={isRunning || !selectedAcceptance.readiness.canRender}
                    >
                      <Play size={14} />
                      运行 {selectedAcceptance.validation.testCaseCount} 项自测
                    </Button>
                  ) : null}
                  {selectedTestReport ? (
                    <Button
                      size="sm"
                      onClick={() => openTemplateFolder(selectedTestReport.outputDir)}
                      disabled={isRunning}
                    >
                      <FolderOpen size={14} />
                      打开测试产物
                    </Button>
                  ) : null}
                </div>
                {selectedAcceptance ? (
                  <div className="template-acceptance-result">
                    <p>{selectedAcceptance.readiness.summary}</p>
                    <div className="template-acceptance-metrics">
                      <span>
                        阻断 <strong>{selectedAcceptanceIssues.filter((item) => item.status === "fail").length}</strong>
                      </span>
                      <span>
                        提示 <strong>{selectedAcceptanceIssues.filter((item) => item.status === "warn").length}</strong>
                      </span>
                      <span>
                        自测 <strong>{selectedAcceptance.validation.testCaseCount}</strong>
                      </span>
                    </div>
                    {selectedAcceptanceIssues.length > 0 ? (
                      <div className="template-acceptance-issues">
                        {selectedAcceptanceIssues.map((issue) => (
                          <div key={issue.id}>
                            <span className={`status-pill ${issue.status === "fail" ? "is-danger" : "is-warn"}`}>
                              {issue.status === "fail" ? "阻断" : "提示"}
                            </span>
                            <p>
                              <strong>{issue.label}</strong>
                              <span>{issue.message}</span>
                              {issue.fixHint ? <small>{issue.fixHint}</small> : null}
                            </p>
                          </div>
                        ))}
                      </div>
                    ) : null}
                    {selectedAcceptance.validation.testCaseCount === 0 ? (
                      <small className="template-acceptance-note">
                        该模板没有声明 tests，自测不是必需项；仍可按可用性结果生成。
                      </small>
                    ) : null}
                    {selectedTestReport ? (
                      <div className="template-test-cases">
                        {selectedTestReport.cases.map((testCase) => (
                          <div key={testCase.id}>
                            <span className={`status-pill ${testCase.passed ? "is-ok" : "is-danger"}`}>
                              {testCase.passed ? "通过" : "失败"}
                            </span>
                            <p>
                              <strong>{testCase.id}</strong>
                              <span>{testCase.durationMs} ms</span>
                              {testCase.error ? <small>{testCase.error}</small> : null}
                            </p>
                          </div>
                        ))}
                      </div>
                    ) : null}
                  </div>
                ) : (
                  <p className="template-acceptance-empty">
                    导入模板后先检查一次，即可确认它在当前电脑上能否生成。
                  </p>
                )}
              </TemplateDetailSection>
              <TemplateDetailSection
                id="template-management-section"
                title="管理与导出"
                meta="归档、契约、本地目录与移除"
                open={openDetailSections.management}
                onToggle={() => toggleDetailSection("management")}
              >
                <div className="template-detail-secondary-actions">
                  <Button
                    size="sm"
                    onClick={() => exportTemplateArchive(selectedTemplate)}
                    disabled={isRunning || !selectedTemplate.pathAvailable}
                  >
                    <Archive size={14} />
                    导出模板包
                  </Button>
                  {lastArchiveExport?.templateId === selectedTemplate.id ? (
                    <Button
                      size="sm"
                      onClick={() => revealArchive(lastArchiveExport.path)}
                      disabled={isRunning}
                    >
                      <Download size={14} />
                      定位导出
                    </Button>
                  ) : null}
                  <Button
                    size="sm"
                    onClick={() => openTemplateFolder(selectedTemplate.path)}
                    disabled={isRunning || !selectedTemplate.pathAvailable}
                  >
                    <FolderOpen size={14} />
                    打开目录
                  </Button>
                  <Button
                    size="sm"
                    onClick={() => exportContract(selectedTemplate)}
                    disabled={isRunning || !selectedTemplate.pathAvailable}
                  >
                    导出契约
                  </Button>
                  {canExportSkeleton(selectedTemplate) ? (
                    <Button
                      size="sm"
                      onClick={() => exportSkeleton(selectedTemplate, "minimal")}
                      disabled={isRunning || !selectedTemplate.pathAvailable}
                    >
                      导出内容骨架
                    </Button>
                  ) : null}
                  <Button
                    size="sm"
                    onClick={() => refreshTemplate(selectedTemplate.id)}
                    disabled={isRunning || !selectedTemplate.pathAvailable}
                  >
                    重新校验
                  </Button>
                  <Button
                    size="sm"
                    variant="danger"
                    onClick={() => forgetTemplate(selectedTemplate.id)}
                    disabled={isRunning}
                  >
                    移除记录
                  </Button>
                </div>
                <p className="template-privacy-note">
                  仅保存路径和结构信息，模板文件不会进入应用安装包。
                </p>
              </TemplateDetailSection>
            </div>
          ) : (
            <div className="template-drop">
              <strong>选择一个模板</strong>
              <span>结构、输入协议和可用操作会显示在这里。</span>
            </div>
          )}
        </CardContent>
      </Card>
      <ConfigDialog
        open={archiveImportOpen}
        title="导入模板包"
        description="把 .rdeckpack 解包到你指定的外部目录，并自动加入模板库。"
        onClose={() => setArchiveImportOpen(false)}
      >
        <div className="form-section archive-import-form">
          <div className="form-stack">
            <label>
              模板包文件
              <div className="field-row">
                <Input
                  value={archivePath}
                  onChange={(event) => setArchivePath(event.target.value)}
                  placeholder="选择 .rdeckpack 文件"
                />
                <Button type="button" onClick={chooseTemplateArchive} disabled={isRunning}>
                  选择
                </Button>
              </div>
            </label>
            <label>
              外部模板根目录
              <div className="field-row">
                <Input
                  value={archiveRoot}
                  onChange={(event) => setArchiveRoot(event.target.value)}
                  placeholder="模板会解包到该目录下"
                />
                <Button type="button" onClick={chooseArchiveRoot} disabled={isRunning}>
                  选择
                </Button>
              </div>
            </label>
            <label className="archive-overwrite-option">
              <span>
                <strong>覆盖同名模板</strong>
                <small>目标目录已存在时，先替换旧模板再导入。</small>
              </span>
              <Switch
                checked={archiveForce}
                onChange={(event) => setArchiveForce(event.target.checked)}
                disabled={isRunning}
                aria-label="覆盖同名模板"
              />
            </label>
          </div>
          <div className="action-row archive-import-actions">
            <Button onClick={() => setArchiveImportOpen(false)} disabled={isRunning}>
              取消
            </Button>
            <Button
              variant="primary"
              onClick={importTemplateArchive}
              disabled={isRunning || !archivePath.trim() || !archiveRoot.trim()}
            >
              <Upload size={14} />
              导入并链接
            </Button>
          </div>
        </div>
      </ConfigDialog>
      <ConfigDialog
        open={configOpen}
        title="模板高级创建"
        description="从 Office 文件生成草稿包，或把外部脚本封装成模板 adapter。"
        onClose={() => setConfigOpen(false)}
      >
        <div className="form-section">
          <div className="section-heading">
            <strong>从 Office 文件生成草稿模板包</strong>
            <span>扫描页面、占位符或命名区域，在外部目录生成可继续编辑的 manifest。</span>
          </div>
          <div className="form-stack">
            <label>
              文件格式
              <select
                className="ui-select"
                value={draftFormat}
                onChange={(event) => changeDraftFormat(event.target.value as OfficeTemplateFormat)}
              >
                <option value="pptx">PPTX 页面模板</option>
                <option value="docx">DOCX 文档模板</option>
                <option value="xlsx">XLSX 工作簿模板</option>
              </select>
            </label>
            <label>
              {draftFormat.toUpperCase()} 模板文件
              <div className="field-row">
                <Input
                  value={officeTemplatePath}
                  onChange={(event) => setOfficeTemplatePath(event.target.value)}
                  placeholder={`选择 .${draftFormat} 文件`}
                />
                <Button type="button" onClick={chooseOfficeTemplateFile} disabled={isRunning}>
                  选择
                </Button>
              </div>
            </label>
            <label>
              草稿模板包目录
              <div className="field-row">
                <Input value={draftDir} onChange={(event) => setDraftDir(event.target.value)} />
                <Button type="button" onClick={chooseDraftFolder} disabled={isRunning}>
                  选择
                </Button>
              </div>
            </label>
          </div>
          <div className="action-row">
            <Button
              variant="primary"
              onClick={createOfficeDraft}
              disabled={isRunning || !officeTemplatePath.trim() || !draftDir.trim()}
            >
              生成并链接
            </Button>
          </div>
        </div>
        <div className="form-section">
          <div className="section-heading">
            <strong>创建脚本型模板包</strong>
            <span>生成外部 adapter 模板包，并把脚本复制到包内以便归档和迁移。</span>
          </div>
          <div className="form-stack">
            <label>
              脚本入口
              <div className="field-row">
                <Input value={scriptPath} onChange={(event) => setScriptPath(event.target.value)} />
                <Button type="button" onClick={chooseScriptFile} disabled={isRunning}>
                  选择
                </Button>
              </div>
            </label>
            <label>
              模板包目录
              <div className="field-row">
                <Input
                  value={scriptAdapterDir}
                  onChange={(event) => setScriptAdapterDir(event.target.value)}
                />
                <Button type="button" onClick={chooseScriptAdapterFolder} disabled={isRunning}>
                  选择
                </Button>
              </div>
            </label>
            <div className="split-fields">
              <label>
                模板 ID
                <Input
                  value={scriptTemplateId}
                  onChange={(event) => setScriptTemplateId(event.target.value)}
                />
              </label>
              <label>
                模板名称
                <Input
                  value={scriptTemplateName}
                  onChange={(event) => setScriptTemplateName(event.target.value)}
                />
              </label>
            </div>
            <div className="split-fields">
              <label>
                输出格式
                <select
                  className="ui-select"
                  value={scriptFormat}
                  onChange={(event) => setScriptFormat(event.target.value)}
                >
                  <option value="pptx">PPTX</option>
                  <option value="docx">DOCX</option>
                  <option value="xlsx">XLSX</option>
                </select>
              </label>
              <label>
                输入格式
                <select
                  className="ui-select"
                  value={scriptInputFormat}
                  onChange={(event) => setScriptInputFormat(event.target.value)}
                >
                  <option value="json">JSON</option>
                  <option value="md">Markdown</option>
                </select>
              </label>
              <label>
                运行时
                <Input
                  value={scriptRuntime}
                  onChange={(event) => setScriptRuntime(event.target.value)}
                  placeholder="自动推断"
                />
              </label>
            </div>
          </div>
          <div className="action-row">
            <Button variant="primary" onClick={() => createScriptAdapter(false)} disabled={isRunning}>
              生成并链接
            </Button>
            <Button onClick={() => createScriptAdapter(true)} disabled={isRunning}>
              覆盖并链接
            </Button>
          </div>
        </div>
      </ConfigDialog>
    </div>
  );
}

type TemplateDetailSectionProps = {
  id: string;
  title: string;
  meta: string;
  badge?: ReactNode;
  open: boolean;
  onToggle: () => void;
  children: ReactNode;
};

function TemplateDetailSection({
  id,
  title,
  meta,
  badge,
  open,
  onToggle,
  children,
}: TemplateDetailSectionProps) {
  return (
    <section className={`template-detail-section${open ? " is-open" : ""}`}>
      <button
        type="button"
        className="template-detail-section-trigger"
        aria-expanded={open}
        aria-controls={id}
        onClick={onToggle}
      >
        <span>
          <strong>{title}</strong>
          <small>{meta}</small>
        </span>
        <span className="template-detail-section-state">
          {badge}
          <ChevronDown size={15} aria-hidden="true" />
        </span>
      </button>
      {open ? (
        <div className="template-detail-section-body" id={id}>
          {children}
        </div>
      ) : null}
    </section>
  );
}

function upsertTemplate(current: TemplatePackRecord[], record: TemplatePackRecord) {
  const next = current.filter((item) => item.id !== record.id);
  return [record, ...next];
}

function templateArtifactPath(template: TemplatePackRecord, fileName: string) {
  return `${trimTrailingSlash(template.path)}/${fileName}`;
}

function trimTrailingSlash(value: string) {
  return value.replace(/[\\/]+$/g, "");
}

function defaultRecipeForSkeleton(template: TemplatePackRecord) {
  if (template.format === "docx") {
    return template.documentRecipeIds[0] || undefined;
  }
  if (template.format === "xlsx") {
    return template.workbookRecipeIds[0] || undefined;
  }
  return undefined;
}

function canExportSkeleton(template: TemplatePackRecord) {
  if (template.format === "pptx") {
    return template.pageTemplateCount > 0;
  }
  if (template.format === "docx") {
    return template.blockTemplateCount > 0 && template.documentRecipeCount > 0;
  }
  if (template.format === "xlsx") {
    return template.sheetTemplateCount > 0 && template.workbookRecipeCount > 0;
  }
  return false;
}

function buildDraftDir(officePath: string) {
  if (!officePath.trim()) {
    return "";
  }
  const normalized = officePath.replace(/\\/g, "/");
  const separator = normalized.lastIndexOf("/");
  const parent = separator >= 0 ? normalized.slice(0, separator) : "";
  const directoryName = `${fileStem(officePath)}-draft-${timestampLabel()}`;
  return parent ? `${parent}/${directoryName}` : directoryName;
}

function fileStem(path: string) {
  const fileName = path.split(/[\\/]/).filter(Boolean).pop() ?? "office-template";
  return (
    fileName
      .replace(/\.(pptx|docx|xlsx)$/i, "")
      .replace(/[^\p{L}\p{N}_-]+/gu, "-")
      .replace(/^-+|-+$/g, "")
      .slice(0, 80) || "office-template"
  );
}

function officeDraftSummary(
  format: OfficeTemplateFormat,
  inspection: OfficeTemplatePackDraft["inspection"],
) {
  if (format === "docx") {
    const bindable = inspection.placeholders?.filter((item) => item.bindable).length ?? 0;
    return `${inspection.placeholderCount ?? 0} 个占位符，${bindable} 个可直接绑定`;
  }
  if (format === "xlsx") {
    const bindable = inspection.namedRanges?.filter((item) => item.bindable).length ?? 0;
    return `${inspection.sheetCount ?? 0} 个工作表，${bindable} 个可绑定命名区域`;
  }
  const bindable =
    inspection.slides?.reduce((total, slide) => total + slide.bindableShapeCount, 0) ?? 0;
  return `${inspection.slideCount ?? 0} 页，${bindable} 个可绑定对象`;
}

function timestampLabel() {
  const now = new Date();
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${now.getFullYear()}${pad(now.getMonth() + 1)}${pad(now.getDate())}-${pad(
    now.getHours(),
  )}${pad(now.getMinutes())}${pad(now.getSeconds())}`;
}

function formatDate(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return date.toLocaleString();
}

function formatTemplateStructure(template: TemplatePackRecord) {
  const typeLabel = templateTypeLabel(template.templateType);
  if (template.format === "docx") {
    return `${typeLabel}；${template.blockTemplateCount} 个文档块，${template.documentRecipeCount} 个文档结构`;
  }
  if (template.format === "xlsx") {
    return `${typeLabel}；${template.sheetTemplateCount} 个工作表模板，${template.workbookRecipeCount} 个工作簿结构`;
  }
  return `${typeLabel}；${template.pageTemplateCount} 个页面模板，${template.deckRecipeCount} 个 PPT 结构`;
}

function formatRenderer(template: TemplatePackRecord) {
  if (template.rendererType) {
    const entry = template.rendererEntry ? `；${template.rendererEntry}` : "";
    const pipeline =
      template.pipelineStepCount > 0 ? `；${template.pipelineStepCount} 个流水线步骤` : "";
    return `${template.rendererType}${entry}${pipeline}`;
  }
  if (template.pipelineStepCount > 0) {
    return `${template.pipelineStepCount} 个流水线步骤`;
  }
  return "内置声明式渲染器";
}

function templateTypeLabel(templateType: string | null | undefined) {
  if (templateType === "script") {
    return "脚本型";
  }
  if (templateType === "hybrid") {
    return "混合型";
  }
  return "声明式";
}

function formatInputContract(template: TemplatePackRecord) {
  const formats = template.inputFormats.length > 0 ? template.inputFormats.join(", ") : "json";
  const schema = template.inputSchemaId ?? template.inputSchema;
  const profile = template.inputProfileName ?? template.inputProfile;
  if (profile && schema && template.mdProfile) {
    return `${formats}; ${profile}; 协议 ${schema}; Markdown ${template.mdProfile}`;
  }
  if (profile && schema) {
    return `${formats}; ${profile}; 协议 ${schema}`;
  }
  if (profile) {
    return `${formats}; ${profile}`;
  }
  if (schema && template.mdProfile) {
    return `${formats}; 协议 ${schema}; Markdown ${template.mdProfile}`;
  }
  if (schema) {
    return `${formats}; 协议 ${schema}`;
  }
  if (template.mdProfile) {
    return `${formats}; Markdown ${template.mdProfile}`;
  }
  return formats;
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
