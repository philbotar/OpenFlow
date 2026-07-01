import type { Project, Workflow } from "../types";

export const EXPANDED_PROJECTS_STORAGE_KEY = "openflow.expandedProjectIds";

type StorageLike = Pick<Storage, "getItem" | "setItem" | "removeItem"> | null | undefined;

export function allAssignedWorkflowIds(projects: Project[]): Set<string> {
  return new Set(projects.flatMap((project) => project.workflow_ids));
}

export function independentWorkflows(workflows: Workflow[], projects: Project[]): Workflow[] {
  const assigned = allAssignedWorkflowIds(projects);
  return workflows.filter((workflow) => !assigned.has(workflow.id));
}

export function projectsContainingWorkflow(
  projects: Project[],
  workflowId: string,
): Project[] {
  return projects.filter((project) => project.workflow_ids.includes(workflowId));
}

export function findProjectForWorkflow(
  projects: Project[],
  workflowId: string,
): Project | undefined {
  return projects.find((project) => project.workflow_ids.includes(workflowId));
}

export function workflowsForProject(workflows: Workflow[], project: Project): Workflow[] {
  const ids = new Set(project.workflow_ids);
  return workflows.filter((workflow) => ids.has(workflow.id));
}

export function workflowsAddableToProject(
  workflows: Workflow[],
  projects: Project[],
  projectId: string,
): Workflow[] {
  const target = projects.find((project) => project.id === projectId);
  if (!target) return [];
  const inTarget = new Set(target.workflow_ids);
  return workflows.filter((workflow) => !inTarget.has(workflow.id));
}

export function workflowMembershipLabel(projects: Project[], workflowId: string): string {
  const memberships = projectsContainingWorkflow(projects, workflowId).map((project) => project.name);
  if (memberships.length > 0) return memberships.join(", ");
  return "App workflows";
}

export function executionCwdForWorkflow(
  projects: Project[],
  workflowId: string,
  activeProjectId: string | null = null,
): string | null {
  const memberships = projectsContainingWorkflow(projects, workflowId);
  if (memberships.length === 0) return null;

  const active =
    activeProjectId
      ? memberships.find((project) => project.id === activeProjectId)
      : undefined;
  const project = active ?? memberships[0];
  const cwd = (project.default_execution_cwd || project.path).trim();
  return cwd === "" ? null : cwd;
}

export function readExpandedProjectIds(storage: StorageLike): Set<string> {
  const raw = storage?.getItem(EXPANDED_PROJECTS_STORAGE_KEY);
  if (!raw) return new Set();
  try {
    return new Set(JSON.parse(raw) as string[]);
  } catch {
    return new Set();
  }
}

export function writeExpandedProjectIds(storage: StorageLike, ids: Set<string>) {
  storage?.setItem(EXPANDED_PROJECTS_STORAGE_KEY, JSON.stringify([...ids]));
}
