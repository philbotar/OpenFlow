import { createSignal, type Accessor, type Setter } from "solid-js";
import { createStore, reconcile, type SetStoreFunction } from "solid-js/store";
import { toast } from "solid-sonner";
import * as desktop from "../../api";
import type {
  AppSettings,
  EdgeId,
  NodeId,
  Workflow,
  WorkflowRunState,
} from "../../lib/types";
import {
  cloneWorkflow,
  createIdleRunState,
  normalizeRunState,
  replaceWorkflow,
} from "../../lib/workflow";
import { normalizeError, STATUS_TOAST_ID, toastMessageForDebugMode } from "../../lib/utils";

type ToastHandler = (message: string, context?: string) => void;

export function createToastApi(
  settings: Accessor<AppSettings>,
  setLocalDebugLogPath: Setter<string | null>,
) {
  const debugOutputEnabled = () => settings().local_diagnostics?.debug_output === true;
  const appendLocalDebugLog = (level: string, message: string, context?: string) => {
    if (!debugOutputEnabled()) return;
    void desktop
      .appendDebugLog(settings(), {
        level,
        message,
        context: context ?? null,
      })
      .then((result) => {
        if (result.path) {
          setLocalDebugLogPath(result.path);
        }
      })
      .catch(() => undefined);
  };

  const clearStatusToast = () => toast.dismiss(STATUS_TOAST_ID);
  const showErrorToast: ToastHandler = (message, context) => {
    appendLocalDebugLog("error", message, context);
    toast.error(toastMessageForDebugMode(message, debugOutputEnabled()), {
      id: STATUS_TOAST_ID,
    });
  };
  const showSuccessToast: ToastHandler = (message, context) => {
    appendLocalDebugLog("success", message, context);
    toast.success(message, { id: STATUS_TOAST_ID });
  };
  const showInfoToast: ToastHandler = (message, context) => {
    appendLocalDebugLog("info", message, context);
    toast(toastMessageForDebugMode(message, debugOutputEnabled()), { id: STATUS_TOAST_ID });
  };

  return {
    clearStatusToast,
    showErrorToast,
    showSuccessToast,
    showInfoToast,
  };
}

export function createRunStateKernel(activeWorkflowId: Accessor<string | null>) {
  const [runStateStore, setRunStateStore] = createStore<{
    current: WorkflowRunState | null;
  }>({ current: null });
  const runState = () => runStateStore.current;
  const setRunState = (next: WorkflowRunState | null) => {
    const normalized = next === null ? null : normalizeRunState(next);
    if (normalized === null || runStateStore.current === null) {
      setRunStateStore("current", normalized);
      return;
    }
    setRunStateStore("current", reconcile(normalized, { key: "id" }));
  };
  const applyRunStateSnapshot = (next: WorkflowRunState | null) => {
    setRunStateStore("current", next === null ? null : normalizeRunState(next));
  };
  const [runStateByWorkflowId, setRunStateByWorkflowId] = createStore<
    Record<string, WorkflowRunState>
  >({});
  const [backendRunWorkflowId, setBackendRunWorkflowId] = createSignal<string | null>(null);

  const cacheRunStateForWorkflow = (workflowId: string, state: WorkflowRunState) => {
    setRunStateByWorkflowId(workflowId, normalizeRunState(state));
  };

  const publishBackendRunState = (nextRunState: WorkflowRunState) => {
    const backendId = backendRunWorkflowId();
    if (backendId) {
      cacheRunStateForWorkflow(backendId, nextRunState);
    }
    if (activeWorkflowId() === backendId) {
      setRunState(nextRunState);
    }
  };

  return {
    runState,
    setRunState,
    applyRunStateSnapshot,
    runStateByWorkflowId,
    setRunStateByWorkflowId,
    backendRunWorkflowId,
    setBackendRunWorkflowId,
    cacheRunStateForWorkflow,
    publishBackendRunState,
  };
}

export interface WorkflowMutationHelpers {
  updateActiveWorkflow: (mutator: (draft: Workflow) => void) => Workflow | null;
  validateActiveWorkflow: (workflow: Workflow, onInvalid?: () => void) => Promise<boolean>;
  updateCurrentNode: (mutator: (node: Workflow["nodes"][number]) => void) => void;
  updateCurrentNodeToolConfig: (
    mutator: (tools: Workflow["nodes"][number]["agent"]["tools"]) => void,
  ) => void;
}

