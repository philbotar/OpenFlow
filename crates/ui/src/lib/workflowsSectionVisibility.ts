export const WORKFLOWS_SECTION_STORAGE_KEY = "openflow.workflowsSectionHidden";

type StorageLike = Pick<Storage, "getItem" | "setItem"> | null | undefined;

export function readWorkflowsSectionHidden(storage: StorageLike): boolean {
  const rawValue = storage?.getItem(WORKFLOWS_SECTION_STORAGE_KEY);
  if (rawValue === null || rawValue === undefined) {
    return false;
  }

  if (rawValue === "true") {
    return true;
  }

  return false;
}

export function writeWorkflowsSectionHidden(
  storage: StorageLike,
  hidden: boolean,
): void {
  storage?.setItem(WORKFLOWS_SECTION_STORAGE_KEY, String(hidden));
}
