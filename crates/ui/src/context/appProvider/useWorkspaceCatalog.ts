import { createEffect, createMemo, createSignal, type Accessor, type Setter } from "solid-js";
import * as desktop from "../../api";
import { confirmNativeDialog, openNativeDialog } from "../../api";
import {
  executionCwdForWorkflow,
  findProjectForWorkflow,
  independentWorkflows,
  readExpandedProjectIds,
  workflowsAddableToProject,
  workflowsForProject,
  writeExpandedProjectIds,
} from "../../lib/projects";
import {
  activeProfile,
  cloneWorkflow,
  inferRunStateWorkflowId,
  replaceWorkflow,
} from "../../lib/workflow";
import { normalizeError } from "../../lib/utils";
import type {
  AgentDefinition,
  AppSettings,
  Project,
  RunSummary,
  ScheduleDraft,
  SkillSummary,
  Workflow,
  WorkflowRunState,
  WorkflowSchedule,
} from "../../lib/types";
import type { Screen } from "../../lib/types";

type ToastHandler = (message: string, context?: string) => void;

interface UseWorkspaceCatalogParams {
  applySchemaEditor: () => boolean;
  closeAddNodePicker: () => void;
  revealProjectsSection: () => void;
  navigateToScreen: (screen: Screen) => void;
  setScreen: Setter<Screen>;
  selectWorkflow: (workflow: Workflow) => void;
  runState: Accessor<WorkflowRunState | null>;
  backendRunWorkflowId: Accessor<string | null>;
  setBackendRunWorkflowId: Setter<string | null>;
  cacheRunStateForWorkflow: (workflowId: string, state: WorkflowRunState) => void;
  setRunStateByWorkflowId: (setter: (state: Record<string, WorkflowRunState>) => Record<string, WorkflowRunState>) => void;
  refreshReadiness: (nextSettings?: AppSettings) => Promise<void>;
  settings: Accessor<AppSettings>;
  setSettings: Setter<AppSettings>;
  showErrorToast: ToastHandler;
  showSuccessToast: ToastHandler;
}

