import type { ChatRole } from "../../lib/types";
import { chatRoleLabel } from "../../lib/utils";
import type { MessageRole } from "./Message";

export function chatRoleToMessageFrom(role: ChatRole): MessageRole {
  switch (role) {
    case "system":
    case "System":
      return "system";
    case "thinking":
    case "Thinking":
      return "thinking";
    case "user":
    case "User":
      return "user";
    case "assistant":
    case "Assistant":
      return "assistant";
  }
}

export function messageLabel(
  role: ChatRole,
  nodeLabel: string | null | undefined,
): string {
  return chatRoleLabel(role, nodeLabel);
}
