export const PANEL_VISIBILITY_STORAGE_KEY = "openflow.rightPanelHidden";
export const LEFT_PANEL_VISIBILITY_STORAGE_KEY = "openflow.leftPanelHidden";
export const WORKFLOWS_SECTION_STORAGE_KEY = "openflow.workflowsSectionHidden";
export const PROJECTS_SECTION_STORAGE_KEY = "openflow.projectsSectionHidden";

type StorageLike = Pick<Storage, "getItem" | "setItem"> | null | undefined;

export function readStoredBoolean(storage: StorageLike, key: string): boolean {
  const rawValue = storage?.getItem(key);
  if (rawValue === null || rawValue === undefined) {
    return false;
  }

  if (rawValue === "true") {
    return true;
  }

  return false;
}

export function writeStoredBoolean(
  storage: StorageLike,
  key: string,
  value: boolean,
): void {
  storage?.setItem(key, String(value));
}

export function readStoredRightPanelHidden(storage: StorageLike): boolean {
  return readStoredBoolean(storage, PANEL_VISIBILITY_STORAGE_KEY);
}

export function writeStoredRightPanelHidden(
  storage: StorageLike,
  hidden: boolean,
): void {
  writeStoredBoolean(storage, PANEL_VISIBILITY_STORAGE_KEY, hidden);
}

export function readStoredLeftPanelHidden(storage: StorageLike): boolean {
  return readStoredBoolean(storage, LEFT_PANEL_VISIBILITY_STORAGE_KEY);
}

export function writeStoredLeftPanelHidden(
  storage: StorageLike,
  hidden: boolean,
): void {
  writeStoredBoolean(storage, LEFT_PANEL_VISIBILITY_STORAGE_KEY, hidden);
}

export function readWorkflowsSectionHidden(storage: StorageLike): boolean {
  return readStoredBoolean(storage, WORKFLOWS_SECTION_STORAGE_KEY);
}

export function writeWorkflowsSectionHidden(
  storage: StorageLike,
  hidden: boolean,
): void {
  writeStoredBoolean(storage, WORKFLOWS_SECTION_STORAGE_KEY, hidden);
}

export function readProjectsSectionHidden(storage: StorageLike): boolean {
  return readStoredBoolean(storage, PROJECTS_SECTION_STORAGE_KEY);
}

export function writeProjectsSectionHidden(
  storage: StorageLike,
  hidden: boolean,
): void {
  writeStoredBoolean(storage, PROJECTS_SECTION_STORAGE_KEY, hidden);
}
