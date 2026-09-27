import { mkdir, readFile, writeFile } from "node:fs/promises";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { basename, dirname, extname, isAbsolute, posix as pathPosix, resolve } from "node:path";
import {
  Automizer,
  ModifyChartHelper,
  ModifyImageHelper,
  ModifyTableHelper,
  ModifyTextHelper,
  type ChartData,
  type ISlide,
  type ModifyTableParams,
  type ShapeModificationCallback,
  type TableData,
  type TableRow,
  type TableRowStyle,
} from "pptx-automizer";
import JSZip from "jszip";

export type RenderJob = {
  jobId: string;
  createdAt: string;
  format: "pptx";
  deckRecipe?: string;
  recipeStepCount: number;
  plannedPages: Array<{
    pageIndex: number;
    pageTemplate: string;
    sourceSlide: number;
    dataPath: string;
    repeatIndex?: number;
    dataValue?: unknown;
  }>;
  templateDir: string;
  manifestFile: string;
  contentFile: string;
  contentRoot?: string;
  contentFormat?: string;
  contentValue?: unknown;
  markdownFile?: string;
  outputFile: string;
};

type TemplateManifest = {
  schemaVersion: string;
  templateId: string;
  name: string;
  format: "pptx";
  entry: string;
  pageTemplates?: Record<string, PageTemplate>;
};

type PageTemplate = {
  sourceSlide: number;
  description?: string;
  bindings?: Record<string, PageBinding>;
};

type PageBinding = {
  shapeName?: string;
  creationId?: string;
  dataPath?: string;
  type?: "text" | "list" | "image" | "backgroundImage" | "table" | "chart" | string;
  required?: boolean;
  maxLength?: number;
  maxItems?: number;
  fit?: "cover" | "contain" | "stretch" | string;
  chartMode?: "image" | "native" | string;
  chartOptions?: ChartBindingOptions;
  tableOptions?: TableBindingOptions;
};

type ChartBindingOptions = {
  title?: boolean | string;
  removeLegend?: boolean;
  minimizeLegend?: boolean;
  axisRange?: {
    min?: number;
    max?: number;
    majorUnit?: number;
    minorUnit?: number;
  };
};

type TableBindingOptions = {
  adjustWidth?: boolean;
  adjustHeight?: boolean;
  styleId?: string;
  styleAttribs?: string[];
  headerStyle?: TableCellStyleSpec;
  bodyStyle?: TableCellStyleSpec;
  columnWidths?: number[];
  rowHeights?: number[];
  expand?: Array<{
    tag: string;
    mode: "row" | "column";
    count: number;
  }>;
};

type TableCellStyleSpec = {
  color?: string;
  background?: string;
  bold?: boolean;
  italic?: boolean;
  underline?: boolean;
  size?: number;
  borderColor?: string;
  borderWeight?: number;
};

type RenderResult = {
  rendererStatus: "rendered";
  jobId: string;
  deckRecipe: string | null;
  plannedPageCount: number;
  outputFile: string;
  warnings: string[];
  summary: unknown;
  cleanup: PptxCleanupSummary;
  contentCheck: ContentCheckSummary;
};

type PptxCleanupSummary = {
  visibleSlideCount: number;
  slidePartCount: number;
  removedSlideRelationships: number;
  removedSlideParts: number;
  removedSlideRelationshipParts: number;
  removedNotesSlideParts: number;
  removedNotesSlideRelationshipParts: number;
  renamedSlideParts: number;
  renamedNotesSlideParts: number;
  removedMediaRelationships: number;
  removedPresentationLayoutRelationships: number;
  removedContentTypeOverrides: number;
  removedDuplicateContentTypeOverrides: number;
  updatedAppProperties: number;
  svgFallbackRelationships: number;
  removedSvgBlipExtensions: number;
  removedChartRelationships: number;
  removedChartParts: number;
  removedChartRelationshipParts: number;
  removedEmbeddedWorkbookParts: number;
};

type ExpectedBinding = {
  pageIndex: number;
  pageTemplate: string;
  bindingId: string;
  expectedText: string;
};

type ContentCheckSummary = {
  status: "passed" | "failed";
  checkedPages: number;
  checkedBindings: number;
  missing: Array<{
    pageIndex: number;
    pageTemplate: string;
    bindingId: string;
    expectedText: string;
  }>;
};

export async function renderPptx(job: RenderJob): Promise<RenderResult> {
  if (job.format !== "pptx") {
    throw new Error(`unsupported render job format: ${job.format}`);
  }
  if (job.plannedPages.length === 0) {
    throw new Error("render job has no plannedPages");
  }

  const manifest = await readJson<TemplateManifest>(job.manifestFile);
  const content = job.contentValue ?? (await readJson<unknown>(job.contentFile));
  if (manifest.format !== "pptx") {
    throw new Error(`template manifest format is not pptx: ${manifest.format}`);
  }
  const expectedBindings = buildExpectedBindings(job, manifest, content, []);

  const outputDir = dirname(job.outputFile);
  await mkdir(outputDir, { recursive: true });

  const warnings: string[] = [];
  const automizer = new Automizer({
    templateDir: job.templateDir,
    outputDir,
    removeExistingSlides: true,
    cleanup: true,
    autoImportSlideMasters: true,
  });

  automizer.loadRoot(manifest.entry);
  automizer.load(manifest.entry, "template");
  const loadedMedia = new Set<string>();

  for (const page of job.plannedPages) {
    const pageTemplate = manifest.pageTemplates?.[page.pageTemplate];
    if (!pageTemplate) {
      throw new Error(`missing pageTemplate in manifest: ${page.pageTemplate}`);
    }
    const pageData =
      page.dataValue === undefined ? resolveDataPath(content, page.dataPath, content) : page.dataValue;
    automizer.addSlide("template", pageTemplate.sourceSlide, (slide: ISlide) => {
      for (const [bindingId, binding] of Object.entries(pageTemplate.bindings ?? {})) {
        if (!binding.shapeName) {
          warnings.push(
            `page ${page.pageIndex} binding '${bindingId}' has no shapeName; creationId rendering is not implemented yet`,
          );
          continue;
        }
        const value = resolveBindingValue(pageData, binding, bindingId, page.pageIndex);
        if (value === undefined || value === null) {
          if (binding.required) {
            throw new Error(
              `page ${page.pageIndex} binding '${bindingId}' is required but resolved empty`,
            );
          }
          continue;
        }
        const bindingType = binding.type ?? "text";
        if (bindingType === "image" || bindingType === "backgroundImage") {
          const mediaName = loadImageBindingMedia(
            automizer,
            job,
            value,
            loadedMedia,
            bindingId,
            page.pageIndex,
          );
          const replaceImage = replaceImageRelation(mediaName);
          slide.modifyElement(binding.shapeName, [replaceImage]);
          continue;
        }
        if (bindingType === "chart" && binding.chartMode === "image") {
          const mediaName = loadChartImageMedia(
            automizer,
            job,
            value,
            loadedMedia,
            bindingId,
            page.pageIndex,
          );
          const replaceImage = replaceImageRelation(mediaName);
          slide.modifyElement(binding.shapeName, [replaceImage]);
          continue;
        }
        if (bindingType === "chart") {
          const chartData = formatNativeChartData(value, page.pageIndex, bindingId);
          const modifications = buildNativeChartModifications(
            chartData,
            value,
            binding.chartOptions,
          );
          slide.modifyElement(binding.shapeName, modifications);
          continue;
        }
        if (bindingType === "table") {
          const tableData = formatTableData(
            value,
            binding.tableOptions,
            warnings,
            page.pageIndex,
            bindingId,
          );
          slide.modifyElement(
            binding.shapeName,
            buildTableModifications(tableData, binding.tableOptions),
          );
          continue;
        }
        const text = formatBindingValue(value, binding, warnings, page.pageIndex, bindingId);
        slide.modifyElement(binding.shapeName, [ModifyTextHelper.setText(text)]);
      }
    });
  }

  const summary = await automizer.write(basename(job.outputFile));
  const cleanup = await cleanupPptxPackage(job.outputFile);
  const contentCheck = await checkRenderedContent(job.outputFile, expectedBindings);
  return {
    rendererStatus: "rendered",
    jobId: job.jobId,
    deckRecipe: job.deckRecipe ?? null,
    plannedPageCount: job.plannedPages.length,
    outputFile: job.outputFile,
    warnings,
    summary,
    cleanup,
    contentCheck,
  };
}

