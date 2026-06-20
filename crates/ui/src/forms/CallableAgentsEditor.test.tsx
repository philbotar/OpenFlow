// @vitest-environment jsdom
import { render } from "solid-js/web";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { CallableAgentsEditor } from "./CallableAgentsEditor";
import type { AgentDefinition } from "../lib/types";

const agents: AgentDefinition[] = [
  {
    id: "agent-1",
    name: "Researcher",
    system_prompt: "sys",
    task_prompt: "task",
    model: "gpt-4o-mini",
    output_schema: { type: "object" },
    auto_start: true,
    tools: {
      approvalMode: null,
    },
  },
];

describe("CallableAgentsEditor", () => {
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

  test("renders saved agents when expanded", () => {
    dispose = render(
      () => (
        <CallableAgentsEditor
          allowAll={false}
          selectedIds={[]}
          agents={agents}
          onAllowAllChange={() => {}}
          onToggle={() => {}}
        />
      ),
      container,
    );

    expect(container.textContent).toContain("Researcher");
    expect(container.textContent).toContain("Allow all agents");
  });

  test("calls onToggle when agent checkbox changes", () => {
    const onToggle = vi.fn();
    dispose = render(
      () => (
        <CallableAgentsEditor
          allowAll={false}
          selectedIds={[]}
          agents={agents}
          onAllowAllChange={() => {}}
          onToggle={onToggle}
        />
      ),
      container,
    );

    const checkboxes = container.querySelectorAll('input[type="checkbox"]');
    const agentCheckbox = checkboxes[1] as HTMLInputElement;
    agentCheckbox.click();
    expect(onToggle).toHaveBeenCalledWith("agent-1", true);
  });

  test("calls onAllowAllChange from allow-all checkbox", () => {
    const onAllowAllChange = vi.fn();
    dispose = render(
      () => (
        <CallableAgentsEditor
          allowAll={false}
          selectedIds={[]}
          agents={agents}
          onAllowAllChange={onAllowAllChange}
          onToggle={() => {}}
        />
      ),
      container,
    );

    const allowAllCheckbox = container.querySelector(
      'input[type="checkbox"]',
    ) as HTMLInputElement;
    allowAllCheckbox.click();
    expect(onAllowAllChange).toHaveBeenCalledWith(true);
  });

  test("disables individual agent checkboxes when allow all is enabled", () => {
    dispose = render(
      () => (
        <CallableAgentsEditor
          allowAll
          selectedIds={[]}
          agents={agents}
          onAllowAllChange={() => {}}
          onToggle={() => {}}
        />
      ),
      container,
    );

    const checkboxes = container.querySelectorAll('input[type="checkbox"]');
    const agentCheckbox = checkboxes[1] as HTMLInputElement;
    expect(agentCheckbox.disabled).toBe(true);
    expect(agentCheckbox.checked).toBe(true);
  });
});
