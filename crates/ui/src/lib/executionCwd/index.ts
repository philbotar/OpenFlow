export function executionCwdForRun(value: string): string | null {
  const trimmed = value.trim();
  return trimmed === "" ? null : trimmed;
}