async function readJson<T>(path: string): Promise<T> {
  const raw = await readFile(path, "utf8");
  return JSON.parse(raw) as T;
}

function resolveBindingValue(
  pageData: unknown,
  binding: PageBinding,
  bindingId: string,
  pageIndex: number,
): unknown {
  const dataPath = binding.dataPath ?? "$";
  try {
    return resolveDataPath(pageData, dataPath, pageData);
  } catch (error) {
    if (binding.required) {
      throw error;
    }
    throw new Error(
      `page ${pageIndex} binding '${bindingId}' has invalid dataPath '${dataPath}': ${
        error instanceof Error ? error.message : String(error)
      }`,
    );
  }
}

function loadImageBindingMedia(
  automizer: Automizer,
  job: RenderJob,
  value: unknown,
  loadedMedia: Set<string>,
  bindingId: string,
  pageIndex: number,
): string {
  const src = imageSource(value);
  if (!src) {
    throw new Error(
      `page ${pageIndex} binding '${bindingId}' expects image data as a path string or object with src`,
    );
  }
  if (src.startsWith("http://") || src.startsWith("https://") || src.startsWith("data:")) {
    throw new Error(
      `page ${pageIndex} binding '${bindingId}' uses unsupported image source '${src}'; use a local content-pack asset`,
    );
  }

  const contentRoot = job.contentRoot ?? dirname(job.contentFile);
  const normalizedSrc = src.replaceAll("\\", "/");
  const mediaFile = isAbsolute(normalizedSrc) ? basename(normalizedSrc) : normalizedSrc;
  const mediaDir = isAbsolute(normalizedSrc)
    ? dirname(normalizedSrc)
    : resolve(contentRoot);
  const absolutePath = isAbsolute(normalizedSrc)
    ? normalizedSrc
    : resolve(contentRoot, normalizedSrc);

  if (!existsSync(absolutePath)) {
    throw new Error(
      `page ${pageIndex} binding '${bindingId}' image file not found: ${absolutePath}`,
    );
  }

  const preparedMedia = prepareMediaForPowerPoint(
    absolutePath,
    mediaFile,
    mediaDir,
    job,
    bindingId,
  );
  const loadKey = `${preparedMedia.mediaDir}\0${preparedMedia.mediaFile}`;
  if (!loadedMedia.has(loadKey)) {
    automizer.loadMedia(preparedMedia.mediaFile, preparedMedia.mediaDir);
    loadedMedia.add(loadKey);
  }
  return preparedMedia.mediaFile;
}

function loadChartImageMedia(
  automizer: Automizer,
  job: RenderJob,
  value: unknown,
  loadedMedia: Set<string>,
  bindingId: string,
  pageIndex: number,
): string {
  if (imageSource(value)) {
    return loadImageBindingMedia(automizer, job, value, loadedMedia, bindingId, pageIndex);
  }

  const chart = normalizeChartImageData(value, pageIndex, bindingId);
  const generatedDir = resolve(dirname(job.outputFile), ".rdeckforge-media");
  mkdirSync(generatedDir, { recursive: true });
  const svgFile = `${safeFilePart(job.jobId)}-${pageIndex}-${safeFilePart(bindingId)}.svg`;
  const absolutePath = resolve(generatedDir, svgFile);
  writeFileSync(absolutePath, renderChartSvg(chart), "utf8");
  const mediaFile = rasterizeSvgForPowerPoint(
    absolutePath,
    generatedDir,
    `${safeFilePart(job.jobId)}-${pageIndex}-${safeFilePart(bindingId)}.png`,
  );

  const loadKey = `${generatedDir}\0${mediaFile}`;
  if (!loadedMedia.has(loadKey)) {
    automizer.loadMedia(mediaFile, generatedDir);
    loadedMedia.add(loadKey);
  }
  return mediaFile;
}

function prepareMediaForPowerPoint(
  absolutePath: string,
  mediaFile: string,
  mediaDir: string,
  job: RenderJob,
  bindingId: string,
): { mediaFile: string; mediaDir: string } {
  if (extname(absolutePath).toLowerCase() !== ".svg") {
    return { mediaFile, mediaDir };
  }
  const generatedDir = resolve(dirname(job.outputFile), ".rdeckforge-media");
  mkdirSync(generatedDir, { recursive: true });
  const hash = createHash("sha1").update(readFileSync(absolutePath)).digest("hex").slice(0, 12);
  return {
    mediaFile: rasterizeSvgForPowerPoint(
      absolutePath,
      generatedDir,
      `${safeFilePart(bindingId)}-${hash}.png`,
    ),
    mediaDir: generatedDir,
  };
}

function rasterizeSvgForPowerPoint(
  svgPath: string,
  outputDir: string,
  outputFile: string,
): string {
  mkdirSync(outputDir, { recursive: true });
  const finalFile = outputFile.endsWith(".png") ? outputFile : `${outputFile}.png`;
  const finalPath = resolve(outputDir, finalFile);
  if (existsSync(finalPath)) {
    return finalFile;
  }

  const scratchDir = resolve(outputDir, `.qlmanage-${safeFilePart(finalFile)}-${Date.now()}`);
  mkdirSync(scratchDir, { recursive: true });
  try {
    execFileSync("/usr/bin/qlmanage", ["-t", "-s", "1600", "-o", scratchDir, svgPath], {
      stdio: "ignore",
    });
    const png = readdirSync(scratchDir).find((name) => name.toLowerCase().endsWith(".png"));
    if (!png) {
      throw new Error("qlmanage did not produce a PNG thumbnail");
    }
    renameSync(resolve(scratchDir, png), finalPath);
    return finalFile;
  } catch (error) {
    throw new Error(
      `failed to rasterize SVG for PowerPoint compatibility: ${svgPath}: ${
        error instanceof Error ? error.message : String(error)
      }`,
    );
  } finally {
    rmSync(scratchDir, { recursive: true, force: true });
  }
}

function imageSource(value: unknown): string | undefined {
  if (typeof value === "string") {
    return value;
  }
  if (value && typeof value === "object" && !Array.isArray(value)) {
    const src = (value as Record<string, unknown>).src;
    return typeof src === "string" ? src : undefined;
  }
  return undefined;
}

type ChartImageSeries = {
  name: string;
  values: number[];
  color?: string;
};

type ChartImageData = {
  title?: string;
  type: "bar" | "line";
  labels: string[];
  series: ChartImageSeries[];
};

function normalizeChartImageData(
  value: unknown,
  pageIndex: number,
  bindingId: string,
): ChartImageData {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(
      `page ${pageIndex} binding '${bindingId}' expects chart image data as an object or image source`,
    );
  }

  const object = value as Record<string, unknown>;
  const labels = normalizeStringArray(object.labels ?? object.categories);
  const type = object.type === "line" ? "line" : "bar";
  const series = normalizeChartSeries(object);
  if (labels.length === 0) {
    throw new Error(`page ${pageIndex} binding '${bindingId}' chart has no labels`);
  }
  if (series.length === 0) {
    throw new Error(`page ${pageIndex} binding '${bindingId}' chart has no series`);
  }
  if (series.some((item) => item.values.length === 0)) {
    throw new Error(`page ${pageIndex} binding '${bindingId}' chart series has no values`);
  }

  return {
    title: typeof object.title === "string" ? object.title : undefined,
    type,
    labels,
    series,
  };
}