export function useWorkspaceCatalog(params: UseWorkspaceCatalogParams) {
  const [workflows, setWorkflows] = createSignal<Workflow[]>([]);
  const [projects, setProjects] = createSignal<Project[]>([]);
  const [expandedProjectIds, setExpandedProjectIds] = createSignal(
    readExpandedProjectIds(globalThis.localStorage),
  );
  const [selectedProjectId, setSelectedProjectId] = createSignal<string | null>(null);
  const [activeWorkflowId, setActiveWorkflowId] = createSignal<string | null>(null);
  const [editingWorkflowId, setEditingWorkflowId] = createSignal<string | null>(null);
  const [workflowNameDraft, setWorkflowNameDraft] = createSignal("");
  const [agents, setAgents] = createSignal<AgentDefinition[]>([]);
  const [selectedAgentId, setSelectedAgentId] = createSignal<string | null>(null);
  const [editingAgentId, setEditingAgentId] = createSignal<string | null>(null);
  const [agentNameDraft, setAgentNameDraft] = createSignal("");
  const [agentSchemaDraft, setAgentSchemaDraft] = createSignal("");
  const [assignWorkflowPickerProjectId, setAssignWorkflowPickerProjectId] =
    createSignal<string | null>(null);
  const [availableSkills, setAvailableSkills] = createSignal<SkillSummary[]>([]);
  const [appReady, setAppReady] = createSignal(false);

  let workflowNameInput: HTMLInputElement | undefined;
  let agentNameInput: HTMLInputElement | undefined;

  const setWorkflowNameInputRef = (el: HTMLInputElement | undefined) => {
    workflowNameInput = el;
  };
  const setAgentNameInputRef = (el: HTMLInputElement | undefined) => {
    agentNameInput = el;
  };

  createEffect(() => {
    const workflowId = editingWorkflowId();
    if (!workflowId) return;
    queueMicrotask(() => {
      if (editingWorkflowId() !== workflowId || !workflowNameInput) return;
      workflowNameInput.focus();
      workflowNameInput.setSelectionRange(0, workflowNameInput.value.length);
    });
  });

  createEffect(() => {
    const agentId = editingAgentId();
    if (!agentId) return;
    queueMicrotask(() => {
      if (editingAgentId() !== agentId || !agentNameInput) return;
      agentNameInput.focus();
      agentNameInput.setSelectionRange(0, agentNameInput.value.length);
    });
  });

  const activeWorkflow = createMemo(() =>
    workflows().find((workflow) => workflow.id === activeWorkflowId()),
  );
  const independentWorkflowsMemo = createMemo(() =>
    independentWorkflows(workflows(), projects()),
  );
  const activeProject = createMemo(() => {
    const workflowId = activeWorkflowId();
    if (!workflowId) return undefined;
    const selected = selectedProjectId();
    if (selected) {
      const project = projects().find((item) => item.id === selected);
      if (project?.workflow_ids.includes(workflowId)) return project;
    }
    return findProjectForWorkflow(projects(), workflowId);
  });
  const executionCwdForActiveWorkflow = createMemo(() => {
    const workflowId = activeWorkflowId();
    if (!workflowId) return null;
    return executionCwdForWorkflow(projects(), workflowId, selectedProjectId());
  });
  const selectedAgent = createMemo(
    () => agents().find((agent) => agent.id === selectedAgentId()) ?? null,
  );
  const skillById = createMemo(() => {
    const map = new Map<string, SkillSummary>();
    for (const skill of availableSkills()) {
      map.set(skill.id, skill);
    }
    return map;
  });

  createEffect(() => {
    const agent = selectedAgent();
    setAgentSchemaDraft(agent ? JSON.stringify(agent.output_schema, null, 2) : "");
  });

  const initializeWorkspace = async (
    initialWorkflows: Workflow[],
    initialAgents: AgentDefinition[],
    initialProjects: Project[],
    initialSettings: AppSettings,
    initialRunState: WorkflowRunState | null,
  ) => {
    let nextWorkflows = initialWorkflows;
    if (nextWorkflows.length === 0) {
      nextWorkflows = [await desktop.createWorkflow("Workflow 1")];
    }
    const firstWorkflow = nextWorkflows[0];
    setWorkflows(nextWorkflows);
    setProjects(initialProjects);
    setAgents(initialAgents);
    setSelectedAgentId(initialAgents[0]?.id ?? null);
    setAgentSchemaDraft(
      initialAgents[0] ? JSON.stringify(initialAgents[0].output_schema, null, 2) : "",
    );
    const backendId = inferRunStateWorkflowId(initialRunState, nextWorkflows);
    params.setBackendRunWorkflowId(backendId);
    if (initialRunState && backendId) {
      params.cacheRunStateForWorkflow(backendId, initialRunState);
    }
    params.setSettings(structuredClone(initialSettings));
    setActiveWorkflowId(firstWorkflow.id);
    params.selectWorkflow(firstWorkflow);
    setAppReady(true);
    await params.refreshReadiness(initialSettings);
  };

  const handleSwitchWorkflow = (workflowId: string) => {
    if (!params.applySchemaEditor()) return;
    const workflow = workflows().find((item) => item.id === workflowId);
    if (!workflow) return;
    params.closeAddNodePicker();
    params.selectWorkflow(workflow);
    params.setScreen("editor");
  };

  const expandProject = (projectId: string) => {
    setExpandedProjectIds((current) => {
      const next = new Set(current);
      next.add(projectId);
      writeExpandedProjectIds(globalThis.localStorage, next);
      return next;
    });
  };

  const handleCreateWorkflow = async (projectId?: string) => {
    try {
      const workflow = await desktop.createWorkflow(`Workflow ${workflows().length + 1}`);
      setWorkflows([...workflows(), workflow]);
      if (projectId) {
        const nextProjects = await desktop.assignWorkflowToProject(projectId, workflow.id);
        setProjects(nextProjects);
        params.revealProjectsSection();
        expandProject(projectId);
        setSelectedProjectId(projectId);
      }
      params.selectWorkflow(workflow);
      params.setScreen("editor");
      params.showSuccessToast("Created workflow");
    } catch (error) {
      params.showErrorToast(normalizeError(error));
    }
  };

  const handleOpenAssignWorkflowPicker = (projectId: string) => {
    params.closeAddNodePicker();
    setSelectedProjectId(projectId);
    expandProject(projectId);
    setAssignWorkflowPickerProjectId(projectId);
  };

  const closeAssignWorkflowPicker = () => setAssignWorkflowPickerProjectId(null);

  const workflowsAddableToProjectMemo = (projectId: string) =>
    workflowsAddableToProject(workflows(), projects(), projectId);

  const handleCopyWorkflowToProject = async (projectId: string, sourceWorkflowId: string) => {
    try {
      const result = await desktop.copyWorkflowToProject(projectId, sourceWorkflowId);
      setWorkflows([...workflows(), result.workflow]);
      setProjects(result.projects);
      expandProject(projectId);
      setSelectedProjectId(projectId);
      closeAssignWorkflowPicker();
      params.selectWorkflow(result.workflow);
      params.setScreen("editor");
      params.showSuccessToast("Copied workflow");
    } catch (error) {
      params.showErrorToast(normalizeError(error));
    }
  };

  const handleDeleteActiveWorkflow = async () => {
    const workflow = activeWorkflow();
    if (!workflow) return;
    if (params.runState()?.active && params.backendRunWorkflowId() === workflow.id) {
      params.showErrorToast("Stop the run before deleting this workflow.");
      return;
    }
    const confirmed = await confirmNativeDialog(
      `Delete "${workflow.name}" permanently? This cannot be undone.`,
      { title: "Delete workflow", kind: "warning" },
    );
    if (!confirmed) return;
    try {
      const nextProjects = await desktop.deleteWorkflow(workflow.id);
      setProjects(nextProjects);
      params.setRunStateByWorkflowId((state) => {
        const { [workflow.id]: _removed, ...rest } = state;
        return rest;
      });
      let remaining = workflows().filter((item) => item.id !== workflow.id);
      if (remaining.length === 0) {
        const created = await desktop.createWorkflow("Workflow 1");
        remaining = [created];
      }
      setWorkflows(remaining);
      params.selectWorkflow(remaining[0]);
      params.showSuccessToast(`Deleted ${workflow.name}`);
    } catch (error) {
      params.showErrorToast(normalizeError(error));
    }
  };

  const handleAddProject = async () => {
    try {
      const selected = await openNativeDialog({
        directory: true,
        multiple: false,
        title: "Select project folder",
      });
      if (!selected || Array.isArray(selected)) return;
      const project = await desktop.createProjectFromDirectory(selected);
      setProjects([...projects(), project]);
      params.revealProjectsSection();
      setSelectedProjectId(project.id);
      setExpandedProjectIds((current) => {
        const next = new Set(current);
        next.add(project.id);
        writeExpandedProjectIds(globalThis.localStorage, next);
        return next;
      });
      params.showSuccessToast(`Added project ${project.name}`);
    } catch (error) {
      params.showErrorToast(normalizeError(error));
    }
  };

  const handleSelectProject = (projectId: string) => {
    setSelectedProjectId(projectId);
  };

  const handleToggleProjectExpanded = (projectId: string) => {
    setExpandedProjectIds((current) => {
      const next = new Set(current);
      if (next.has(projectId)) {
        next.delete(projectId);
      } else {
        next.add(projectId);
      }
      writeExpandedProjectIds(globalThis.localStorage, next);
      return next;
    });
  };

  const isProjectExpanded = (projectId: string) => expandedProjectIds().has(projectId);
  const workflowsForProjectMemo = (project: Project) => workflowsForProject(workflows(), project);

  const handleOpenAgents = () => {
    params.closeAddNodePicker();
    params.navigateToScreen("agents");
    if (!selectedAgentId() && agents().length > 0) {
      setSelectedAgentId(agents()[0].id);
    }
  };

  const handleOpenSchedule = () => {
    params.closeAddNodePicker();
    params.navigateToScreen("schedule");
  };

  const handleSaveWorkflowSchedule = async (
    workflowId: string,
    schedule: WorkflowSchedule | null,
  ) => {
    const current = workflows().find((workflow) => workflow.id === workflowId);
    if (!current) return;
    const next = cloneWorkflow(current);
    next.settings.schedule = schedule;
    try {
      const saved = await desktop.saveWorkflow(next);
      setWorkflows(replaceWorkflow(workflows(), saved));
      params.showSuccessToast(`Saved schedule for "${saved.name}"`);
    } catch (error) {
      params.showErrorToast(normalizeError(error));
    }
  };

  const scheduleFromPreset = (draft: ScheduleDraft) => desktop.scheduleFromPreset(draft);
  const scheduleDraftFromSchedule = (schedule: WorkflowSchedule) =>
    desktop.scheduleDraftFromSchedule(schedule);
  const describeWorkflowSchedule = (schedule: WorkflowSchedule) =>
    desktop.describeWorkflowSchedule(schedule);

  const updateSelectedAgent = (mutator: (draft: AgentDefinition) => void) => {
    const current = selectedAgent();
    if (!current) return;
    const next = { ...current, output_schema: structuredClone(current.output_schema) };
    mutator(next);
    setAgents(agents().map((agent) => (agent.id === next.id ? next : agent)));
  };

  const handleAgentSchemaInput = (text: string) => {
    setAgentSchemaDraft(text);
    try {
      const parsed = JSON.parse(text);
      updateSelectedAgent((draft) => {
        draft.output_schema = parsed;
      });
    } catch {
      // preserve draft until save
    }
  };

  const handleCreateAgent = async () => {
    try {
      const agent = await desktop.createAgentDefinition(`Agent ${agents().length + 1}`);
      const defaultModel = activeProfile(params.settings()).default_model;
      if (defaultModel && !agent.model) {
        agent.model = defaultModel;
      }
      setAgents([...agents(), agent]);
      setSelectedAgentId(agent.id);
      setAgentSchemaDraft(JSON.stringify(agent.output_schema, null, 2));
      params.setScreen("agents");
      params.showSuccessToast("Created agent");
    } catch (error) {
      params.showErrorToast(normalizeError(error));
    }
  };

  const handleStartAgentNameEdit = (agentId: string, currentName: string) => {
    setEditingAgentId(agentId);
    setAgentNameDraft(currentName);
  };

  const handleCancelAgentNameEdit = () => {
    setEditingAgentId(null);
    setAgentNameDraft("");
  };

  const handleAgentNameCommit = () => {
    const agentId = editingAgentId();
    if (!agentId) return;
    const nextName = agentNameDraft().trim();
    if (nextName !== "") {
      setAgents(
        agents().map((agent) => (agent.id === agentId ? { ...agent, name: nextName } : agent)),
      );
    }
    handleCancelAgentNameEdit();
  };

  const handleAgentNameKeyDown = (event: KeyboardEvent) => {
    if (event.key === "Enter") {
      event.preventDefault();
      handleAgentNameCommit();
      return;
    }
    if (event.key === "Escape") {
      event.preventDefault();
      handleCancelAgentNameEdit();
    }
  };

  const handleSaveAgents = async () => {
    if (selectedAgent()) {
      try {
        const parsed = JSON.parse(agentSchemaDraft());
        updateSelectedAgent((draft) => {
          draft.output_schema = parsed;
        });
      } catch (error) {
        params.showErrorToast(`agent output schema JSON invalid: ${normalizeError(error)}`);
        return;
      }
    }
    try {
      await desktop.saveAgents(agents());
      params.showSuccessToast("Saved agents");
    } catch (error) {
      params.showErrorToast(normalizeError(error));
    }
  };

  const handleStartWorkflowNameEdit = (workflowId: string, currentName: string) => {
    setEditingWorkflowId(workflowId);
    setWorkflowNameDraft(currentName);
  };
  const handleCancelWorkflowNameEdit = () => {
    setEditingWorkflowId(null);
    setWorkflowNameDraft("");
  };
  const handleWorkflowNameCommit = () => {
    const workflowId = editingWorkflowId();
    if (!workflowId) return;
    const nextName = workflowNameDraft().trim();
    if (nextName !== "") {
      setWorkflows(
        workflows().map((workflow) =>
          workflow.id === workflowId ? { ...workflow, name: nextName } : workflow,
        ),
      );
    }
    handleCancelWorkflowNameEdit();
  };
  const handleWorkflowNameKeyDown = (event: KeyboardEvent) => {
    if (event.key === "Enter") {
      event.preventDefault();
      handleWorkflowNameCommit();
      return;
    }
    if (event.key === "Escape") {
      event.preventDefault();
      handleCancelWorkflowNameEdit();
    }
  };

  const handleRefreshSkills = async () => {
    try {
      setAvailableSkills(await desktop.listSkills());
    } catch (error) {
      params.showErrorToast(normalizeError(error));
    }
  };

  return {
    workflows,
    setWorkflows,
    projects,
    setProjects,
    selectedProjectId,
    setSelectedProjectId,
    activeWorkflowId,
    setActiveWorkflowId,
    editingWorkflowId,
    workflowNameDraft,
    setWorkflowNameDraft,
    agents,
    setAgents,
    selectedAgentId,
    setSelectedAgentId,
    editingAgentId,
    agentNameDraft,
    setAgentNameDraft,
    agentSchemaDraft,
    assignWorkflowPickerProjectId,
    availableSkills,
    setAvailableSkills,
    skillById,
    appReady,
    setAppReady,
    activeWorkflow,
    activeProject,
    independentWorkflows: independentWorkflowsMemo,
    executionCwdForActiveWorkflow,
    selectedAgent,
    setWorkflowNameInputRef,
    setAgentNameInputRef,
    initializeWorkspace,
    handleSwitchWorkflow,
    handleCreateWorkflow,
    handleOpenAssignWorkflowPicker,
    closeAssignWorkflowPicker,
    workflowsAddableToProject: workflowsAddableToProjectMemo,
    handleCopyWorkflowToProject,
    handleDeleteActiveWorkflow,
    handleAddProject,
    handleSelectProject,
    handleToggleProjectExpanded,
    isProjectExpanded,
    workflowsForProject: workflowsForProjectMemo,
    handleOpenAgents,
    handleOpenSchedule,
    handleSaveWorkflowSchedule,
    scheduleFromPreset,
    scheduleDraftFromSchedule,
    describeWorkflowSchedule,
    handleCreateAgent,
    handleSaveAgents,
    handleAgentSchemaInput,
    updateSelectedAgent,
    handleStartAgentNameEdit,
    handleCancelAgentNameEdit,
    handleAgentNameCommit,
    handleAgentNameKeyDown,
    handleStartWorkflowNameEdit,
    handleCancelWorkflowNameEdit,
    handleWorkflowNameCommit,
    handleWorkflowNameKeyDown,
    handleRefreshSkills,
  };
}
