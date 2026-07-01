import { Show, createMemo, createSignal } from "solid-js";
import GitBranch from "lucide-solid/icons/git-branch";
import Layers from "lucide-solid/icons/layers";
import WorkflowCanvasHost from "../../canvas/WorkflowCanvasHost";
import { Spinner } from "../Spinner";
import type { Workflow, WorkflowAuthoringValidation } from "../../lib/types";
import { projectWorkflowCanvasGraph } from "../../lib/workflow";

const noop = () => undefined;

export function AuthoringDraftPreview(props: {
  draft: Workflow;
  validation: WorkflowAuthoringValidation | null;
  busy: boolean;
  colorMode: "light" | "dark";
}) {
  const [selectedNodeId, setSelectedNodeId] = createSignal<string | null>(null);
  const [selectedEdgeId, setSelectedEdgeId] = createSignal<string | null>(null);

  const graph = createMemo(() => projectWorkflowCanvasGraph(props.draft));

  const nodeCount = () => props.draft.nodes.length;
  const edgeCount = () => props.draft.edges.length;
  const layerCount = () => props.validation?.dag?.layerCount ?? null;

  return (
    <aside class="workflow-authoring-preview" aria-label="Proposed workflow preview">
      <header class="workflow-authoring-preview-header">
        <div class="workflow-authoring-preview-heading">
          <p class="workflow-authoring-preview-eyebrow">Proposed workflow</p>
          <h2 class="workflow-authoring-preview-title" title={props.draft.name}>
            {props.draft.name}
          </h2>
        </div>
        <Show when={props.busy}>
          <div class="workflow-authoring-preview-busy" aria-live="polite">
            <Spinner size="sm" />
            <span>Updating…</span>
          </div>
        </Show>
      </header>

      <div class="workflow-authoring-preview-meta" role="status">
        <span class="workflow-authoring-preview-stat">
          <Layers class="workflow-authoring-preview-stat-icon" aria-hidden="true" />
          {nodeCount()} node{nodeCount() === 1 ? "" : "s"}
        </span>
        <span class="workflow-authoring-preview-stat">
          <GitBranch class="workflow-authoring-preview-stat-icon" aria-hidden="true" />
          {edgeCount()} edge{edgeCount() === 1 ? "" : "s"}
        </span>
        <Show when={layerCount() !== null}>
          <span class="workflow-authoring-preview-stat">
            {layerCount()} layer{layerCount() === 1 ? "" : "s"}
          </span>
        </Show>
      </div>

      <div class="workflow-authoring-preview-canvas canvas-panel">
        <WorkflowCanvasHost
          graph={graph()}
          selectedNodeId={selectedNodeId()}
          selectedEdgeId={selectedEdgeId()}
          statusByNode={null}
          subagentsByNode={null}
          viewportEnabled
          previewMode
          colorMode={props.colorMode}
          onSelectNode={setSelectedNodeId}
          onSelectEdge={setSelectedEdgeId}
          onUpdateNodePosition={noop}
          onAutoLayout={noop}
          onCreateEdge={noop}
          onReconnectEdge={noop}
          onDeleteEdge={noop}
          onAddNode={noop}
        />
      </div>
    </aside>
  );
}