function normalizeChartSeries(object: Record<string, unknown>): ChartImageSeries[] {
  if (Array.isArray(object.series)) {
    return object.series
      .map((entry, index) => {
        if (Array.isArray(entry)) {
          return {
            name: `Series ${index + 1}`,
            values: normalizeNumberArray(entry),
          };
        }
        if (entry && typeof entry === "object") {
          const item = entry as Record<string, unknown>;
          return {
            name:
              stringValue(item.name) ??
              stringValue(item.label) ??
              stringValue(item.title) ??
              `Series ${index + 1}`,
            values: normalizeNumberArray(item.values ?? item.data),
            color: stringValue(item.color),
          };
        }
        return { name: `Series ${index + 1}`, values: [] };
      })
      .filter((item) => item.values.length > 0);
  }
  if (Array.isArray(object.values) || Array.isArray(object.data)) {
    return [
      {
        name: stringValue(object.name) ?? "Series 1",
        values: normalizeNumberArray(object.values ?? object.data),
        color: stringValue(object.color),
      },
    ];
  }
  return [];
}

function formatNativeChartData(value: unknown, pageIndex: number, bindingId: string): ChartData {
  const chart = normalizeChartImageData(value, pageIndex, bindingId);
  return {
    series: chart.series.map((series) => ({
      label: series.name,
      style: series.color ? { color: colorValue(series.color) } : undefined,
    })),
    categories: chart.labels.map((label, index) => ({
      label,
      values: chart.series.map((series) => series.values[index] ?? null),
      styles: chart.series.map((series) =>
        series.color ? { color: colorValue(series.color) } : null,
      ),
    })),
  };
}

function buildNativeChartModifications(
  chartData: ChartData,
  value: unknown,
  options?: ChartBindingOptions,
): ShapeModificationCallback[] {
  const modifications = [
    ModifyChartHelper.setChartData(chartData) as unknown as ShapeModificationCallback,
  ];
  const chartTitle =
    typeof options?.title === "string"
      ? options.title
      : options?.title === false
        ? undefined
        : value && typeof value === "object" && !Array.isArray(value)
          ? stringValue((value as Record<string, unknown>).title)
          : undefined;
  if (chartTitle) {
    modifications.push(
      ModifyChartHelper.setChartTitle(chartTitle) as unknown as ShapeModificationCallback,
    );
  }
  if (options?.removeLegend) {
    modifications.push(
      ModifyChartHelper.removeChartLegend() as unknown as ShapeModificationCallback,
    );
  } else if (options?.minimizeLegend) {
    modifications.push(
      ModifyChartHelper.minimizeChartLegend() as unknown as ShapeModificationCallback,
    );
  }
  if (options?.axisRange) {
    modifications.push(
      ModifyChartHelper.setAxisRange({
        min: options.axisRange.min,
        max: options.axisRange.max,
        majorUnit: options.axisRange.majorUnit,
        minorUnit: options.axisRange.minorUnit,
      }) as unknown as ShapeModificationCallback,
    );
  }
  return modifications;
}

function normalizeStringArray(value: unknown): string[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value.map((item) => stringifyValue(item));
}

function normalizeNumberArray(value: unknown): number[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value.map((item) => Number(item)).filter((item) => Number.isFinite(item));
}

function stringValue(value: unknown): string | undefined {
  return typeof value === "string" && value.trim() ? value : undefined;
}

function renderChartSvg(chart: ChartImageData): string {
  const width = 960;
  const height = 540;
  const margin = { top: chart.title ? 72 : 44, right: 44, bottom: 86, left: 76 };
  const plotWidth = width - margin.left - margin.right;
  const plotHeight = height - margin.top - margin.bottom;
  const palette = ["#2563EB", "#0F766E", "#9333EA", "#EA580C", "#475569"];
  const allValues = chart.series.flatMap((item) => item.values);
  const minValue = Math.min(0, ...allValues);
  const maxValue = Math.max(1, ...allValues);
  const range = maxValue - minValue || 1;
  const xFor = (index: number) =>
    margin.left + (chart.labels.length === 1 ? plotWidth / 2 : (index * plotWidth) / (chart.labels.length - 1));
  const yFor = (value: number) =>
    margin.top + plotHeight - ((value - minValue) / range) * plotHeight;

  const grid = [0, 0.25, 0.5, 0.75, 1]
    .map((ratio) => {
      const y = margin.top + plotHeight * ratio;
      const label = maxValue - range * ratio;
      return `<line x1="${margin.left}" y1="${round(y)}" x2="${width - margin.right}" y2="${round(y)}" stroke="#E2E8F0" stroke-width="1"/><text x="${margin.left - 14}" y="${round(y + 5)}" text-anchor="end" fill="#64748B" font-size="18">${escapeXml(formatChartNumber(label))}</text>`;
    })
    .join("");

  const labels = chart.labels
    .map((label, index) => {
      const x = xFor(index);
      return `<text x="${round(x)}" y="${height - 38}" text-anchor="middle" fill="#475569" font-size="18">${escapeXml(label)}</text>`;
    })
    .join("");

  const seriesMarkup = chart.type === "line" ? renderLineSeries(chart, palette, xFor, yFor) : renderBarSeries(chart, palette, margin, plotWidth, yFor);
  const legend = chart.series
    .map((series, index) => {
      const x = margin.left + index * 168;
      const y = height - 18;
      const color = normalizeColor(series.color) ?? palette[index % palette.length];
      return `<rect x="${x}" y="${y - 12}" width="18" height="10" rx="3" fill="${color}"/><text x="${x + 26}" y="${y - 3}" fill="#475569" font-size="15">${escapeXml(series.name)}</text>`;
    })
    .join("");

  return `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}">
  <rect width="${width}" height="${height}" rx="30" fill="#F8FAFC"/>
  <rect x="18" y="18" width="${width - 36}" height="${height - 36}" rx="26" fill="#FFFFFF" stroke="#D7DEE8"/>
  ${chart.title ? `<text x="${margin.left}" y="48" fill="#172033" font-size="27" font-weight="700">${escapeXml(chart.title)}</text>` : ""}
  ${grid}
  <line x1="${margin.left}" y1="${margin.top + plotHeight}" x2="${width - margin.right}" y2="${margin.top + plotHeight}" stroke="#CBD5E1" stroke-width="2"/>
  <line x1="${margin.left}" y1="${margin.top}" x2="${margin.left}" y2="${margin.top + plotHeight}" stroke="#CBD5E1" stroke-width="2"/>
  ${seriesMarkup}
  ${labels}
  ${legend}
</svg>`;
}

function renderLineSeries(
  chart: ChartImageData,
  palette: string[],
  xFor: (index: number) => number,
  yFor: (value: number) => number,
): string {
  return chart.series
    .map((series, seriesIndex) => {
      const color = normalizeColor(series.color) ?? palette[seriesIndex % palette.length];
      const points = series.values
        .map((value, index) => `${round(xFor(index))},${round(yFor(value))}`)
        .join(" ");
      const dots = series.values
        .map((value, index) => `<circle cx="${round(xFor(index))}" cy="${round(yFor(value))}" r="6" fill="${color}" stroke="#FFFFFF" stroke-width="3"/>`)
        .join("");
      return `<polyline points="${points}" fill="none" stroke="${color}" stroke-width="5" stroke-linecap="round" stroke-linejoin="round"/>${dots}`;
    })
    .join("");
}

function renderBarSeries(
  chart: ChartImageData,
  palette: string[],
  margin: { top: number; right: number; bottom: number; left: number },
  plotWidth: number,
  yFor: (value: number) => number,
): string {
  const groupWidth = plotWidth / Math.max(chart.labels.length, 1);
  const barWidth = Math.max(12, Math.min(46, (groupWidth * 0.62) / chart.series.length));
  const baseline = yFor(0);
  return chart.series
    .map((series, seriesIndex) => {
      const color = normalizeColor(series.color) ?? palette[seriesIndex % palette.length];
      return series.values
        .map((value, index) => {
          const groupStart = margin.left + index * groupWidth + groupWidth * 0.19;
          const x = groupStart + seriesIndex * barWidth;
          const y = Math.min(yFor(value), baseline);
          const h = Math.max(2, Math.abs(baseline - yFor(value)));
          return `<rect x="${round(x)}" y="${round(y)}" width="${round(barWidth - 3)}" height="${round(h)}" rx="7" fill="${color}"/>`;
        })
        .join("");
    })
    .join("");
}

