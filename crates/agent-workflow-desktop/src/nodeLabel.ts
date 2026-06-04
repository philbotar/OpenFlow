export function resolveCommittedNodeLabel(currentLabel: string, draftLabel: string) {
  const trimmed = draftLabel.trim();
  return trimmed === "" ? currentLabel : trimmed;
}
