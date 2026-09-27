export type DiagnosticStatus = "pass" | "warn" | "fail" | "info";

export type TemplateDependencyCheck = {
  id: string;
  label: string;
  kind: string;
  status: DiagnosticStatus;
  required: boolean;
  detected: boolean;
  path?: string | null;
  version?: string | null;
  message: string;
  fixHint?: string | null;
};

export type TemplateReadinessReport = {
  templateId: string;
  templateName: string;
  templateDir: string;
  format: string;
  templateType: string;
  checkedAt: string;
  readiness: DiagnosticStatus;
  canRender: boolean;
  summary: string;
  components: TemplateDependencyCheck[];
  warnings: string[];
};

export type TemplateValidationSummary = {
  templateId: string;
  name: string;
  testCaseCount: number;
  testCaseIds: string[];
};

export type TemplateAssertionResult = {
  id: string;
  passed: boolean;
  message: string;
};

export type TemplateTestCaseResult = {
  id: string;
  inputFile: string;
  outputFile: string;
  recipe?: string | null;
  durationMs: number;
  passed: boolean;
  error?: string | null;
  assertions: TemplateAssertionResult[];
};

export type TemplateTestReport = {
  templateId: string;
  templateName: string;
  templateDir: string;
  format: string;
  outputDir: string;
  selectedCount: number;
  passedCount: number;
  failedCount: number;
  passed: boolean;
  readiness: DiagnosticStatus;
  canRender: boolean;
  cases: TemplateTestCaseResult[];
};

export function templateAcceptanceIssues(report: TemplateReadinessReport) {
  return report.components.filter(
    (component) => component.status === "fail" || component.status === "warn",
  );
}

export function templateAcceptanceStatus(
  report: TemplateReadinessReport | null,
  tests: TemplateTestReport | null,
) {
  if (tests) {
    return tests.passed
      ? { label: "自测通过", className: "is-ok" }
      : { label: "自测失败", className: "is-danger" };
  }
  if (!report) {
    return { label: "未检查", className: "" };
  }
  if (!report.canRender || report.readiness === "fail") {
    return { label: "不可生成", className: "is-danger" };
  }
  if (report.readiness === "warn") {
    return { label: "可以生成，有提示", className: "is-warn" };
  }
  return { label: "可以生成", className: "is-ok" };
}