function normalizeColor(value?: string): string | undefined {
  if (!value) {
    return undefined;
  }
  const color = value.startsWith("#") ? value : `#${value}`;
  return /^#[0-9A-Fa-f]{6}$/.test(color) ? color : undefined;
}

function colorValue(value: string) {
  return { value: value.replace(/^#/, "").toUpperCase() };
}

function formatChartNumber(value: number): string {
  return Number.isInteger(value) ? String(value) : value.toFixed(1);
}

function round(value: number): number {
  return Math.round(value * 10) / 10;
}

function escapeXml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&apos;");
}

function safeFilePart(value: string): string {
  return value.replace(/[^a-zA-Z0-9_-]+/g, "-").replace(/^-+|-+$/g, "") || "chart";
}

function replaceImageRelation(mediaName: string): ShapeModificationCallback {
  const replaceTarget = ModifyImageHelper.setRelationTarget(mediaName) as ShapeModificationCallback;
  return (element, relation, info) => {
    replaceTarget(element, relation, info);
    const relationId = relation?.getAttribute("Id");
    if (!relationId) {
      return;
    }
    for (const svgBlip of [
      ...Array.from(element.getElementsByTagName("asvg:svgBlip")),
      ...Array.from(element.getElementsByTagName("a:svgBlip")),
    ]) {
      svgBlip.setAttribute("r:embed", relationId);
    }
  };
}

function resolveDataPath(root: unknown, expression: string, fallbackRoot: unknown): unknown {
  const trimmed = expression.trim();
  if (isQuotedLiteral(trimmed)) {
    return trimmed.slice(1, -1);
  }
  if (trimmed === "$") {
    return root;
  }
  if (!trimmed.startsWith("$.")) {
    return fallbackRoot;
  }

  let value = root;
  for (const segment of parsePathSegments(trimmed)) {
    if (value === undefined || value === null) {
      return undefined;
    }
    if (typeof segment === "number") {
      if (!Array.isArray(value)) {
        return undefined;
      }
      value = value[segment];
      continue;
    }
    if (typeof value !== "object") {
      return undefined;
    }
    value = (value as Record<string, unknown>)[segment];
  }
  return value;
}

function parsePathSegments(expression: string): Array<string | number> {
  const path = expression.slice(2);
  const segments: Array<string | number> = [];
  let cursor = 0;

  while (cursor < path.length) {
    if (path[cursor] === ".") {
      cursor += 1;
    }
    if (path[cursor] === "[") {
      const close = path.indexOf("]", cursor);
      if (close === -1) {
        throw new Error(`missing closing bracket in dataPath '${expression}'`);
      }
      const index = Number(path.slice(cursor + 1, close));
      if (!Number.isInteger(index)) {
        throw new Error(`array index is not an integer in dataPath '${expression}'`);
      }
      segments.push(index);
      cursor = close + 1;
      continue;
    }

    let next = cursor;
    while (next < path.length && path[next] !== "." && path[next] !== "[") {
      next += 1;
    }
    const key = path.slice(cursor, next);
    if (!key) {
      throw new Error(`empty segment in dataPath '${expression}'`);
    }
    segments.push(key);
    cursor = next;
  }

  return segments;
}

function isQuotedLiteral(value: string): boolean {
  return (
    value.length >= 2 &&
    ((value.startsWith("'") && value.endsWith("'")) ||
      (value.startsWith('"') && value.endsWith('"')))
  );
}

function formatBindingValue(
  value: unknown,
  binding: PageBinding,
  warnings: string[],
  pageIndex: number,
  bindingId: string,
): string {
  const bindingType = binding.type ?? "text";
  if (bindingType === "list") {
    if (!Array.isArray(value)) {
      return stringifyValue(value);
    }
    if (binding.maxItems !== undefined && value.length > binding.maxItems) {
      warnings.push(
        `page ${pageIndex} binding '${bindingId}' has ${value.length} items, maxItems is ${binding.maxItems}`,
      );
    }
    return value.map((item, index) => `${index + 1}. ${formatListItem(item)}`).join("\n");
  }
  if (bindingType !== "text") {
    warnings.push(
      `page ${pageIndex} binding '${bindingId}' uses unsupported type '${bindingType}', rendered as text`,
    );
  }

  const text = stringifyValue(value);
  if (binding.maxLength !== undefined && text.length > binding.maxLength) {
    warnings.push(
      `page ${pageIndex} binding '${bindingId}' length ${text.length} exceeds maxLength ${binding.maxLength}`,
    );
  }
  return text;
}

function formatTableData(
  value: unknown,
  options: TableBindingOptions | undefined,
  warnings: string[],
  pageIndex: number,
  bindingId: string,
): TableData {
  const rows = normalizeTableRows(value);
  if (rows.length === 0) {
    warnings.push(`page ${pageIndex} binding '${bindingId}' table has no rows`);
    return { body: [{ values: [""] }] };
  }
  const maxColumns = Math.max(...rows.map((row) => row.values.length));
  if (maxColumns === 0) {
    warnings.push(`page ${pageIndex} binding '${bindingId}' table has no columns`);
    return { body: [{ values: [""] }] };
  }
  return {
    body: applyTableStyles(
      rows.map((row) => ({
        ...row,
        values: padTableValues(row.values, maxColumns),
      })),
      options,
    ),
  };
}

function buildTableModifications(
  tableData: TableData,
  options?: TableBindingOptions,
): ShapeModificationCallback[] {
  const params: ModifyTableParams = {
    adjustHeight: options?.adjustHeight ?? true,
    adjustWidth: options?.adjustWidth ?? true,
    expand: options?.expand,
  };
  const modifications: ShapeModificationCallback[] = [
    ModifyTableHelper.setTable(tableData, params) as unknown as ShapeModificationCallback,
  ];
  if (options?.styleId) {
    modifications.push(
      ModifyTableHelper.setTableStyle(
        options.styleId,
        options.styleAttribs ?? [],
      ) as unknown as ShapeModificationCallback,
    );
  }
  for (const [index, width] of (options?.columnWidths ?? []).entries()) {
    if (Number.isFinite(width) && width > 0) {
      modifications.push(
        ModifyTableHelper.updateColumnWidth(index, width) as unknown as ShapeModificationCallback,
      );
    }
  }
  for (const [index, height] of (options?.rowHeights ?? []).entries()) {
    if (Number.isFinite(height) && height > 0) {
      modifications.push(
        ModifyTableHelper.updateRowHeight(index, height) as unknown as ShapeModificationCallback,
      );
    }
  }
  return modifications;
}

function applyTableStyles(rows: TableRow[], options?: TableBindingOptions): TableRow[] {
  if (!options?.headerStyle && !options?.bodyStyle) {
    return rows;
  }
  const headerStyle = tableStyleFromSpec(options.headerStyle);
  const bodyStyle = tableStyleFromSpec(options.bodyStyle);
  return rows.map((row, index) => {
    const style = index === 0 ? headerStyle : bodyStyle;
    if (!style) {
      return row;
    }
    return {
      ...row,
      styles: row.values.map(() => style),
    };
  });
}

function tableStyleFromSpec(spec?: TableCellStyleSpec): TableRowStyle | undefined {
  if (!spec) {
    return undefined;
  }
  return {
    size: spec.size,
    color: spec.color ? colorValue(spec.color) : undefined,
    background: spec.background ? colorValue(spec.background) : undefined,
    isBold: spec.bold,
    isItalics: spec.italic,
    isUnderlined: spec.underline,
    border: spec.borderColor
      ? (["lnL", "lnR", "lnT", "lnB"] as const).map((tag) => ({
          tag,
          color: colorValue(spec.borderColor as string),
          weight: spec.borderWeight,
        }))
      : undefined,
  };
}

function normalizeTableRows(value: unknown): TableRow[] {
  if (Array.isArray(value)) {
    if (value.every((row) => Array.isArray(row))) {
      return value.map((row, index) => ({
        label: index === 0 ? "header" : undefined,
        values: normalizeTableValues(row),
      }));
    }
    if (value.every((row) => row && typeof row === "object" && !Array.isArray(row))) {
      const columns = collectObjectColumns(value as Array<Record<string, unknown>>);
      return [
        { label: "header", values: columns },
        ...(value as Array<Record<string, unknown>>).map((row) => ({
          values: columns.map((column) => stringifyValue(row[column])),
        })),
      ];
    }
  }

  if (value && typeof value === "object") {
    const object = value as Record<string, unknown>;
    if (Array.isArray(object.columns) && Array.isArray(object.rows)) {
      const columns = object.columns.map((column) => stringifyValue(column));
      return [
        { label: "header", values: columns },
        ...object.rows.map((row) => normalizeTableRow(row, columns)),
      ];
    }
    if (Array.isArray(object.header) || Array.isArray(object.body)) {
      const headerRows = Array.isArray(object.header)
        ? normalizeHeaderRows(object.header)
        : [];
      const bodyRows = Array.isArray(object.body)
        ? object.body.map((row) => normalizeTableRow(row, firstHeaderValues(headerRows)))
        : [];
      return [...headerRows, ...bodyRows];
    }
  }

  return [];
}

function normalizeHeaderRows(value: unknown[]): TableRow[] {
  if (value.length > 0 && value.every((row) => Array.isArray(row))) {
    return value.map((row) => ({ label: "header", values: normalizeTableValues(row) }));
  }
  return [{ label: "header", values: normalizeTableValues(value) }];
}

function normalizeTableRow(value: unknown, columns: string[] = []): TableRow {
  if (Array.isArray(value)) {
    return { values: normalizeTableValues(value) };
  }
  if (value && typeof value === "object") {
    const object = value as Record<string, unknown>;
    const values = columns.length
      ? columns.map((column) => stringifyValue(object[column]))
      : Object.values(object).map((item) => stringifyValue(item));
    return { values };
  }
  return { values: [stringifyValue(value)] };
}

function normalizeTableValues(values: unknown[]): Array<string | number> {
  return values.map((value) => {
    if (typeof value === "number") {
      return value;
    }
    return stringifyValue(value);
  });
}

function collectObjectColumns(rows: Array<Record<string, unknown>>): string[] {
  const columns = new Set<string>();
  for (const row of rows) {
    for (const key of Object.keys(row)) {
      columns.add(key);
    }
  }
  return [...columns];
}

function firstHeaderValues(rows: TableRow[]): string[] {
  const first = rows[0]?.values ?? [];
  return first.map((value) => stringifyValue(value));
}

function padTableValues(values: Array<string | number>, length: number): Array<string | number> {
  if (values.length >= length) {
    return values;
  }
  return [...values, ...Array.from({ length: length - values.length }, () => "")];
}

function formatListItem(value: unknown): string {
  if (value === null || value === undefined) {
    return "";
  }
  if (typeof value === "object" && !Array.isArray(value)) {
    const item = value as Record<string, unknown>;
    if (item.heading || item.body) {
      return [item.heading, item.body].filter(Boolean).map(stringifyValue).join(": ");
    }
    if (item.label || item.title) {
      return [item.label, item.title].filter(Boolean).map(stringifyValue).join(" ");
    }
  }
  return stringifyValue(value);
}

function stringifyValue(value: unknown): string {
  if (value === null || value === undefined) {
    return "";
  }
  if (typeof value === "string") {
    return value;
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  return JSON.stringify(value);
}

function buildExpectedBindings(
  job: RenderJob,
  manifest: TemplateManifest,
  content: unknown,
  warnings: string[],
): ExpectedBinding[] {
  const expected: ExpectedBinding[] = [];
  for (const page of job.plannedPages) {
    const pageTemplate = manifest.pageTemplates?.[page.pageTemplate];
    if (!pageTemplate) {
      continue;
    }
    const pageData =
      page.dataValue === undefined ? resolveDataPath(content, page.dataPath, content) : page.dataValue;
    for (const [bindingId, binding] of Object.entries(pageTemplate.bindings ?? {})) {
      const bindingType = binding.type ?? "text";
      if (
        bindingType === "image" ||
        bindingType === "backgroundImage" ||
        bindingType === "table" ||
        bindingType === "chart"
      ) {
        continue;
      }
      const value = resolveBindingValue(pageData, binding, bindingId, page.pageIndex);
      if (value === undefined || value === null) {
        continue;
      }
      expected.push({
        pageIndex: page.pageIndex,
        pageTemplate: page.pageTemplate,
        bindingId,
        expectedText: formatBindingValue(value, binding, warnings, page.pageIndex, bindingId),
      });
    }
  }
  return expected;
}

async function checkRenderedContent(
  outputFile: string,
  expectedBindings: ExpectedBinding[],
): Promise<ContentCheckSummary> {
  const buffer = await readFile(outputFile);
  const zip = await JSZip.loadAsync(buffer);
  const slidePaths = await getPresentationSlidePaths(zip);
  const slideTexts = new Map<number, string>();
  for (const [index, slidePath] of slidePaths.entries()) {
    const slideXml = await zip.file(slidePath)?.async("string");
    slideTexts.set(index + 1, normalizeText(extractTextRuns(slideXml ?? "").join("\n")));
  }

  const missing: ContentCheckSummary["missing"] = [];
  for (const binding of expectedBindings) {
    const slideText = slideTexts.get(binding.pageIndex) ?? "";
    const expectedLines = binding.expectedText
      .split("\n")
      .map(normalizeText)
      .filter(Boolean);
    const hasAllLines = expectedLines.every((line) => slideText.includes(line));
    if (!hasAllLines) {
      missing.push(binding);
    }
  }

  return {
    status: missing.length === 0 ? "passed" : "failed",
    checkedPages: slidePaths.length,
    checkedBindings: expectedBindings.length,
    missing,
  };
}

async function getPresentationSlidePaths(zip: JSZip): Promise<string[]> {
  const presentationXml = await zip.file("ppt/presentation.xml")?.async("string");
  const relsXml = await zip.file("ppt/_rels/presentation.xml.rels")?.async("string");
  if (!presentationXml || !relsXml) {
    return [];
  }

  const relationships = new Map<string, string>();
  for (const match of relsXml.matchAll(/<Relationship\b[^>]*\/>/g)) {
    const relationship = match[0];
    const id = readXmlAttr(relationship, "Id");
    const type = readXmlAttr(relationship, "Type");
    const target = readXmlAttr(relationship, "Target");
    if (id && target && type?.endsWith("/slide")) {
      relationships.set(id, resolveRelationshipTarget("ppt/presentation.xml", target) ?? "");
    }
  }

  return [...presentationXml.matchAll(/<p:sldId\b[^>]*r:id="([^"]+)"/g)]
    .map((match) => relationships.get(match[1]))
    .filter((path): path is string => Boolean(path));
}

function extractTextRuns(xml: string): string[] {
  return [...xml.matchAll(/<a:t[^>]*>([\s\S]*?)<\/a:t>/g)].map((match) =>
    decodeXmlText(match[1]),
  );
}

function decodeXmlText(value: string): string {
  return value
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, '"')
    .replace(/&apos;/g, "'")
    .replace(/&amp;/g, "&");
}

function normalizeText(value: string): string {
  return value.replace(/\s+/g, " ").trim();
}

async function cleanupPptxPackage(outputFile: string): Promise<PptxCleanupSummary> {
  const buffer = await readFile(outputFile);
  const zip = await JSZip.loadAsync(buffer);
  const presentationPath = "ppt/presentation.xml";
  const relsPath = "ppt/_rels/presentation.xml.rels";
  const contentTypesPath = "[Content_Types].xml";
  const presentationXml = await zip.file(presentationPath)?.async("string");
  const relsXml = await zip.file(relsPath)?.async("string");
  if (!presentationXml || !relsXml) {
    return {
      visibleSlideCount: 0,
      slidePartCount: countSlideParts(zip),
      removedSlideRelationships: 0,
      removedSlideParts: 0,
      removedSlideRelationshipParts: 0,
      removedNotesSlideParts: 0,
      removedNotesSlideRelationshipParts: 0,
      renamedSlideParts: 0,
      renamedNotesSlideParts: 0,
      removedMediaRelationships: 0,
      removedPresentationLayoutRelationships: 0,
      removedContentTypeOverrides: 0,
      removedDuplicateContentTypeOverrides: 0,
      updatedAppProperties: 0,
      svgFallbackRelationships: 0,
      removedSvgBlipExtensions: 0,
      removedChartRelationships: 0,
      removedChartParts: 0,
      removedChartRelationshipParts: 0,
      removedEmbeddedWorkbookParts: 0,
    };
  }

  const usedSlideRelIds = new Set<string>();
  for (const match of presentationXml.matchAll(/<p:sldId\b[^>]*r:id="([^"]+)"/g)) {
    usedSlideRelIds.add(match[1]);
  }

  const removedTargets: string[] = [];
  let removedPresentationLayoutRelationships = 0;
  const cleanedRelsXml = relsXml.replace(/<Relationship\b[^>]*\/>/g, (relationship) => {
    const id = readXmlAttr(relationship, "Id");
    const type = readXmlAttr(relationship, "Type");
    const target = readXmlAttr(relationship, "Target");
    if (type?.endsWith("/slideLayout")) {
      removedPresentationLayoutRelationships += 1;
      return "";
    }
    if (type?.endsWith("/slide") && id && target && !usedSlideRelIds.has(id)) {
      removedTargets.push(`ppt/${target}`);
      return "";
    }
    return relationship;
  });
  zip.file(relsPath, cleanedRelsXml);

  let removedSlideParts = 0;
  for (const target of removedTargets) {
    if (zip.file(target)) {
      zip.remove(target);
      removedSlideParts += 1;
    }
  }

  let removedNotesSlideParts = 0;
  let removedNotesSlideRelationshipParts = 0;
  for (const relPart of Object.keys(zip.files).filter((path) =>
    /^ppt\/notesSlides\/_rels\/notesSlide\d+\.xml\.rels$/.test(path),
  )) {
    const notesRelXml = await zip.file(relPart)?.async("string");
    if (!notesRelXml) {
      continue;
    }
    const sourcePart = sourcePartForRelationshipPart(relPart);
    const slideTargets = [...notesRelXml.matchAll(/<Relationship\b[^>]*\/>/g)]
      .map((match) => match[0])
      .filter((relationship) => readXmlAttr(relationship, "Type")?.endsWith("/slide"))
      .map((relationship) =>
        resolveRelationshipTarget(sourcePart, readXmlAttr(relationship, "Target")),
      )
      .filter((target): target is string => Boolean(target));

    if (slideTargets.some((target) => !zip.file(target))) {
      if (zip.file(sourcePart)) {
        zip.remove(sourcePart);
        removedNotesSlideParts += 1;
      }
      zip.remove(relPart);
      removedNotesSlideRelationshipParts += 1;
    }
  }

  let removedSlideRelationshipParts = 0;
  removedNotesSlideRelationshipParts += removeOrphanRelationshipParts(
    zip,
    /^ppt\/notesSlides\/_rels\/notesSlide\d+\.xml\.rels$/,
  );
  removedSlideRelationshipParts += removeOrphanRelationshipParts(
    zip,
    /^ppt\/slides\/_rels\/slide\d+\.xml\.rels$/,
  );
  const removedMediaRelationships = await removeInvalidSlideMediaRelationships(zip);
  const removedSvgBlipExtensions = await cleanupNonSvgBlipExtensions(zip);

  const renameSummary = await normalizeSlidePartNames(zip, presentationPath, relsPath);
  const chartCleanup = await cleanupChartParts(zip);

  let removedContentTypeOverrides = 0;
  let removedDuplicateContentTypeOverrides = 0;
  const contentTypesXml = await zip.file(contentTypesPath)?.async("string");
  if (contentTypesXml) {
    const seenContentTypeOverrides = new Set<string>();
    const cleanedContentTypesXml = contentTypesXml.replace(/<Override\b[^>]*\/>/g, (override) => {
      const partName = readXmlAttr(override, "PartName");
      if (partName && !zip.file(partName.replace(/^\//, ""))) {
        removedContentTypeOverrides += 1;
        return "";
      }
      return override;
    });
    const dedupedContentTypesXml = cleanedContentTypesXml.replace(
      /<Override\b[^>]*\/>/g,
      (override) => {
        const partName = readXmlAttr(override, "PartName");
        if (!partName) {
          return override;
        }
        if (seenContentTypeOverrides.has(partName)) {
          removedDuplicateContentTypeOverrides += 1;
          return "";
        }
        seenContentTypeOverrides.add(partName);
        return override;
      },
    );
    zip.file(contentTypesPath, dedupedContentTypesXml);
  }

  const updatedAppProperties = await updateAppProperties(zip, usedSlideRelIds.size);

  const cleanedBuffer = await zip.generateAsync({ type: "nodebuffer" });
  await writeFile(outputFile, cleanedBuffer);

  return {
    visibleSlideCount: usedSlideRelIds.size,
    slidePartCount: countSlideParts(zip),
    removedSlideRelationships: removedTargets.length,
    removedSlideParts,
    removedSlideRelationshipParts,
    removedNotesSlideParts,
    removedNotesSlideRelationshipParts,
    renamedSlideParts: renameSummary.renamedSlideParts,
    renamedNotesSlideParts: renameSummary.renamedNotesSlideParts,
    removedMediaRelationships,
    removedPresentationLayoutRelationships,
    removedContentTypeOverrides,
    removedDuplicateContentTypeOverrides,
    updatedAppProperties,
    svgFallbackRelationships: 0,
    removedSvgBlipExtensions,
    removedChartRelationships: chartCleanup.removedChartRelationships,
    removedChartParts: chartCleanup.removedChartParts,
    removedChartRelationshipParts: chartCleanup.removedChartRelationshipParts,
    removedEmbeddedWorkbookParts: chartCleanup.removedEmbeddedWorkbookParts,
  };
}

async function cleanupNonSvgBlipExtensions(zip: JSZip): Promise<number> {
  let removed = 0;
  for (const slidePath of Object.keys(zip.files).filter((path) =>
    /^ppt\/slides\/slide\d+\.xml$/.test(path),
  )) {
    const relsPath = relationshipPartForSourcePart(slidePath);
    const slideXml = await zip.file(slidePath)?.async("string");
    const relsXml = await zip.file(relsPath)?.async("string");
    if (!slideXml || !relsXml || !slideXml.includes("svgBlip")) {
      continue;
    }

    const targetsById = new Map(
      readRelationships(relsXml).map((relationship) => [relationship.id, relationship.target]),
    );
    let nextXml = slideXml.replace(
      /<a:ext\b[^>]*>\s*<a?svg:svgBlip\b[^>]*r:embed="([^"]+)"[^>]*\/>\s*<\/a:ext>/g,
      (match, relationId: string) => {
        const target = targetsById.get(relationId);
        if (target?.toLowerCase().endsWith(".svg")) {
          return match;
        }
        removed += 1;
        return "";
      },
    );
    nextXml = nextXml.replace(/<a:extLst>\s*<\/a:extLst>/g, "");
    if (nextXml !== slideXml) {
      zip.file(slidePath, nextXml);
    }
  }
  return removed;
}

