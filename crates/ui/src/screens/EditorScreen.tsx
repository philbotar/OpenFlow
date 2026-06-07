import { Show } from "solid-js";
import { useAppContext } from "../context/AppContext";
import { NodePickerModal } from "../components/NodePickerModal";
import { InspectorPanel } from "../panels/InspectorPanel";
import { WorkflowSettingsPanel } from "../panels/WorkflowSettingsPanel";
import { DockPanel } from "../panels/DockPanel";
import WorkflowCanvasHost from "../canvas/WorkflowCanvasHost";
import { COLLAPSED_DOCK_HEIGHT } from "../lib/utils";

export function EditorScreen() {
  const ctx = useAppContext();
  const showRightPanel = () =>
    ctx.workflowSettingsOpen() || Boolean(ctx.selectedNodeId());

  return (
    <div
      class="editor-screen"
      style={{
        "--dock-height": `${ctx.dockOpen() ? ctx.dockHeight() : COLLAPSED_DOCK_HEIGHT}px`,
      }}
    >
      <NodePickerModal />

      <div
        class="workspace-grid"
        classList={{ "workspace-grid--no-inspector": !showRightPanel() }}
      >
        <section class="canvas-panel">
          <WorkflowCanvasHost
            graph={ctx.canvasGraph()}
            selectedNodeId={ctx.selectedNodeId()}
            selectedEdgeId={ctx.selectedEdgeId()}
            statusByNode={ctx.canvasStatusByNode()}
            subagentsByNode={ctx.canvasSubagentsByNode()}
            onSelectNode={ctx.handleSelectNode}
            onSelectEdge={ctx.handleSelectEdge}
            onUpdateNodePosition={ctx.handleCanvasNodePosition}
            onCreateEdge={ctx.handleCreateEdge}
            onReconnectEdge={ctx.handleReconnectEdge}
            onDeleteEdge={ctx.handleDeleteEdge}
            onAddNode={() => ctx.handleOpenAddNodePicker()}
          />
        </section>

        <Show when={ctx.workflowSettingsOpen()}>
          <WorkflowSettingsPanel />
        </Show>
        <Show when={!ctx.workflowSettingsOpen() && ctx.selectedNodeId()}>
          <InspectorPanel />
        </Show>
      </div>

      <DockPanel />
    </div>
  );
}
