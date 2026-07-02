import type { NodeToolConfig } from "../types";

export function createEmptyToolConfig(): NodeToolConfig {
  return {
    approvalMode: "write",
  };
}