async function cleanupChartParts(zip: JSZip): Promise<{
  removedChartRelationships: number;
  removedChartParts: number;
  removedChartRelationshipParts: number;
  removedEmbeddedWorkbookParts: number;
}> {
  let removedChartRelationships = 0;
  const usedChartParts = new Set<string>();

  for (const relPart of Object.keys(zip.files).filter((path) =>
    /^ppt\/slides\/_rels\/slide\d+\.xml\.rels$/.test(path),
  )) {
    const sourcePart = sourcePartForRelationshipPart(relPart);
    const slideXml = await zip.file(sourcePart)?.async("string");
    const relsXml = await zip.file(relPart)?.async("string");
    if (!slideXml || !relsXml) {
      continue;
    }
    const usedRelIds = new Set(
      [...slideXml.matchAll(/\br:(?:embed|link|id)="([^"]+)"/g)].map((match) => match[1]),
    );
    const cleaned = relsXml.replace(/<Relationship\b[^>]*\/>/g, (relationship) => {
      const id = readXmlAttr(relationship, "Id");
      const type = readXmlAttr(relationship, "Type");
      const target = readXmlAttr(relationship, "Target");
      if (!type?.endsWith("/chart")) {
        return relationship;
      }
      const targetPath = resolveRelationshipTarget(sourcePart, target);
      if (!id || !usedRelIds.has(id) || !targetPath || !zip.file(targetPath)) {
        removedChartRelationships += 1;
        return "";
      }
      usedChartParts.add(targetPath);
      return relationship;
    });
    if (cleaned !== relsXml) {
      zip.file(relPart, cleaned);
    }
  }

  let removedChartParts = 0;
  for (const chartPart of Object.keys(zip.files).filter((path) =>
    /^ppt\/charts\/chart\d+\.xml$/.test(path),
  )) {
    if (!usedChartParts.has(chartPart)) {
      zip.remove(chartPart);
      removedChartParts += 1;
    }
  }

  let removedChartRelationshipParts = removeOrphanRelationshipParts(
    zip,
    /^ppt\/charts\/_rels\/chart\d+\.xml\.rels$/,
  );

  const usedEmbeddings = new Set<string>();
  for (const relPart of Object.keys(zip.files).filter((path) =>
    /^ppt\/charts\/_rels\/chart\d+\.xml\.rels$/.test(path),
  )) {
    const sourcePart = sourcePartForRelationshipPart(relPart);
    const relsXml = await zip.file(relPart)?.async("string");
    if (!relsXml) {
      continue;
    }
    const cleaned = relsXml.replace(/<Relationship\b[^>]*\/>/g, (relationship) => {
      const type = readXmlAttr(relationship, "Type");
      const target = readXmlAttr(relationship, "Target");
      if (!type?.endsWith("/package")) {
        return relationship;
      }
      const targetPath = resolveRelationshipTarget(sourcePart, target);
      if (!targetPath || !zip.file(targetPath)) {
        return "";
      }
      usedEmbeddings.add(targetPath);
      return relationship;
    });
    if (cleaned !== relsXml) {
      zip.file(relPart, cleaned);
    }
  }

  let removedEmbeddedWorkbookParts = 0;
  for (const embeddingPart of Object.keys(zip.files).filter((path) =>
    /^ppt\/embeddings\/.+\.xlsx$/.test(path),
  )) {
    if (!usedEmbeddings.has(embeddingPart)) {
      zip.remove(embeddingPart);
      removedEmbeddedWorkbookParts += 1;
    }
  }
  removedChartRelationshipParts += removeOrphanRelationshipParts(
    zip,
    /^ppt\/charts\/_rels\/chart\d+\.xml\.rels$/,
  );

  return {
    removedChartRelationships,
    removedChartParts,
    removedChartRelationshipParts,
    removedEmbeddedWorkbookParts,
  };
}

