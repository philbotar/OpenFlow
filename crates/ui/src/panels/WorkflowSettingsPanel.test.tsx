// @vitest-environment jsdom
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, beforeEach, describe, expect, test } from "vitest";
import type { Workflow } from "../lib/types";
import { createEmptyToolConfig } from "../lib/workflow";
import { AppContext, type AppContextValue } from "../context/AppContext";
import { WorkflowSettingsPanel } from "./WorkflowSettingsPanel";

function makeWorkflow(overrides: Partial<Workflow["settings"]> = {}): Workflow {
  return {
    id: "wf-1",
    name: "Smoke",
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
    settings: {
      shared_context: "Initial context",
      retry_policy: { max_attempts: 3, backoff_ms: 1_000 },
      ...overrides,
    },
  };
}

describe("WorkflowSettingsPanel", () => {
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

  function renderPanel(initial: Workflow) {
    const [workflow, setWorkflow] = createSignal(initial);
    const updateActiveWorkflowSettings: AppContextValue["updateActiveWorkflowSettings"] = (
      mutator,
    ) => {
      setWorkflow((current) => {
        const next = structuredClone(current);
        mutator(next.settings);
        return next;
      });
    };

    const ctx = {
      activeWorkflow: workflow,
      updateActiveWorkflowSettings,
    } as AppContextValue;

    dispose = render(
      () => (
        <AppContext.Provider value={ctx}>
          <WorkflowSettingsPanel />
        </AppContext.Provider>
      ),
      container,
    );

    return { workflow };
  }

  test("renders shared context and retry fields from active workflow", () => {
    renderPanel(makeWorkflow());

    const textarea = container.querySelector("textarea") as HTMLTextAreaElement;
    const numberInputs = container.querySelectorAll('input[type="number"]');

    expect(textarea.value).toBe("Initial context");
    expect((numberInputs[0] as HTMLInputElement | undefined)?.value).toBe("3");
    expect((numberInputs[1] as HTMLInputElement | undefined)?.value).toBe("1000");
  });

  test("updates shared context through context mutator", () => {
    const { workflow } = renderPanel(makeWorkflow());
    const textarea = container.querySelector("textarea") as HTMLTextAreaElement;

    textarea.value = "Updated shared context";
    textarea.dispatchEvent(new Event("input", { bubbles: true }));

    expect(workflow().settings.shared_context).toBe("Updated shared context");
  });

  test("clamps max retry attempts between 0 and 10", () => {
    const { workflow } = renderPanel(makeWorkflow());
    const maxAttemptsInput = container.querySelectorAll('input[type="number"]')[0] as HTMLInputElement;

    maxAttemptsInput.value = "99";
    maxAttemptsInput.dispatchEvent(new Event("input", { bubbles: true }));
    expect(workflow().settings.retry_policy?.max_attempts).toBe(10);

    maxAttemptsInput.value = "-5";
    maxAttemptsInput.dispatchEvent(new Event("input", { bubbles: true }));
    expect(workflow().settings.retry_policy?.max_attempts).toBe(0);
  });

  test("updates retry backoff ms and rejects invalid input as zero", () => {
    const { workflow } = renderPanel(makeWorkflow());
    const backoffInput = container.querySelectorAll('input[type="number"]')[1] as HTMLInputElement;

    backoffInput.value = "2500";
    backoffInput.dispatchEvent(new Event("input", { bubbles: true }));
    expect(workflow().settings.retry_policy?.backoff_ms).toBe(2500);

    backoffInput.value = "not-a-number";
    backoffInput.dispatchEvent(new Event("input", { bubbles: true }));
    expect(workflow().settings.retry_policy?.backoff_ms).toBe(0);
  });
});
