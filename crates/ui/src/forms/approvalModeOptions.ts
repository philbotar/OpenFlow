import type { ApprovalMode } from "../lib/types";

export const APPROVAL_MODE_OPTIONS: { value: ApprovalMode; label: string }[] = [
  { value: "read_only", label: "Read only" },
  { value: "write", label: "Read auto-approve, write prompt" },
  { value: "always_ask", label: "Always ask" },
  { value: "yolo", label: "Auto-approve all" },
];