function readRelationships(xml: string): Array<{ id: string; target: string; type: string }> {
  return [...xml.matchAll(/<Relationship\b[^>]*\/>/g)]
    .map((match) => {
      const relationship = match[0];
      return {
        id: readXmlAttr(relationship, "Id") ?? "",
        target: readXmlAttr(relationship, "Target") ?? "",
        type: readXmlAttr(relationship, "Type") ?? "",
      };
    })
    .filter((relationship) => relationship.id && relationship.target);
}

async function updateAppProperties(zip: JSZip, visibleSlideCount: number): Promise<number> {
  const appPath = "docProps/app.xml";
  const appXml = await zip.file(appPath)?.async("string");
  if (!appXml) {
    return 0;
  }
  const previousSlideTitleCount =
    readSlideTitleHeadingPair(appXml) ?? readSimpleXmlElement(appXml, "Slides") ?? 0;
  const notesCount = Object.keys(zip.files).filter((path) =>
    /^ppt\/notesSlides\/notesSlide\d+\.xml$/.test(path),
  ).length;
  let nextXml = replaceSimpleXmlElement(appXml, "Slides", String(visibleSlideCount));
  nextXml = replaceSimpleXmlElement(nextXml, "Notes", String(notesCount));
  nextXml = updateSlideTitleHeadingPair(nextXml, visibleSlideCount);
  nextXml = updateTitlesOfParts(nextXml, visibleSlideCount, previousSlideTitleCount);
  if (nextXml === appXml) {
    return 0;
  }
  zip.file(appPath, nextXml);
  return 1;
}

