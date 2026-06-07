import type { ChatRole } from "../../lib/types";
import { chatRoleLabel } from "../../lib/utils";
import type { MessageRole } from "./Message";

export function chatRoleToMessageFrom(role: ChatRole): MessageRole {
  switch (role) {
    case "System":
      return "system";
    case "Thinking":
      return "thinking";
    case "User":
      return "user";
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
