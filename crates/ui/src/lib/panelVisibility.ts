export const PANEL_VISIBILITY_STORAGE_KEY = "openflow.rightPanelHidden";

type StorageLike = Pick<Storage, "getItem" | "setItem"> | null | undefined;

export function readStoredRightPanelHidden(storage: StorageLike): boolean {
  const rawValue = storage?.getItem(PANEL_VISIBILITY_STORAGE_KEY);
  if (rawValue === null || rawValue === undefined) {
    return false;
  }

  if (rawValue === "true") {
    return true;
  }

  return false;
}

export function writeStoredRightPanelHidden(
  storage: StorageLike,
  hidden: boolean,
): void {
  storage?.setItem(PANEL_VISIBILITY_STORAGE_KEY, String(hidden));
}
