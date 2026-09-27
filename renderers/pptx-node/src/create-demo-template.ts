import { mkdir } from "node:fs/promises";
import { dirname, resolve } from "node:path";

type PptxGenLike = {
  layout: string;
  author: string;
  company: string;
  subject: string;
  title: string;
  lang: string;
  theme: Record<string, string>;
  ShapeType: {
    rect: string;
    roundRect: string;
  };
  ChartType: {
    line: string;
    bar: string;
  };
  addSlide: () => PptxSlide;
  writeFile: (props: { fileName: string }) => Promise<string>;
};

type PptxSlide = {
  background: { color: string };
  addText: (text: string, options?: Record<string, unknown>) => PptxSlide;
  addShape: (shape: string, options?: Record<string, unknown>) => PptxSlide;
  addImage: (options?: Record<string, unknown>) => PptxSlide;
  addTable: (rows: unknown[], options?: Record<string, unknown>) => PptxSlide;
  addChart: (type: string, data: unknown[], options?: Record<string, unknown>) => PptxSlide;
};

const { default: PptxGenJSModule } = await import("pptxgenjs");
const PptxGenJS = PptxGenJSModule as unknown as new () => PptxGenLike;

const outputFile = resolve(
  "../../examples/templates/demo-medical-teaching-v1/template.pptx",
);

const pptx = new PptxGenJS();
pptx.layout = "LAYOUT_WIDE";
pptx.author = "rDeckForge";
pptx.company = "r-series";
pptx.subject = "Generated neutral demo template";
pptx.title = "Demo Medical Teaching Template";
pptx.lang = "zh-CN";
pptx.theme = {
  headFontFace: "Aptos Display",
  bodyFontFace: "Aptos",
  lang: "zh-CN",
};

const colors = {
  ink: "172033",
  muted: "64748B",
  line: "D7DEE8",
  accent: "2563EB",
  accentSoft: "DBEAFE",
  canvas: "F8FAFC",
  white: "FFFFFF",
};
const placeholderImageData =
  "data:image/svg+xml;base64," +
  Buffer.from(
    `<svg xmlns="http://www.w3.org/2000/svg" width="960" height="640" viewBox="0 0 960 640">
      <rect width="960" height="640" rx="44" fill="#DBEAFE"/>
      <rect x="48" y="48" width="864" height="544" rx="32" fill="#FFFFFF" opacity="0.72"/>
      <path d="M250 420h460L585 285l-92 102-70-74-173 107Z" fill="#2563EB" opacity="0.78"/>
      <circle cx="358" cy="214" r="58" fill="#2563EB" opacity="0.45"/>
      <text x="480" y="530" text-anchor="middle" font-family="Arial" font-size="38" fill="#1D4ED8">image placeholder</text>
    </svg>`,
  ).toString("base64");

addCover();
addGoals();
addSectionDivider();
addChapterContent();
addAssessmentTable();
addScoreChart();
addNativeScoreChart();
addSummary();
addEvaluation();
addClosing();

await mkdir(dirname(outputFile), { recursive: true });
await pptx.writeFile({ fileName: outputFile });
console.log(JSON.stringify({ ok: true, outputFile, slideCount: 10 }));

function addCover() {
  const slide = pptx.addSlide();
  addBackground(slide);
  addKicker(slide, "DEMO TEMPLATE");
  slide.addText("Course title", {
    x: 0.72,
    y: 1.55,
    w: 8.8,
    h: 1.3,
    objectName: "ph:title",
    color: colors.ink,
    fontFace: "Aptos Display",
    fontSize: 34,
    bold: true,
    breakLine: false,
  });
  slide.addText("Speaker", {
    x: 0.78,
    y: 4.75,
    w: 4.2,
    h: 0.34,
    objectName: "ph:speaker",
    color: colors.muted,
    fontSize: 14,
  });
  slide.addText("Date", {
    x: 0.78,
    y: 5.22,
    w: 4.2,
    h: 0.34,
    objectName: "ph:date",
    color: colors.muted,
    fontSize: 14,
  });
  slide.addImage({
    data: placeholderImageData,
    x: 8.35,
    y: 1.34,
    w: 3.78,
    h: 2.64,
    objectName: "ph:coverImage",
  });
}

function addGoals() {
  const slide = pptx.addSlide();
  addBackground(slide);
  addTitle(slide, "Teaching goals", "ph:title");
  addPanel(slide, 0.78, 1.85, 11.75, 4.2);
  slide.addText("1. Goal item\n2. Goal item\n3. Goal item", {
    x: 1.22,
    y: 2.28,
    w: 10.85,
    h: 3.2,
    objectName: "ph:goals",
    color: colors.ink,
    fontSize: 21,
    breakLine: false,
    fit: "shrink",
    valign: "mid",
  });
}

function addSectionDivider() {
  const slide = pptx.addSlide();
  addBackground(slide);
  slide.addText("Part label", {
    x: 0.9,
    y: 2.1,
    w: 3.8,
    h: 0.42,
    objectName: "ph:sectionLabel",
    color: colors.accent,
    fontSize: 18,
    bold: true,
  });
  slide.addText("Section title", {
    x: 0.88,
    y: 2.72,
    w: 10.4,
    h: 1.0,
    objectName: "ph:title",
    color: colors.ink,
    fontSize: 34,
    bold: true,
  });
}

