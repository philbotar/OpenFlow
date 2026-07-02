// @vitest-environment jsdom
import { render } from "solid-js/web";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import type { Project, Workflow } from "../../lib/types";
import { createEmptyToolConfig } from "../../lib/workflow/testHelpers";
import { ProjectFolderRow } from "./ProjectFolderRow";

const project: Project = {
  id: "project-1",
  name: "Demo Project",
  path: "/tmp/demo",
  metadata: { description: "" },
  workflow_ids: ["wf-1"],
  default_execution_cwd: "/tmp/demo",
};

const workflow: Workflow = {
  id: "wf-1",
  name: "Feature flow",
  nodes: [
    {
      id: "node-1",
      label: "Plan",
      kind: "Agent",
      position: { x: 0, y: 0 },
      agent: {
        system_prompt: "sys",
        task_prompt: "task",
        model: "gpt-4o-mini",
        output_schema: { type: "object" },
        auto_start: true,
        tools: createEmptyToolConfig(),
        callable_agents: [],
        allow_all_callable_agents: false,
      },
    },
  ],
  edges: [],
  settings: { shared_context: "" },
};

function defaultProps(overrides: Partial<Parameters<typeof ProjectFolderRow>[0]> = {}) {
  return {
    project,
    workflows: [workflow],
    expanded: false,
    selected: false,
    activeWorkflowId: null,
    screen: "editor",
    editingWorkflowId: null,
    workflowNameDraft: "",
    onToggleExpand: vi.fn(),
    onSelectProject: vi.fn(),
    onSelectWorkflow: vi.fn(),
    onRenameWorkflow: vi.fn(),
    onCreateWorkflow: vi.fn(),
    onAddExistingWorkflow: vi.fn(),
    setWorkflowNameInputRef: vi.fn(),
    setWorkflowNameDraft: vi.fn(),
    onWorkflowNameCommit: vi.fn(),
    onWorkflowNameKeyDown: vi.fn(),
    ...overrides,
  };
}

describe("ProjectFolderRow", () => {
  let container: HTMLDivElement;
  let dispose: (() => void) | undefined;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
  });

  afterEach(() => {
    dispose?.();
    container.remove();
  });

  test("selects project and toggles expand from header click", () => {
    const onSelectProject = vi.fn();
    const onToggleExpand = vi.fn();
    dispose = render(
      () => (
        <ProjectFolderRow
          {...defaultProps({ onSelectProject, onToggleExpand })}
        />
      ),
      container,
    );

    (container.querySelector(".project-folder-row") as HTMLButtonElement).click();

    expect(onSelectProject).toHaveBeenCalledTimes(1);
    expect(onToggleExpand).toHaveBeenCalledTimes(1);
  });

  test("shows workflow rows when expanded and selects workflow", () => {
    const onSelectWorkflow = vi.fn();
    dispose = render(
      () => (
        <ProjectFolderRow
          {...defaultProps({ expanded: true, onSelectWorkflow })}
        />
      ),
      container,
    );

    expect(container.textContent).toContain("Feature flow");
    (container.querySelector(".workflow-row-main") as HTMLButtonElement).click();
    expect(onSelectWorkflow).toHaveBeenCalledWith("wf-1");
  });

  test("add menu creates or assigns workflows", () => {
    const onCreateWorkflow = vi.fn();
    const onAddExistingWorkflow = vi.fn();
    dispose = render(
      () => (
        <ProjectFolderRow
          {...defaultProps({ onCreateWorkflow, onAddExistingWorkflow })}
        />
      ),
      container,
    );

    (container.querySelector(".project-folder-action") as HTMLButtonElement).click();
    const items = container.querySelectorAll(".project-folder-menu-item");
    expect(items).toHaveLength(2);

    (items[0] as HTMLButtonElement).click();
    expect(onCreateWorkflow).toHaveBeenCalledTimes(1);

    (container.querySelector(".project-folder-action") as HTMLButtonElement).click();
    (container.querySelectorAll(".project-folder-menu-item")[1] as HTMLButtonElement).click();
    expect(onAddExistingWorkflow).toHaveBeenCalledTimes(1);
  });

  test("closes menu on outside pointer down", () => {
    dispose = render(() => <ProjectFolderRow {...defaultProps()} />, container);

    (container.querySelector(".project-folder-action") as HTMLButtonElement).click();
    expect(container.querySelector(".project-folder-menu")).not.toBeNull();

    document.body.dispatchEvent(new MouseEvent("pointerdown", { bubbles: true }));
    expect(container.querySelector(".project-folder-menu")).toBeNull();
  });
});
