import { createMemo, createSignal, type Accessor, type Setter } from "solid-js";
import {
  LEFT_PANEL_VISIBILITY_STORAGE_KEY,
  PANEL_VISIBILITY_STORAGE_KEY,
  PROJECTS_SECTION_STORAGE_KEY,
  readStoredBoolean,
  WORKFLOWS_SECTION_STORAGE_KEY,
  writeStoredBoolean,
} from "../../lib/storedBoolean";
import type {
  AppSettings,
  EdgeId,
  NodeId,
  ProviderProfile,
  Workflow,
  WorkflowRunState,
} from "../../lib/types";
import {
  dagreLayoutWorkflowLeftToRight,
  nextNodePlacement,
  nodeOutput,
  projectWorkflowCanvasGraph,
  projectWorkflowCanvasStatusByNode,
  projectWorkflowCanvasSubagentsByNode,
  removeSelectedNode,
  replaceWorkflow,
  selectedNode,
  withDefaultReasoningFromProfile,
  withDefaultReasoningFromWorkflow,
  type WorkflowCanvasGraph,
  type WorkflowCanvasStatusByNode,
  type WorkflowCanvasSubagentsByNode,
} from "../../lib/workflow";
import { normalizeError } from "../../lib/utils";
import { createWorkflowMutationHelpers, persistAll } from "./shared";
import * as desktop from "../../api";

type ToastHandler = (message: string, context?: string) => void;

interface UseWorkflowEditorParams {
  workflows: Accessor<Workflow[]>;
  setWorkflows: Setter<Workflow[]>;
  activeWorkflow: Accessor<Workflow | undefined>;
  runState: Accessor<WorkflowRunState | null>;
  settings: Accessor<AppSettings>;
  activeProfileMemo: Accessor<ProviderProfile>;
  isCompactViewport: Accessor<boolean>;
  showErrorToast: ToastHandler;
  showSuccessToast: ToastHandler;
  clearStatusToast: () => void;
}

