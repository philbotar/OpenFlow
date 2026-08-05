import type { ApprovalMode } from "../lib/types";

export const APPROVAL_MODE_OPTIONS: { value: ApprovalMode; label: string }[] = [
  { value: "read_only", label: "Read only" },
  { value: "write", label: "Read auto-approve, write prompt" },
  { value: "always_ask", label: "Always ask" },
  { value: "yolo", label: "Auto-approve all" },
];

export const CHAT_APPROVAL_MODE_STORAGE_KEY = "openflow.lastChatApprovalMode";

type StorageLike = Pick<Storage, "getItem" | "setItem"> | null | undefined;

export function readStoredApprovalMode(
  storage: StorageLike,
): ApprovalMode | null {
  const rawValue = storage?.getItem(CHAT_APPROVAL_MODE_STORAGE_KEY);
  return (
    APPROVAL_MODE_OPTIONS.find((option) => option.value === rawValue)?.value ??
    null
  );
}

export function writeStoredApprovalMode(
  storage: StorageLike,
  value: ApprovalMode,
): void {
  storage?.setItem(CHAT_APPROVAL_MODE_STORAGE_KEY, value);
}
