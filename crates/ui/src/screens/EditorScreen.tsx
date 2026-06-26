import { Show } from "solid-js";
import { useAppContext } from "../context/AppContext";
import { NodePickerModal } from "@/components";
import { InspectorPanel } from "../panels/InspectorPanel";
import { WorkflowSettingsPanel } from "../panels/WorkflowSettingsPanel";
import { DockPanel } from "../panels/DockPanel";
import WorkflowCanvasHost from "../canvas/WorkflowCanvasHost";
import { COLLAPSED_DOCK_HEIGHT } from "@/lib/utils";

export function EditorScreen() {
  const ctx = useAppContext();
  const showInspectorPanel = () =>
    ctx.inspectorOpen() &&
    !ctx.workflowSettingsOpen() &&
    Boolean(ctx.selectedNodeId());
  const showRightPanel = () =>
    !ctx.rightPanelHidden() && (ctx.workflowSettingsOpen() || showInspectorPanel());
  const chatFocusActive = () => ctx.chatFocusMode() && ctx.dockOpen();

  return (
    <div
      class="editor-screen"
      classList={{
        "editor-screen--chat-focus": chatFocusActive(),
        "editor-screen--no-right-panel": !showRightPanel(),
      }}
      style={{
        "--dock-height": `${ctx.dockOpen() ? ctx.dockHeight() : COLLAPSED_DOCK_HEIGHT}px`,
      }}
    >
      <NodePickerModal />

      <div class="editor-main">
        <section class="canvas-panel">
          <WorkflowCanvasHost
            graph={ctx.canvasGraph()}
            selectedNodeId={ctx.selectedNodeId()}
            selectedEdgeId={ctx.selectedEdgeId()}
            statusByNode={ctx.canvasStatusByNode()}
            subagentsByNode={ctx.canvasSubagentsByNode()}
            chatFocusNode={ctx.chatFocusNode()}
            viewportEnabled={!chatFocusActive()}
            runActive={Boolean(ctx.runState()?.active)}
            colorMode={ctx.resolvedTheme()}
            onSelectNode={ctx.handleSelectNode}
            onSelectEdge={ctx.handleSelectEdge}
            onUpdateNodePosition={ctx.handleCanvasNodePosition}
            onCreateEdge={ctx.handleCreateEdge}
            onReconnectEdge={ctx.handleReconnectEdge}
            onDeleteEdge={ctx.handleDeleteEdge}
            onAddNode={() => ctx.handleOpenAddNodePicker()}
            onInterruptNode={(nodeId) => void ctx.handleInterruptNode(nodeId)}
            onRetryNode={(nodeId) => void ctx.handleRetryNode(nodeId)}
          />
        </section>

        <DockPanel />
      </div>

      <Show when={!ctx.rightPanelHidden() && ctx.workflowSettingsOpen()}>
        <WorkflowSettingsPanel />
      </Show>
      <Show when={showInspectorPanel() && !ctx.rightPanelHidden()}>
        <InspectorPanel />
      </Show>
    </div>
  );
}
