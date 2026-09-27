export type TemplatePackRecord = {
  id: string;
  path: string;
  pathAvailable: boolean;
  templateId: string;
  name: string;
  familyId?: string | null;
  role?: string | null;
  format: string;
  templateType: string;
  entryPath: string;
  rendererType?: string | null;
  rendererEntry?: string | null;
  pipelineStepCount: number;
  pageTemplateCount: number;
  deckRecipeCount: number;
  blockTemplateCount: number;
  documentRecipeCount: number;
  sheetTemplateCount: number;
  workbookRecipeCount: number;
  pageTemplateIds: string[];
  deckRecipeIds: string[];
  blockTemplateIds: string[];
  documentRecipeIds: string[];
  sheetTemplateIds: string[];
  workbookRecipeIds: string[];
  legacyLayoutCount: number;
  inputFormats: string[];
  inputProfile?: string | null;
  inputProfileName?: string | null;
  inputSchema?: string | null;
  inputSchemaId?: string | null;
  inputBuiltinSchema?: boolean;
  mdProfile?: string | null;
  previewCover?: string | null;
  previewSlides?: TemplatePreviewSlide[];
  healthStatus?: string;
  healthChecked?: string[];
  healthWarnings?: string[];
  warnings: string[];
  linkedAt: string;
  lastValidatedAt: string;
};

export type TemplatePreviewSlide = {
  title?: string | null;
  image: string;
};

export type TemplateFamily = {
  key: string;
  name: string;
  format: string;
  familyId?: string;
  schemaId?: string;
  roles: string[];
  templates: TemplatePackRecord[];
};

export type TemplateStructureGroup = {
  label: string;
  ids: string[];
};

export function groupTemplateFamilies(records: TemplatePackRecord[]): TemplateFamily[] {
  const groups = new Map<string, TemplatePackRecord[]>();

  for (const record of records) {
    const key = templateFamilyKey(record);
    groups.set(key, [...(groups.get(key) ?? []), record]);
  }

  return Array.from(groups, ([key, templates]) => ({
    key,
    name: templateFamilyName(templates),
    format: templates[0]?.format ?? "pptx",
    familyId: templates.find((template) => template.familyId?.trim())?.familyId?.trim(),
    schemaId: templates[0]?.inputSchemaId?.trim() || undefined,
    roles: Array.from(new Set(templates.map(templateDocumentRole))).sort(),
    templates,
  }));
}

export function filterTemplateFamilies(
  families: TemplateFamily[],
  query: string,
  format: string,
) {
  const normalizedQuery = query.trim().toLocaleLowerCase();
  const normalizedFormat = format.trim().toLocaleLowerCase();

  return families
    .map((family) => ({
      ...family,
      templates: family.templates.filter((template) => {
        if (normalizedFormat && normalizedFormat !== "all" && template.format !== normalizedFormat) {
          return false;
        }
        if (!normalizedQuery) {
          return true;
        }
        return [
          template.name,
          template.templateId,
          template.inputSchemaId ?? "",
          template.inputProfileName ?? "",
        ].some((value) => value.toLocaleLowerCase().includes(normalizedQuery));
      }),
    }))
    .filter((family) => family.templates.length > 0);
}

export function templateStructureGroups(record: TemplatePackRecord): TemplateStructureGroup[] {
  if (record.format === "docx") {
    return [
      { label: "文档块", ids: record.blockTemplateIds },
      { label: "文档结构", ids: record.documentRecipeIds },
    ];
  }
  if (record.format === "xlsx") {
    return [
      { label: "工作表", ids: record.sheetTemplateIds },
      { label: "工作簿结构", ids: record.workbookRecipeIds },
    ];
  }
  return [
    { label: "页面模板", ids: record.pageTemplateIds },
    { label: "PPT 结构", ids: record.deckRecipeIds },
  ];
}

export function templatePrimaryPath(record: TemplatePackRecord) {
  const candidate = record.entryPath.trim() || record.rendererEntry?.trim() || "";
  if (!candidate) {
    return null;
  }
  if (/^(?:[a-zA-Z]:[\\/]|[\\/])/.test(candidate)) {
    return candidate;
  }
  return `${record.path.replace(/[\\/]+$/g, "")}/${candidate.replace(/^[\\/]+/g, "")}`;
}

