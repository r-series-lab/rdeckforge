export type OfficeRenderResult = {
  sidecar?: {
    data?: {
      outputFile?: string;
      plannedPageCount?: number;
    };
  };
  outputFile?: string;
  outputIntegrity?: {
    status: "passed";
    format: string;
    fileSize: number;
    entryCount: number;
    relationshipCount: number;
    requiredEntries: string[];
  };
  format?: string;
  renderResult?: OfficeRenderResult;
  plannedPageCount?: number | null;
  sheetCount?: number | null;
};

export function renderedOutputFile(result: OfficeRenderResult | null | undefined): string | null {
  const visited = new Set<OfficeRenderResult>();
  let current = result;

  while (current && !visited.has(current)) {
    visited.add(current);
    if (current.outputFile) {
      return current.outputFile;
    }
    if (current.sidecar?.data?.outputFile) {
      return current.sidecar.data.outputFile;
    }
    current = current.renderResult;
  }

  return null;
}
