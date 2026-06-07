// @vitest-environment jsdom
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

vi.mock("lucide-solid/icons/arrow-up", () => ({
  default: () => null,
}));

import { DockPanel } from "./DockPanel";
import { AppContext } from "../context/AppContext";

describe("DockPanel chat tab reactivity", () => {
  beforeEach(() => {
    document.body.innerHTML = "";
  });

  afterEach(() => {
    vi.clearAllMocks();
    document.body.innerHTML = "";
  });

  test("switches to the chat branch when the tab signal changes", async () => {
    const [bottomTab, setBottomTab] = createSignal<"overview" | "chat" | "trace">(
      "overview",
    );
    const [dockOpen, setDockOpen] = createSignal(false);

    const ctx = {
      bottomTab,
      dockOpen,
      handleSelectBottomTab: (tab: "overview" | "chat" | "trace") => {
        setBottomTab(tab);
        setDockOpen(true);
      },
      handleDockResizePointerDown: () => {},
      hasRunTraceMemo: () => false,
      runState: () => null,
      selectedTraceIndex: () => null,
      setSelectedTraceIndex: () => {},
      selectedTrace: () => null,
      chatMessages: () => [],
      currentNode: () => ({ label: "Node 1" }),
      selectedPendingApproval: () => null,
      chatComposerBusyMemo: () => false,
      chatEnabledMemo: () => true,
      chatInput: () => "",
      setChatInput: () => {},
      handleChatInputKeyDown: () => {},
      chatSubmission: () => ({ submittedText: "", invokedSkills: [] }),
      canSendChatMemo: () => false,
      handleSubmitChat: async () => {},
      handleToolApproval: async () => {},
      setChatHistoryRef: () => {},
      currentNodeOutput: () => null,
      activeProfileMemo: () => ({ default_model: "" }),
      chatComposerBusy: () => false,
      setWorkflowNameInputRef: () => {},
      workflows: () => [],
      agents: () => [],
      activeWorkflowId: () => null,
      selectedNodeId: () => null,
      selectedEdgeId: () => null,
      screen: () => "editor",
      settings: () => ({ active_provider: "openai" }),
      readiness: () => ({ ready: true }),
      schemaText: () => "",
      newModelInputByProvider: () => ({}),
      providerKeyInputByProvider: () => ({}),
      uiZoom: () => 1,
      editingWorkflowId: () => null,
      workflowNameDraft: () => "",
      selectedAgentId: () => null,
      editingAgentId: () => null,
      agentNameDraft: () => "",
      editingNodeId: () => null,
      nodeLabelDraft: () => "",
      agentSchemaDraft: () => "",
      addNodePickerOpen: () => false,
      isMaximized: () => false,
      setWorkflowNameDraft: () => {},
      setNewModelInputByProvider: () => {},
      setProviderKeyInputByProvider: () => {},
      setNodeLabelDraft: () => {},
      setSchemaText: () => {},
      setSelectedAgentId: () => {},
      setScreen: () => {},
      selectedAgent: () => null,
      canvasGraph: () => null,
      canvasStatusByNode: () => null,
      canvasSubagentsByNode: () => null,
      providerIdsMemo: () => [],
      activeProviderKeyInput: () => "",
      selectedTraceIndex2: () => null,
      hasRunTraceMemo2: () => false,
      handleSwitchWorkflow: () => {},
      handleCreateWorkflow: async () => {},
      handleOpenAgents: () => {},
      handleCreateAgent: async () => {},
      handleSaveAgents: async () => {},
      handleAgentSchemaInput: () => {},
      updateSelectedAgent: () => {},
      handleSaveSettings: async () => {},
      handleAddKnownModel: () => {},
      handleRemoveKnownModel: () => {},
      handleApiKeyInput: () => {},
      updateSettings: async () => {},
      handleSelectNode: () => {},
      handleSelectEdge: () => {},
      handleCanvasNodePosition: () => {},
      handleCreateEdge: () => {},
      handleReconnectEdge: () => {},
      handleDeleteEdge: () => {},
      handleDeleteSelectedNode: () => {},
      handleOpenAddNodePicker: () => {},
      handleAddNode: async () => {},
      closeAddNodePicker: () => {},
      handleValidate: async () => {},
      handleRun: async () => {},
      handleClearRunTrace: async () => {},
      handleStartNodeLabelEdit: () => {},
      handleCancelNodeLabelEdit: () => {},
      handleCommitNodeLabel: () => {},
      handleStartWorkflowNameEdit: () => {},
      handleCancelWorkflowNameEdit: () => {},
      handleWorkflowNameCommit: () => {},
      handleWorkflowNameKeyDown: () => {},
      updateCurrentNode: () => {},
      updateCurrentNodeToolConfig: () => {},
      setToolEnabled: () => {},
      applySchemaEditor: () => true,
      persistAll: async () => true,
      handleZoomIn: () => {},
      handleZoomOut: () => {},
      handleZoomReset: () => {},
    } as any;

    const container = document.createElement("div");
    document.body.append(container);
    render(() => (
      <AppContext.Provider value={ctx}>
        <DockPanel />
      </AppContext.Provider>
    ), container);

    expect(container.querySelector(".chat-composer")).toBeNull();

    const chatButton = Array.from(container.querySelectorAll(".dock-tab-switcher button")).find(
      (button) => button.textContent === "Chat",
    ) as HTMLButtonElement;
    chatButton.click();
    await Promise.resolve();

    expect(bottomTab()).toBe("chat");
    expect(dockOpen()).toBe(true);
    expect(container.querySelector(".chat-composer")).not.toBeNull();
    expect(chatButton.classList.contains("active")).toBe(true);
  });
});