export function templatePreviewImage(record: TemplatePackRecord | null | undefined) {
  return record?.previewCover?.trim() || record?.previewSlides?.[0]?.image?.trim() || "";
}

export function templatePreviewSlides(record: TemplatePackRecord | null | undefined) {
  return record?.previewSlides?.filter((slide) => slide.image.trim()) ?? [];
}

export function templateFamilyKey(record: TemplatePackRecord) {
  const familyId = record.familyId?.trim();
  if (familyId) {
    return `family:${familyId}`;
  }
  const schemaId = record.inputSchemaId?.trim();
  return schemaId
    ? `${record.format.toLowerCase()}:schema:${schemaId}`
    : `${record.format.toLowerCase()}:template:${record.id}`;
}

export function templateDocumentRole(record: TemplatePackRecord) {
  const role = record.role?.trim();
  if (role) {
    return role;
  }
  if (record.format === "docx") {
    return "document";
  }
  if (record.format === "xlsx") {
    return "workbook";
  }
  return "slides";
}

export function templateRoleLabel(role: string) {
  if (role === "slides") return "课件";
  if (role === "document") return "文档";
  if (role === "workbook") return "表格";
  if (role === "lesson_plan") return "教案";
  if (role === "assessment") return "考核";
  if (role === "report") return "报告";
  return role;
}

export function templateVariantLabel(record: TemplatePackRecord, family: TemplateFamily) {
  const prefix = `${family.name} · `;
  return record.name.startsWith(prefix) ? record.name.slice(prefix.length) : record.name;
}

export function templateVariantSlug(record: TemplatePackRecord, family: TemplateFamily) {
  const idParts = family.templates.map((template) => template.templateId.split("-"));
  const commonPartCount = commonPrefixLength(idParts);
  const variant = record.templateId.split("-").slice(commonPartCount).join("-");
  return sanitizeFilePart(variant || record.templateId);
}

export function templateSupportsContent(record: TemplatePackRecord, contentPath: string) {
  if (!record.pathAvailable) {
    return false;
  }
  const extension = contentPath.split(".").pop()?.toLowerCase();
  const contentFormat = extension === "markdown" ? "md" : extension;
  if (!contentFormat || !["json", "md"].includes(contentFormat)) {
    return true;
  }
  const formats = record.inputFormats.length > 0 ? record.inputFormats : ["json"];
  return formats.some((format) => {
    const normalized = format.toLowerCase() === "markdown" ? "md" : format.toLowerCase();
    return normalized === contentFormat;
  });
}

export function firstRecipe(record: TemplatePackRecord) {
  if (
    record.templateType === "script" ||
    (record.templateType === "hybrid" && Boolean(record.rendererType))
  ) {
    return "";
  }
  return recipeIdsForTemplate(record)[0] ?? "";
}

export function recipeIdsForTemplate(record: TemplatePackRecord) {
  if (record.format === "docx") {
    return record.documentRecipeIds;
  }
  if (record.format === "xlsx") {
    return record.workbookRecipeIds;
  }
  return record.deckRecipeIds;
}

function templateFamilyName(templates: TemplatePackRecord[]) {
  if (templates.length === 1) {
    return templates[0].name;
  }

  const names = templates.map((template) => template.name);
  const dotPrefixes = names.map((name) => name.split(" · ")[0]?.trim() ?? "");
  if (dotPrefixes[0] && dotPrefixes.every((prefix) => prefix === dotPrefixes[0])) {
    return dotPrefixes[0];
  }

  const profileName = templates.find((template) => template.inputProfileName)?.inputProfileName;
  return profileName?.trim() || templates[0].inputSchemaId?.trim() || templates[0].name;
}

function commonPrefixLength(values: string[][]) {
  if (values.length === 0) {
    return 0;
  }
  const shortestLength = Math.min(...values.map((value) => value.length));
  let index = 0;
  while (index < shortestLength && values.every((value) => value[index] === values[0][index])) {
    index += 1;
  }
  return index;
}

function sanitizeFilePart(value: string) {
  return (
    value
      .replace(/[^a-zA-Z0-9_-]+/g, "-")
      .replace(/^-+|-+$/g, "")
      .toLowerCase() || "variant"
  );
}
