export const PANEL_VISIBILITY_STORAGE_KEY = "openflow.rightPanelHidden";
export const LEFT_PANEL_VISIBILITY_STORAGE_KEY = "openflow.leftPanelHidden";

type StorageLike = Pick<Storage, "getItem" | "setItem"> | null | undefined;

function readStoredPanelHidden(storage: StorageLike, key: string): boolean {
  const rawValue = storage?.getItem(key);
  if (rawValue === null || rawValue === undefined) {
    return false;
  }

  if (rawValue === "true") {
    return true;
  }

  return false;
}

function writeStoredPanelHidden(
  storage: StorageLike,
  key: string,
  hidden: boolean,
): void {
  storage?.setItem(key, String(hidden));
}

export function readStoredRightPanelHidden(storage: StorageLike): boolean {
  return readStoredPanelHidden(storage, PANEL_VISIBILITY_STORAGE_KEY);
}

export function writeStoredRightPanelHidden(
  storage: StorageLike,
  hidden: boolean,
): void {
  writeStoredPanelHidden(storage, PANEL_VISIBILITY_STORAGE_KEY, hidden);
}

export function readStoredLeftPanelHidden(storage: StorageLike): boolean {
  return readStoredPanelHidden(storage, LEFT_PANEL_VISIBILITY_STORAGE_KEY);
}

export function writeStoredLeftPanelHidden(
  storage: StorageLike,
  hidden: boolean,
): void {
  writeStoredPanelHidden(storage, LEFT_PANEL_VISIBILITY_STORAGE_KEY, hidden);
}