function replaceSimpleXmlElement(xml: string, tagName: string, value: string): string {
  return xml.replace(
    new RegExp(`<${tagName}>[^<]*</${tagName}>`),
    `<${tagName}>${value}</${tagName}>`,
  );
}

function readSimpleXmlElement(xml: string, tagName: string): number | undefined {
  const value = new RegExp(`<${tagName}>(\\d+)</${tagName}>`).exec(xml)?.[1];
  return value ? Number(value) : undefined;
}

function readSlideTitleHeadingPair(xml: string): number | undefined {
  const value =
    /<vt:variant>\s*<vt:lpstr>Slide Titles<\/vt:lpstr>\s*<\/vt:variant>\s*<vt:variant>\s*<vt:i4>(\d+)<\/vt:i4>\s*<\/vt:variant>/.exec(
      xml,
    )?.[1];
  return value ? Number(value) : undefined;
}

function updateSlideTitleHeadingPair(xml: string, visibleSlideCount: number): string {
  return xml.replace(
    /(<vt:variant>\s*<vt:lpstr>Slide Titles<\/vt:lpstr>\s*<\/vt:variant>\s*<vt:variant>\s*<vt:i4>)\d+(<\/vt:i4>\s*<\/vt:variant>)/,
    `$1${visibleSlideCount}$2`,
  );
}

function updateTitlesOfParts(
  xml: string,
  visibleSlideCount: number,
  previousSlideTitleCount: number,
): string {
  return xml.replace(
    /<TitlesOfParts>\s*<vt:vector size="(\d+)" baseType="lpstr">([\s\S]*?)<\/vt:vector>\s*<\/TitlesOfParts>/,
    (match, _size, body: string) => {
      const entries = [...body.matchAll(/<vt:lpstr>([\s\S]*?)<\/vt:lpstr>/g)].map(
        (entry) => entry[1],
      );
      if (entries.length === 0) {
        return match;
      }
      const prefixLength = Math.max(0, entries.length - previousSlideTitleCount);
      const prefix = entries.slice(0, prefixLength);
      const slideEntries = Array.from(
        { length: visibleSlideCount },
        (_, index) => `<vt:lpstr>Slide ${index + 1}</vt:lpstr>`,
      );
      const prefixEntries = prefix.map((entry) => `<vt:lpstr>${entry}</vt:lpstr>`);
      return `<TitlesOfParts><vt:vector size="${prefixEntries.length + slideEntries.length}" baseType="lpstr">\n\t\t\t${prefixEntries.join("\n\t\t\t")}\n\t\t\t${slideEntries.join("")}\n\t\t</vt:vector></TitlesOfParts>`;
    },
  );
}

function readXmlAttr(xml: string, attr: string): string | undefined {
  return new RegExp(`${attr}="([^"]+)"`).exec(xml)?.[1];
}

function countSlideParts(zip: JSZip): number {
  return Object.keys(zip.files).filter((path) => /^ppt\/slides\/slide\d+\.xml$/.test(path)).length;
}

async function removeInvalidSlideMediaRelationships(zip: JSZip): Promise<number> {
  let removed = 0;
  for (const relPart of Object.keys(zip.files).filter((path) =>
    /^ppt\/slides\/_rels\/slide\d+\.xml\.rels$/.test(path),
  )) {
    const sourcePart = sourcePartForRelationshipPart(relPart);
    const slideXml = await zip.file(sourcePart)?.async("string");
    const relsXml = await zip.file(relPart)?.async("string");
    if (!slideXml || !relsXml) {
      continue;
    }
    const usedRelIds = new Set(
      [...slideXml.matchAll(/r:(?:embed|link)="([^"]+)"/g)].map((match) => match[1]),
    );
    const cleaned = relsXml.replace(/<Relationship\b[^>]*\/>/g, (relationship) => {
      const id = readXmlAttr(relationship, "Id");
      const type = readXmlAttr(relationship, "Type");
      const target = readXmlAttr(relationship, "Target");
      if (!type?.endsWith("/image")) {
        return relationship;
      }
      const targetPath = resolveRelationshipTarget(sourcePart, target);
      if (!id || !usedRelIds.has(id) || !targetPath || !zip.file(targetPath)) {
        removed += 1;
        return "";
      }
      return relationship;
    });
    if (cleaned !== relsXml) {
      zip.file(relPart, cleaned);
    }
  }
  return removed;
}

