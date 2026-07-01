export const PANEL_VISIBILITY_STORAGE_KEY = "openflow.rightPanelHidden";
export const LEFT_PANEL_VISIBILITY_STORAGE_KEY = "openflow.leftPanelHidden";
export const WORKFLOWS_SECTION_STORAGE_KEY = "openflow.workflowsSectionHidden";
export const PROJECTS_SECTION_STORAGE_KEY = "openflow.projectsSectionHidden";

type StorageLike = Pick<Storage, "getItem" | "setItem"> | null | undefined;

export function readStoredBoolean(storage: StorageLike, key: string): boolean {
  const rawValue = storage?.getItem(key);
  return rawValue === "true";
}

export function writeStoredBoolean(
  storage: StorageLike,
  key: string,
  value: boolean,
): void {
  storage?.setItem(key, String(value));
}
