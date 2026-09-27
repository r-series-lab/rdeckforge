export function inspectTemplateDraft(inputFile: string) {
  return {
    inputFile,
    layouts: {},
    warnings: ["Shape extraction is not implemented in the scaffold."],
  };
}