export function createWorkflowMutationHelpers(params: {
  activeWorkflow: Accessor<Workflow | undefined>;
  workflows: Accessor<Workflow[]>;
  setWorkflows: Setter<Workflow[]>;
  selectedNodeId: Accessor<NodeId | null>;
  showErrorToast: ToastHandler;
}): WorkflowMutationHelpers {
  const updateActiveWorkflow = (mutator: (draft: Workflow) => void): Workflow | null => {
    const workflow = params.activeWorkflow();
    if (!workflow) return null;
    const next = cloneWorkflow(workflow);
    mutator(next);
    params.setWorkflows(replaceWorkflow(params.workflows(), next));
    return next;
  };

  const validateActiveWorkflow = async (
    workflow: Workflow,
    onInvalid?: () => void,
  ): Promise<boolean> => {
    try {
      await desktop.validateWorkflow(workflow);
      return true;
    } catch (error) {
      onInvalid?.();
      params.showErrorToast(normalizeError(error));
      return false;
    }
  };

  const updateCurrentNode = (mutator: (node: Workflow["nodes"][number]) => void) => {
    const nodeId = params.selectedNodeId();
    if (!nodeId) return;
    updateActiveWorkflow((draft) => {
      const nextNode = draft.nodes.find((item) => item.id === nodeId);
      if (nextNode) mutator(nextNode);
    });
  };

  const updateCurrentNodeToolConfig = (
    mutator: (tools: Workflow["nodes"][number]["agent"]["tools"]) => void,
  ) => {
    updateCurrentNode((node) => mutator(node.agent.tools));
  };

  return {
    updateActiveWorkflow,
    validateActiveWorkflow,
    updateCurrentNode,
    updateCurrentNodeToolConfig,
  };
}

export async function persistAll(params: {
  applySchemaEditor: () => boolean;
  workflows: Accessor<Workflow[]>;
  settings: Accessor<AppSettings>;
  showSuccessToast: ToastHandler;
  showErrorToast: ToastHandler;
  successText?: string;
}) {
  if (!params.applySchemaEditor()) return false;
  try {
    await desktop.saveWorkflows(params.workflows());
    await desktop.saveSettings(params.settings());
    params.showSuccessToast(params.successText ?? "Saved");
    return true;
  } catch (error) {
    params.showErrorToast(normalizeError(error));
    return false;
  }
}

export function selectWorkflow(params: {
  workflow: Workflow;
  activeWorkflowId: Accessor<string | null>;
  backendRunWorkflowId: Accessor<string | null>;
  runState: Accessor<WorkflowRunState | null>;
  runStateByWorkflowId: Record<string, WorkflowRunState>;
  cacheRunStateForWorkflow: (workflowId: string, state: WorkflowRunState) => void;
  applyRunStateSnapshot: (next: WorkflowRunState | null) => void;
  setActiveWorkflowId: Setter<string | null>;
  setSelectedNodeId: Setter<NodeId | null>;
  setSelectedEdgeId: Setter<EdgeId | null>;
  setEditingNodeId: Setter<NodeId | null>;
  setNodeLabelDraft: Setter<string>;
  setSelectedTraceIndex: Setter<number | null>;
  resetWorkflowChatUi: () => void;
}) {
  const restoreRunStateForWorkflow = (workflow: Workflow) => {
    const workflowId = workflow.id;
    const backendId = params.backendRunWorkflowId();
    const cached = params.runStateByWorkflowId[workflowId];
    if (backendId === workflowId) {
      params.applyRunStateSnapshot(cached ?? createIdleRunState(workflow));
      void desktop
        .getRunState()
        .then((live) => {
          if (!live || params.activeWorkflowId() !== workflowId) {
            return;
          }
          params.cacheRunStateForWorkflow(workflowId, live);
          params.applyRunStateSnapshot(live);
        })
        .catch(() => undefined);
      return;
    }
    params.applyRunStateSnapshot(cached ?? createIdleRunState(workflow));
  };

  const previousId = params.activeWorkflowId();
  if (previousId && previousId !== params.workflow.id) {
    const backendId = params.backendRunWorkflowId();
    const toCache =
      previousId === backendId ? params.runStateByWorkflowId[previousId] ?? params.runState() : params.runState();
    if (toCache) {
      params.cacheRunStateForWorkflow(previousId, toCache);
    }
  }
  params.setActiveWorkflowId(params.workflow.id);
  params.setSelectedNodeId(params.workflow.nodes[0]?.id ?? null);
  params.setSelectedEdgeId(null);
  params.setEditingNodeId(null);
  params.setNodeLabelDraft("");
  params.setSelectedTraceIndex(null);
  params.resetWorkflowChatUi();
  restoreRunStateForWorkflow(params.workflow);
}
