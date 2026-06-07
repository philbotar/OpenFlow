import { ConversationComposer } from "./ConversationComposer";
import { ConversationMessages } from "./ConversationMessages";
import { ToolApprovalCard } from "./ToolApprovalCard";

export function ChatPanel() {
  return (
    <div class="chat-layout">
      <ConversationMessages />
      <ToolApprovalCard />
      <ConversationComposer />
    </div>
  );
}
