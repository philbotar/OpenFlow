import { Show, createEffect, createMemo, createSignal } from "solid-js";
import GitBranch from "lucide-solid/icons/git-branch";
import Layers from "lucide-solid/icons/layers";
import WorkflowCanvasHost from "../../canvas/WorkflowCanvasHost";
import { Spinner } from "../Spinner";
import type {
  AppSettings,
  SkillSummary,
  Workflow,
  WorkflowAuthoringValidation,
} from "../../lib/types";
import { projectWorkflowCanvasGraph } from "../../lib/workflow";
import { AuthoringDraftInspector } from "./AuthoringDraftInspector";

const noop = () => undefined;

export function AuthoringDraftPreview(props: {
  draft: Workflow;
  validation: WorkflowAuthoringValidation | null;
  pendingChanges: boolean;
  busy: boolean;
  colorMode: "light" | "dark";
  uiZoom: number;
  settings: AppSettings;
  availableSkills: readonly SkillSummary[];
  onDraftChange: (mutator: (draft: Workflow) => void) => void;
}) {
  const [selectedNodeId, setSelectedNodeId] = createSignal<string | null>(null);
  const [selectedEdgeId, setSelectedEdgeId] = createSignal<string | null>(null);

  const graph = createMemo(() => projectWorkflowCanvasGraph(props.draft));
  const selectedNode = createMemo(() =>
    props.draft.nodes.find((node) => node.id === selectedNodeId()),
  );

  createEffect(() => {
    const selectedId = selectedNodeId();
    if (selectedId && !props.draft.nodes.some((node) => node.id === selectedId)) {
      setSelectedNodeId(null);
    }
  });

  const nodeCount = () => props.draft.nodes.length;
  const edgeCount = () => props.draft.edges.length;
  const layerCount = () => props.validation?.dag?.layerCount ?? null;

  return (
    <>
      <aside
        class="workflow-authoring-preview"
        aria-label={
          props.pendingChanges
            ? "Proposed workflow preview"
            : "Workflow starting point preview"
        }
      >
        <header class="workflow-authoring-preview-header">
          <div class="workflow-authoring-preview-heading">
            <p class="workflow-authoring-preview-eyebrow">
              {props.pendingChanges ? "Proposed workflow" : "Workflow starting point"}
            </p>
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

        <div class="workflow-authoring-preview-content">
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
              uiZoom={props.uiZoom}
              onSelectNode={(nodeId) => {
                setSelectedNodeId(nodeId);
                if (nodeId) setSelectedEdgeId(null);
              }}
              onSelectEdge={(edgeId) => {
                setSelectedNodeId(null);
                setSelectedEdgeId(edgeId);
              }}
              onUpdateNodePosition={noop}
              onAutoLayout={noop}
              onCreateEdge={noop}
              onReconnectEdge={noop}
              onDeleteEdge={noop}
              onDeleteNode={noop}
              onAddNode={noop}
            />
          </div>
        </div>
      </aside>

      <Show when={selectedNode()}>
        {(node) => (
          <AuthoringDraftInspector
            node={node()}
            settings={props.settings}
            workflowSettings={props.draft.settings}
            availableSkills={props.availableSkills}
            onNodeChange={(mutator) =>
              props.onDraftChange((draft) => {
                const draftNode = draft.nodes.find((item) => item.id === node().id);
                if (draftNode) mutator(draftNode);
              })
            }
          />
        )}
      </Show>
    </>
  );
}