async function normalizeSlidePartNames(
  zip: JSZip,
  presentationPath: string,
  relsPath: string,
): Promise<{ renamedSlideParts: number; renamedNotesSlideParts: number }> {
  const presentationXml = await zip.file(presentationPath)?.async("string");
  const relsXml = await zip.file(relsPath)?.async("string");
  if (!presentationXml || !relsXml) {
    return { renamedSlideParts: 0, renamedNotesSlideParts: 0 };
  }

  const slideRelIds = [...presentationXml.matchAll(/<p:sldId\b[^>]*r:id="([^"]+)"/g)].map(
    (match) => match[1],
  );
  const slideRelationships = new Map<string, string>();
  for (const match of relsXml.matchAll(/<Relationship\b[^>]*\/>/g)) {
    const relationship = match[0];
    const id = readXmlAttr(relationship, "Id");
    const type = readXmlAttr(relationship, "Type");
    const target = readXmlAttr(relationship, "Target");
    if (id && target && type?.endsWith("/slide")) {
      slideRelationships.set(id, resolveRelationshipTarget("ppt/presentation.xml", target) ?? "");
    }
  }

  const partMap = new Map<string, string>();
  let renamedSlideParts = 0;
  let renamedNotesSlideParts = 0;

  for (const [index, relId] of slideRelIds.entries()) {
    const oldSlidePart = slideRelationships.get(relId);
    if (!oldSlidePart || !zip.file(oldSlidePart)) {
      continue;
    }
    const slideNumber = index + 1;
    const newSlidePart = `ppt/slides/slide${slideNumber}.xml`;
    const oldSlideRelPart = relationshipPartForSourcePart(oldSlidePart);
    const newSlideRelPart = relationshipPartForSourcePart(newSlidePart);

    if (oldSlidePart !== newSlidePart) {
      partMap.set(oldSlidePart, newSlidePart);
      renamedSlideParts += 1;
    }
    if (zip.file(oldSlideRelPart) && oldSlideRelPart !== newSlideRelPart) {
      partMap.set(oldSlideRelPart, newSlideRelPart);
    }

    const slideRelsXml = await zip.file(oldSlideRelPart)?.async("string");
    if (slideRelsXml) {
      for (const match of slideRelsXml.matchAll(/<Relationship\b[^>]*\/>/g)) {
        const relationship = match[0];
        const type = readXmlAttr(relationship, "Type");
        const target = readXmlAttr(relationship, "Target");
        if (!type?.endsWith("/notesSlide")) {
          continue;
        }
        const oldNotesPart = resolveRelationshipTarget(oldSlidePart, target);
        if (!oldNotesPart || !zip.file(oldNotesPart)) {
          continue;
        }
        const newNotesPart = `ppt/notesSlides/notesSlide${slideNumber}.xml`;
        const oldNotesRelPart = relationshipPartForSourcePart(oldNotesPart);
        const newNotesRelPart = relationshipPartForSourcePart(newNotesPart);
        if (oldNotesPart !== newNotesPart) {
          partMap.set(oldNotesPart, newNotesPart);
          renamedNotesSlideParts += 1;
        }
        if (zip.file(oldNotesRelPart) && oldNotesRelPart !== newNotesRelPart) {
          partMap.set(oldNotesRelPart, newNotesRelPart);
        }
      }
    }
  }

  if (partMap.size === 0) {
    return { renamedSlideParts: 0, renamedNotesSlideParts: 0 };
  }

  await updateRelationshipTargets(zip, partMap);
  await updateContentTypePartNames(zip, partMap);
  await renameZipParts(zip, partMap);

  return { renamedSlideParts, renamedNotesSlideParts };
}

async function updateRelationshipTargets(zip: JSZip, partMap: Map<string, string>) {
  for (const relPart of Object.keys(zip.files).filter((path) => path.endsWith(".rels"))) {
    const relXml = await zip.file(relPart)?.async("string");
    if (!relXml) {
      continue;
    }
    const sourcePart = relPart === "_rels/.rels" ? "" : sourcePartForRelationshipPart(relPart);
    const nextXml = relXml.replace(/<Relationship\b[^>]*\/>/g, (relationship) => {
      if (readXmlAttr(relationship, "TargetMode") === "External") {
        return relationship;
      }
      const target = readXmlAttr(relationship, "Target");
      const resolvedTarget = resolveRelationshipTarget(sourcePart, target);
      if (!resolvedTarget) {
        return relationship;
      }
      const renamedTarget = partMap.get(resolvedTarget);
      if (!renamedTarget) {
        return relationship;
      }
      return relationship.replace(
        /Target="[^"]*"/,
        `Target="${relationshipTargetFrom(sourcePart, renamedTarget)}"`,
      );
    });
    if (nextXml !== relXml) {
      zip.file(relPart, nextXml);
    }
  }
}

async function updateContentTypePartNames(zip: JSZip, partMap: Map<string, string>) {
  const contentTypesPath = "[Content_Types].xml";
  const contentTypesXml = await zip.file(contentTypesPath)?.async("string");
  if (!contentTypesXml) {
    return;
  }
  const nextXml = contentTypesXml.replace(/<Override\b[^>]*\/>/g, (override) => {
    const partName = readXmlAttr(override, "PartName");
    if (!partName) {
      return override;
    }
    const renamedPart = partMap.get(partName.replace(/^\//, ""));
    if (!renamedPart) {
      return override;
    }
    return override.replace(/PartName="[^"]*"/, `PartName="/${renamedPart}"`);
  });
  if (nextXml !== contentTypesXml) {
    zip.file(contentTypesPath, nextXml);
  }
}

async function renameZipParts(zip: JSZip, partMap: Map<string, string>) {
  const moves = [...partMap.entries()].filter(([from, to]) => from !== to && zip.file(from));
  const payloads: Array<{ from: string; to: string; data: Buffer }> = [];
  for (const [from, to] of moves) {
    const data = await zip.file(from)?.async("nodebuffer");
    if (data) {
      payloads.push({ from, to, data });
    }
  }
  for (const { to } of payloads) {
    if (zip.file(to)) {
      zip.remove(to);
    }
  }
  for (const { from } of payloads) {
    zip.remove(from);
  }
  for (const { to, data } of payloads) {
    zip.file(to, data);
  }
}

function removeOrphanRelationshipParts(zip: JSZip, pattern = /\/_rels\/.+\.rels$/): number {
  let removed = 0;
  for (const relPart of Object.keys(zip.files).filter((path) => pattern.test(path))) {
    const sourcePart = sourcePartForRelationshipPart(relPart);
    if (!zip.file(sourcePart)) {
      zip.remove(relPart);
      removed += 1;
    }
  }
  return removed;
}

function sourcePartForRelationshipPart(relPart: string): string {
  return relPart.replace(/\/_rels\/([^/]+)\.rels$/, "/$1");
}

function relationshipPartForSourcePart(sourcePart: string): string {
  return `${pathPosix.dirname(sourcePart)}/_rels/${pathPosix.basename(sourcePart)}.rels`;
}

function resolveRelationshipTarget(sourcePart: string, target?: string): string | undefined {
  if (!target || /^[a-z]+:/i.test(target)) {
    return undefined;
  }
  if (target.startsWith("/")) {
    return target.slice(1);
  }
  return pathPosix.normalize(pathPosix.join(pathPosix.dirname(sourcePart), target));
}

function relationshipTargetFrom(sourcePart: string, targetPart: string): string {
  if (!sourcePart) {
    return targetPart;
  }
  return pathPosix.relative(pathPosix.dirname(sourcePart), targetPart);
}
