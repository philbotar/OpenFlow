import { ConversationComposer } from "./ConversationComposer";
import { ConversationMessages } from "./ConversationMessages";
import { FileChangesPanel } from "./FileChangesPanel";
import { ToolApprovalCard } from "./ToolApprovalCard";

export function ChatPanel() {
  return (
    <div class="chat-layout">
      <ConversationMessages />
      <div class="chat-side-panels">
        <ToolApprovalCard />
        <FileChangesPanel />
      </div>
      <ConversationComposer />
    </div>
  );
}
