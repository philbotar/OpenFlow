import { describe, expect, test } from "vitest";
import type { Project, Workflow } from "../types";
import {
  executionCwdForWorkflow,
  independentWorkflows,
  workflowMembershipLabel,
  workflowsAddableToProject,
  workflowsForProject,
} from ".";

function makeProject(id: string, name: string, workflowIds: string[] = []): Project {
  return {
    id,
    path: `/tmp/${name}`,
    name,
    metadata: { description: "" },
    workflow_ids: workflowIds,
    default_execution_cwd: `/tmp/${name}`,
  };
}

function makeWorkflow(id: string): Workflow {
  return {
    id,
    name: id,
    nodes: [],
    edges: [],
    settings: { shared_context: "" },
  };
}

describe("projects helpers", () => {
  test("independentWorkflows excludes project-linked workflows", () => {
    const workflows = [makeWorkflow("a"), makeWorkflow("b"), makeWorkflow("c")];
    const projects = [makeProject("p1", "Repo", ["a", "b"])];

    expect(independentWorkflows(workflows, projects).map((w) => w.id)).toEqual(["c"]);
  });

  test("workflowsForProject returns linked workflows only", () => {
    const workflows = [makeWorkflow("a"), makeWorkflow("b"), makeWorkflow("c")];
    const project = makeProject("p1", "Repo", ["b"]);

    expect(workflowsForProject(workflows, project).map((w) => w.id)).toEqual(["b"]);
  });

  test("executionCwdForWorkflow uses active project when selected", () => {
    const projects = [
      makeProject("p1", "Repo", ["a"]),
      makeProject("p2", "Other", ["a"]),
    ];

    expect(executionCwdForWorkflow(projects, "a", "p2")).toBe("/tmp/Other");
    expect(executionCwdForWorkflow(projects, "c", "p1")).toBeNull();
  });

  test("workflowsAddableToProject excludes workflows already in target", () => {
    const workflows = [makeWorkflow("a"), makeWorkflow("b")];
    const projects = [makeProject("p1", "Repo", ["a"])];

    expect(workflowsAddableToProject(workflows, projects, "p1").map((w) => w.id)).toEqual(["b"]);
  });

  test("workflowMembershipLabel lists project memberships", () => {
    const projects = [makeProject("p1", "Repo", ["a"]), makeProject("p2", "Other", ["a"])];

    expect(workflowMembershipLabel(projects, "a")).toBe("Repo, Other");
    expect(workflowMembershipLabel(projects, "missing")).toBe("App workflows");
  });
});