function addChapterContent() {
  const slide = pptx.addSlide();
  addBackground(slide);
  addTitle(slide, "Chapter content", "ph:title");
  addPanel(slide, 0.78, 1.6, 11.75, 4.7);
  slide.addText("1. Key point\n2. Key point\n3. Key point", {
    x: 1.18,
    y: 2.02,
    w: 10.95,
    h: 3.78,
    objectName: "ph:items",
    color: colors.ink,
    fontSize: 18,
    fit: "shrink",
    breakLine: false,
  });
}

function addAssessmentTable() {
  const slide = pptx.addSlide();
  addBackground(slide);
  addTitle(slide, "Assessment table", "ph:title");
  slide.addTable(
    [
      ["项目", "观察要点", "教学提示"],
      ["意识状态", "清醒度与配合度", "用案例引导观察"],
      ["生命体征", "趋势变化", "强调记录频率"],
      ["风险因素", "跌倒与管路", "结合评分表"],
    ],
    {
      x: 0.98,
      y: 1.78,
      w: 11.28,
      h: 4.45,
      objectName: "ph:assessmentTable",
      border: { color: colors.line, pt: 1 },
      fill: { color: "FFFFFF" },
    },
  );
}

function addScoreChart() {
  const slide = pptx.addSlide();
  addBackground(slide);
  addTitle(slide, "Score trend", "ph:title");
  slide.addImage({
    data: placeholderImageData,
    x: 1.05,
    y: 1.67,
    w: 11.05,
    h: 4.75,
    objectName: "ph:scoreChart",
  });
}

function addNativeScoreChart() {
  const slide = pptx.addSlide();
  addBackground(slide);
  addTitle(slide, "Editable chart", "ph:title");
  slide.addChart(
    pptx.ChartType.line,
    [
      {
        name: "平均分",
        labels: ["课前", "讲授后", "案例后", "复盘后"],
        values: [60, 70, 80, 90],
      },
      {
        name: "达标率",
        labels: ["课前", "讲授后", "案例后", "复盘后"],
        values: [50, 62, 74, 86],
      },
    ],
    {
      x: 1.08,
      y: 1.72,
      w: 10.98,
      h: 4.62,
      objectName: "ph:nativeScoreChart",
      showTitle: true,
      title: "Native chart placeholder",
      showLegend: true,
      legendPos: "b",
      showValue: false,
      catAxisLabelFontFace: "Aptos",
      valAxisLabelFontFace: "Aptos",
      valAxisMinVal: 0,
      valAxisMaxVal: 100,
      valAxisMajorUnit: 20,
      chartColors: [colors.accent, "0F766E"],
    },
  );
}

function addSummary() {
  const slide = pptx.addSlide();
  addBackground(slide);
  addTitle(slide, "Summary", "ph:title");
  addPanel(slide, 0.78, 1.75, 11.75, 4.55);
  slide.addText("1. Summary item\n2. Summary item", {
    x: 1.18,
    y: 2.22,
    w: 10.95,
    h: 3.5,
    objectName: "ph:summary",
    color: colors.ink,
    fontSize: 20,
    fit: "shrink",
    breakLine: false,
  });
}

function addEvaluation() {
  const slide = pptx.addSlide();
  addBackground(slide);
  addTitle(slide, "Evaluation", "ph:title");
  addPanel(slide, 0.78, 1.75, 11.75, 4.55);
  slide.addText("1. Question\n2. Practice\n3. Feedback", {
    x: 1.18,
    y: 2.22,
    w: 10.95,
    h: 3.5,
    objectName: "ph:items",
    color: colors.ink,
    fontSize: 20,
    fit: "shrink",
    breakLine: false,
  });
}

function addClosing() {
  const slide = pptx.addSlide();
  addBackground(slide);
  slide.addText("Thank you", {
    x: 0.88,
    y: 2.62,
    w: 10.8,
    h: 0.9,
    objectName: "ph:title",
    color: colors.ink,
    fontSize: 38,
    bold: true,
    align: "center",
  });
}

function addBackground(slide: PptxSlide) {
  slide.background = { color: colors.canvas };
  slide.addShape(pptx.ShapeType.rect, {
    x: 0.28,
    y: 0.26,
    w: 12.78,
    h: 6.95,
    fill: { color: colors.white, transparency: 8 },
    line: { color: colors.line, transparency: 15, width: 0.75 },
    radius: 0.16,
    objectName: "chrome:canvas",
  });
  slide.addShape(pptx.ShapeType.rect, {
    x: 0.54,
    y: 0.52,
    w: 0.1,
    h: 6.4,
    fill: { color: colors.accent },
    line: { transparency: 100 },
    objectName: "chrome:accent",
  });
}

function addKicker(slide: PptxSlide, text: string) {
  slide.addText(text, {
    x: 0.78,
    y: 0.86,
    w: 4.5,
    h: 0.3,
    color: colors.accent,
    fontSize: 10,
    bold: true,
    charSpace: 1.3,
    objectName: "chrome:kicker",
  });
}

function addTitle(slide: PptxSlide, text: string, objectName: string) {
  addKicker(slide, "DEMO TEMPLATE");
  slide.addText(text, {
    x: 0.78,
    y: 0.88,
    w: 10.85,
    h: 0.56,
    objectName,
    color: colors.ink,
    fontSize: 25,
    bold: true,
  });
}

function addPanel(slide: PptxSlide, x: number, y: number, w: number, h: number) {
  slide.addShape(pptx.ShapeType.roundRect, {
    x,
    y,
    w,
    h,
    rectRadius: 0.08,
    fill: { color: colors.accentSoft, transparency: 18 },
    line: { color: colors.line, transparency: 15 },
    objectName: "chrome:panel",
  });
}
