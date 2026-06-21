export const WORKFLOWS_SECTION_STORAGE_KEY = "openflow.workflowsSectionHidden";
export const PROJECTS_SECTION_STORAGE_KEY = "openflow.projectsSectionHidden";

type StorageLike = Pick<Storage, "getItem" | "setItem"> | null | undefined;

function readSectionHidden(storage: StorageLike, key: string): boolean {
  const rawValue = storage?.getItem(key);
  if (rawValue === null || rawValue === undefined) {
    return false;
  }

  if (rawValue === "true") {
    return true;
  }

  return false;
}

function writeSectionHidden(storage: StorageLike, key: string, hidden: boolean): void {
  storage?.setItem(key, String(hidden));
}

export function readWorkflowsSectionHidden(storage: StorageLike): boolean {
  return readSectionHidden(storage, WORKFLOWS_SECTION_STORAGE_KEY);
}

export function writeWorkflowsSectionHidden(storage: StorageLike, hidden: boolean): void {
  writeSectionHidden(storage, WORKFLOWS_SECTION_STORAGE_KEY, hidden);
}

export function readProjectsSectionHidden(storage: StorageLike): boolean {
  return readSectionHidden(storage, PROJECTS_SECTION_STORAGE_KEY);
}

export function writeProjectsSectionHidden(storage: StorageLike, hidden: boolean): void {
  writeSectionHidden(storage, PROJECTS_SECTION_STORAGE_KEY, hidden);
}