export function useWorkflowEditor(params: UseWorkflowEditorParams) {
  const [selectedNodeId, setSelectedNodeId] = createSignal<NodeId | null>(null);
  const [selectedEdgeId, setSelectedEdgeId] = createSignal<EdgeId | null>(null);
  const [schemaText, setSchemaText] = createSignal("");
  const [rightPanelHidden, setRightPanelHidden] = createSignal(
    readStoredBoolean(globalThis.localStorage, PANEL_VISIBILITY_STORAGE_KEY),
  );
  const [leftPanelHidden, setLeftPanelHidden] = createSignal(
    readStoredBoolean(globalThis.localStorage, LEFT_PANEL_VISIBILITY_STORAGE_KEY),
  );
  const [workflowsSectionHidden, setWorkflowsSectionHidden] = createSignal(
    readStoredBoolean(globalThis.localStorage, WORKFLOWS_SECTION_STORAGE_KEY),
  );
  const workflowsSectionExpanded = createMemo(() => !workflowsSectionHidden());
  const [projectsSectionHidden, setProjectsSectionHidden] = createSignal(
    readStoredBoolean(globalThis.localStorage, PROJECTS_SECTION_STORAGE_KEY),
  );
  const projectsSectionExpanded = createMemo(() => !projectsSectionHidden());
  const [workflowSettingsOpen, setWorkflowSettingsOpen] = createSignal(false);
  const [inspectorOpen, setInspectorOpen] = createSignal(false);
  const [gitPanelOpen, setGitPanelOpen] = createSignal(false);
  const [editingNodeId, setEditingNodeId] = createSignal<NodeId | null>(null);
  const [nodeLabelDraft, setNodeLabelDraft] = createSignal("");
  const [addNodePickerOpen, setAddNodePickerOpen] = createSignal(false);

  const canvasGraph = createMemo<WorkflowCanvasGraph | null>(
    (previous) => projectWorkflowCanvasGraph(params.activeWorkflow(), previous),
    null,
  );
  const canvasStatusByNode = createMemo<WorkflowCanvasStatusByNode | null>(
    (previous) => projectWorkflowCanvasStatusByNode(params.runState(), previous),
    null,
  );
  const canvasSubagentsByNode = createMemo<WorkflowCanvasSubagentsByNode | null>(
    (previous) => projectWorkflowCanvasSubagentsByNode(params.runState(), previous),
    null,
  );
  const currentNode = createMemo(() =>
    selectedNode(params.activeWorkflow(), selectedNodeId()),
  );
  const currentNodeOutput = createMemo(() => nodeOutput(params.runState(), selectedNodeId()));

  const workflowMutations = createWorkflowMutationHelpers({
    activeWorkflow: params.activeWorkflow,
    workflows: params.workflows,
    setWorkflows: params.setWorkflows,
    selectedNodeId,
    showErrorToast: params.showErrorToast,
  });

  const closeAddNodePicker = () => setAddNodePickerOpen(false);

  const updateActiveWorkflowSettings = (
    mutator: (settings: Workflow["settings"]) => void,
  ) => {
    workflowMutations.updateActiveWorkflow((draft) => {
      mutator(draft.settings);
    });
  };

  const handleToggleWorkflowSettings = () => {
    const opening = !workflowSettingsOpen();
    setWorkflowSettingsOpen(opening);
    if (opening) {
      setInspectorOpen(false);
      setGitPanelOpen(false);
      setRightPanelHidden(false);
      writeStoredBoolean(globalThis.localStorage, PANEL_VISIBILITY_STORAGE_KEY, false);
    }
  };

  const handleToggleInspector = () => {
    const opening = !inspectorOpen();
    if (opening) {
      setWorkflowSettingsOpen(false);
      setGitPanelOpen(false);
      const node = selectedNodeId() ?? params.activeWorkflow()?.nodes[0]?.id ?? null;
      if (!node) {
        return;
      }
      setSelectedNodeId(node);
      setInspectorOpen(true);
      setRightPanelHidden(false);
      writeStoredBoolean(globalThis.localStorage, PANEL_VISIBILITY_STORAGE_KEY, false);
      return;
    }
    setInspectorOpen(false);
  };

  const handleToggleGitPanel = () => {
    const opening = !gitPanelOpen();
    setGitPanelOpen(opening);
    if (opening) {
      setInspectorOpen(false);
      setWorkflowSettingsOpen(false);
      setRightPanelHidden(false);
      writeStoredBoolean(globalThis.localStorage, PANEL_VISIBILITY_STORAGE_KEY, false);
    }
  };

  const handleToggleRightPanel = () => {
    const currentlyHidden = rightPanelHidden();
    if (currentlyHidden) {
      setRightPanelHidden(false);
      writeStoredBoolean(globalThis.localStorage, PANEL_VISIBILITY_STORAGE_KEY, false);
    } else {
      setRightPanelHidden(true);
      writeStoredBoolean(globalThis.localStorage, PANEL_VISIBILITY_STORAGE_KEY, true);
    }
  };

  const handleToggleLeftPanel = () => {
    if (params.isCompactViewport()) return;
    const next = !leftPanelHidden();
    setLeftPanelHidden(next);
    writeStoredBoolean(globalThis.localStorage, LEFT_PANEL_VISIBILITY_STORAGE_KEY, next);
  };

  const handleToggleWorkflowsSection = () => {
    const next = !workflowsSectionExpanded();
    setWorkflowsSectionHidden(!next);
    writeStoredBoolean(globalThis.localStorage, WORKFLOWS_SECTION_STORAGE_KEY, !next);
  };

  const revealProjectsSection = () => {
    if (!projectsSectionExpanded()) {
      setProjectsSectionHidden(false);
      writeStoredBoolean(globalThis.localStorage, PROJECTS_SECTION_STORAGE_KEY, false);
    }
  };

  const handleToggleProjectsSection = () => {
    const next = !projectsSectionExpanded();
    setProjectsSectionHidden(!next);
    writeStoredBoolean(globalThis.localStorage, PROJECTS_SECTION_STORAGE_KEY, !next);
  };

  const handleSelectNodeBase = (nodeId: NodeId | null) => {
    setSelectedEdgeId(null);
    setSelectedNodeId(nodeId);
    setEditingNodeId(null);
    setNodeLabelDraft("");
    if (!nodeId) {
      setInspectorOpen(false);
    }
  };

  const handleSelectEdge = (edgeId: EdgeId | null) => {
    setSelectedEdgeId(edgeId);
    if (edgeId) {
      setSelectedNodeId(null);
      setInspectorOpen(false);
    }
    setEditingNodeId(null);
    setNodeLabelDraft("");
  };

  const handleCanvasNodePosition = (nodeId: NodeId, x: number, y: number) => {
    workflowMutations.updateActiveWorkflow((draft) => {
      const node = draft.nodes.find((item) => item.id === nodeId);
      if (node) {
        node.position.x = x;
        node.position.y = y;
      }
    });
  };

  const handleAutoLayoutWorkflow = () => {
    const workflow = params.activeWorkflow();
    if (!workflow) {
      return;
    }
    const next = dagreLayoutWorkflowLeftToRight(workflow);
    params.setWorkflows(replaceWorkflow(params.workflows(), next));
    params.showSuccessToast("Auto-laid out workflow");
  };

  const handleOpenAddNodePicker = () => {
    if (!params.activeWorkflow()) return;
    setSelectedEdgeId(null);
    setAddNodePickerOpen(true);
  };

  const applySchemaEditor = () => {
    const nodeId = selectedNodeId();
    const workflow = params.activeWorkflow();
    if (!nodeId || !workflow) return true;
    try {
      const parsed = JSON.parse(schemaText());
      workflowMutations.updateActiveWorkflow((draft) => {
        const node = draft.nodes.find((item) => item.id === nodeId);
        if (node) node.agent.output_schema = parsed;
      });
      params.clearStatusToast();
      return true;
    } catch (error) {
      params.showErrorToast(`output schema JSON invalid: ${normalizeError(error)}`);
      return false;
    }
  };

  const persistAllChanges = async (successText = "Saved") =>
    persistAll({
      applySchemaEditor,
      workflows: params.workflows,
      settings: params.settings,
      showSuccessToast: params.showSuccessToast,
      showErrorToast: params.showErrorToast,
      successText,
    });

  const handleAddNode = async (agentId: string | null) => {
    const workflow = params.activeWorkflow();
    if (!workflow) return;
    const placement = nextNodePlacement(workflow);
    try {
      const node = await desktop.createAgentNode(
        placement.index,
        placement.x,
        placement.y,
        agentId,
      );
      const profile = params.activeProfileMemo();
      let nextAgent = withDefaultReasoningFromWorkflow(
        withDefaultReasoningFromProfile(node.agent, profile),
        workflow.settings,
      );
      if (profile.default_model && !nextAgent.model) {
        nextAgent = { ...nextAgent, model: profile.default_model };
      }
      const nextNode = { ...node, agent: nextAgent };
      const nextWorkflow = workflowMutations.updateActiveWorkflow((draft) => {
        draft.nodes.push(nextNode);
      });
      closeAddNodePicker();
      setSelectedNodeId(nextNode.id);
      setSelectedEdgeId(null);
      setEditingNodeId(null);
      setNodeLabelDraft("");
      if (!nextWorkflow) return;
      const valid = await workflowMutations.validateActiveWorkflow(nextWorkflow, () => {
        workflowMutations.updateActiveWorkflow((draft) => {
          draft.nodes = draft.nodes.filter((item) => item.id !== nextNode.id);
        });
        setSelectedNodeId(workflow.nodes[0]?.id ?? null);
      });
      if (valid) {
        params.showSuccessToast(agentId ? "Added saved agent to workflow" : "Added node");
      }
    } catch (error) {
      params.showErrorToast(normalizeError(error));
    }
  };

  const handleDeleteSelectedNode = () => {
    const workflow = params.activeWorkflow();
    const nodeId = selectedNodeId();
    if (!workflow || !nodeId) return;
    const next = removeSelectedNode(workflow, nodeId);
    params.setWorkflows(replaceWorkflow(params.workflows(), next));
    const nextSelected = next.nodes[0]?.id ?? null;
    setSelectedNodeId(nextSelected);
    if (!nextSelected) {
      setInspectorOpen(false);
    }
    setSelectedEdgeId(null);
    setEditingNodeId(null);
    setNodeLabelDraft("");
  };

  const handleDeleteEdge = (edgeId: EdgeId) => {
    workflowMutations.updateActiveWorkflow((draft) => {
      draft.edges = draft.edges.filter((edge) => edge.id !== edgeId);
    });
    if (selectedEdgeId() === edgeId) setSelectedEdgeId(null);
  };

  const handleCreateEdge = (from: NodeId, to: NodeId) => {
    if (from === to) return;
    const edgeId = crypto.randomUUID();
    let created = false;
    const nextWorkflow = workflowMutations.updateActiveWorkflow((draft) => {
      const duplicate = draft.edges.some((edge) => edge.from === from && edge.to === to);
      if (duplicate) return;
      draft.edges.push({ id: edgeId, from, to });
      created = true;
    });
    if (created) {
      setSelectedNodeId(null);
      setSelectedEdgeId(edgeId);
      setEditingNodeId(null);
      setNodeLabelDraft("");
      if (nextWorkflow) {
        void workflowMutations.validateActiveWorkflow(nextWorkflow, () => {
          workflowMutations.updateActiveWorkflow((draft) => {
            draft.edges = draft.edges.filter((edge) => edge.id !== edgeId);
          });
          setSelectedEdgeId(null);
        });
      }
    }
  };

  const handleReconnectEdge = (edgeId: EdgeId, from: NodeId, to: NodeId) => {
    if (from === to) return;
    const existing = params.activeWorkflow()?.edges.find((edge) => edge.id === edgeId);
    if (!existing) return;
    const previousFrom = existing.from;
    const previousTo = existing.to;
    let reconnected = false;
    const nextWorkflow = workflowMutations.updateActiveWorkflow((draft) => {
      const duplicate = draft.edges.some(
        (edge) => edge.id !== edgeId && edge.from === from && edge.to === to,
      );
      if (duplicate) return;
      const edge = draft.edges.find((item) => item.id === edgeId);
      if (!edge) return;
      edge.from = from;
      edge.to = to;
      reconnected = true;
    });
    if (reconnected) {
      setSelectedNodeId(null);
      setSelectedEdgeId(edgeId);
      setEditingNodeId(null);
      setNodeLabelDraft("");
      if (nextWorkflow) {
        void workflowMutations.validateActiveWorkflow(nextWorkflow, () => {
          workflowMutations.updateActiveWorkflow((draft) => {
            const edge = draft.edges.find((item) => item.id === edgeId);
            if (edge) {
              edge.from = previousFrom;
              edge.to = previousTo;
            }
          });
        });
      }
    }
  };

  const handleStartNodeLabelEdit = (nodeId: NodeId, currentLabel: string) => {
    setEditingNodeId(nodeId);
    setNodeLabelDraft(currentLabel);
  };

  const handleCancelNodeLabelEdit = () => {
    setEditingNodeId(null);
    setNodeLabelDraft("");
  };

  const handleCommitNodeLabel = () => {
    const nodeId = editingNodeId();
    if (!nodeId) return;
    const currentLabel = currentNode()?.label ?? "";
    const trimmed = nodeLabelDraft().trim();
    const nextLabel = trimmed === "" ? currentLabel : trimmed;
    workflowMutations.updateActiveWorkflow((draft) => {
      const nextNode = draft.nodes.find((item) => item.id === nodeId);
      if (nextNode) nextNode.label = nextLabel;
    });
    handleCancelNodeLabelEdit();
  };

  return {
    selectedNodeId,
    setSelectedNodeId,
    selectedEdgeId,
    setSelectedEdgeId,
    schemaText,
    setSchemaText,
    rightPanelHidden,
    leftPanelHidden,
    workflowSettingsOpen,
    inspectorOpen,
    gitPanelOpen,
    setGitPanelOpen,
    workflowsSectionExpanded,
    projectsSectionExpanded,
    editingNodeId,
    setEditingNodeId,
    nodeLabelDraft,
    setNodeLabelDraft,
    addNodePickerOpen,
    setAddNodePickerOpen,
    closeAddNodePicker,
    revealProjectsSection,
    canvasGraph,
    canvasStatusByNode,
    canvasSubagentsByNode,
    currentNode,
    currentNodeOutput,
    updateActiveWorkflow: workflowMutations.updateActiveWorkflow,
    validateActiveWorkflow: workflowMutations.validateActiveWorkflow,
    updateCurrentNode: workflowMutations.updateCurrentNode,
    updateCurrentNodeToolConfig: workflowMutations.updateCurrentNodeToolConfig,
    updateActiveWorkflowSettings,
    applySchemaEditor,
    persistAll: persistAllChanges,
    handleSelectNodeBase,
    handleSelectEdge,
    handleCanvasNodePosition,
    handleAutoLayoutWorkflow,
    handleCreateEdge,
    handleReconnectEdge,
    handleDeleteEdge,
    handleDeleteSelectedNode,
    handleOpenAddNodePicker,
    handleAddNode,
    handleStartNodeLabelEdit,
    handleCancelNodeLabelEdit,
    handleCommitNodeLabel,
    handleToggleWorkflowSettings,
    handleToggleInspector,
    handleToggleGitPanel,
    handleToggleRightPanel,
    handleToggleLeftPanel,
    handleToggleWorkflowsSection,
    handleToggleProjectsSection,
  };
}
