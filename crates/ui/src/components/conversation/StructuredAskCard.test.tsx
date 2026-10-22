// @vitest-environment jsdom
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { describe, expect, it, vi } from "vitest";
import { AppContext, type AppContextValue } from "../../context/AppContext";
import type { StructuredUserInput } from "../../lib/types";
import { StructuredAskCard } from "./StructuredAskCard";

const request: StructuredUserInput = {
  questions: [
    {
      id: "target_env",
      header: "Target",
      question: "Which environment should I target?",
      selectionMode: "single",
      options: [
        {
          label: "Staging",
          description: "Use the shared staging environment.",
        },
        {
          label: "Production",
          description: "Use the live production environment.",
        },
      ],
    },
    {
      id: "rollout",
      header: "Rollout",
      question: "Which rollout methods should I use?",
      selectionMode: "multiple",
      options: [
        {
          label: "Gradual",
          description: "Increase traffic in controlled steps.",
        },
        {
          label: "Immediate",
          description: "Send all traffic to the new release.",
        },
      ],
    },
  ],
};

function renderCard(handleSubmitStructuredInput = vi.fn(async () => {})) {
  const container = document.createElement("div");
  document.body.appendChild(container);
  const context = {
    handleSubmitStructuredInput,
    readiness: () => ({ ready: true }),
  } as unknown as AppContextValue;
  const dispose = render(
    () => (
      <AppContext.Provider value={context}>
        <StructuredAskCard nodeId="builder" request={request} />
      </AppContext.Provider>
    ),
    container,
  );
  return { container, dispose, handleSubmitStructuredInput };
}

function buttonNamed(container: HTMLElement, name: string) {
  return Array.from(container.querySelectorAll("button")).find(
    (button) => button.textContent?.trim().startsWith(name),
  );
}

function radioWithValue(container: HTMLElement, value: string) {
  return container.querySelector<HTMLInputElement>(
    `input[type="radio"][value="${value}"]`,
  );
}

function checkboxWithValue(container: HTMLElement, value: string) {
  return container.querySelector<HTMLInputElement>(
    `input[type="checkbox"][value="${value}"]`,
  );
}

describe("StructuredAskCard", () => {
  it("paginates questions while keeping each question a plain list of radio options", () => {
    const { container, dispose } = renderCard();
    try {
      expect(container.textContent).toContain("Question 1 of 2");
      expect(container.textContent).toContain("Which environment should I target?");
      expect(container.textContent).not.toContain("Which rollout methods should I use?");
      expect(container.querySelectorAll("ul.structured-ask-options")).toHaveLength(1);
      expect(container.querySelectorAll('input[type="radio"]')).toHaveLength(2);
      expect(container.querySelectorAll('input[type="checkbox"]')).toHaveLength(0);
      expect(container.textContent).not.toContain("Other");

      const next = buttonNamed(container, "Next");
      expect(next?.disabled).toBe(true);
      radioWithValue(container, "Production")?.click();
      expect(next?.disabled).toBe(false);
      next?.click();

      expect(container.textContent).toContain("Question 2 of 2");
      expect(container.textContent).toContain("Which rollout methods should I use?");
      expect(container.textContent).toContain("Select one or more.");
      expect(container.textContent).not.toContain("Which environment should I target?");
      expect(container.querySelectorAll('input[type="radio"]')).toHaveLength(0);
      expect(container.querySelectorAll('input[type="checkbox"]')).toHaveLength(2);

      buttonNamed(container, "Back")?.click();
      expect(radioWithValue(container, "Production")?.checked).toBe(true);
    } finally {
      dispose();
      container.remove();
    }
  });

  it("requires every question, then submits stable ids with selected labels", async () => {
    const { container, dispose, handleSubmitStructuredInput } = renderCard();
    try {
      expect(buttonNamed(container, "Submit answers")).toBeUndefined();

      radioWithValue(container, "Production")?.click();
      buttonNamed(container, "Next")?.click();
      const submit = buttonNamed(container, "Submit answers");
      expect(submit?.disabled).toBe(true);

      checkboxWithValue(container, "Gradual")?.click();
      expect(submit?.disabled).toBe(false);
      checkboxWithValue(container, "Gradual")?.click();
      expect(submit?.disabled).toBe(true);

      checkboxWithValue(container, "Immediate")?.click();
      checkboxWithValue(container, "Gradual")?.click();
      expect(submit?.disabled).toBe(false);

      submit?.click();
      await Promise.resolve();

      expect(handleSubmitStructuredInput).toHaveBeenCalledWith(
        "builder",
        'Structured answers:\n- target_env: Production\n- rollout: ["Gradual","Immediate"]',
      );
    } finally {
      dispose();
      container.remove();
    }
  });

  it("keeps selections when an equivalent run-state snapshot arrives", () => {
    const [currentRequest, setCurrentRequest] = createSignal(request);
    const container = document.createElement("div");
    document.body.appendChild(container);
    const dispose = render(
      () => (
        <AppContext.Provider
          value={{
            handleSubmitStructuredInput: async () => {},
            readiness: () => ({ ready: true }),
          } as unknown as AppContextValue}
        >
          <StructuredAskCard nodeId="builder" request={currentRequest()} />
        </AppContext.Provider>
      ),
      container,
    );

    try {
      radioWithValue(container, "Production")?.click();
      buttonNamed(container, "Next")?.click();
      checkboxWithValue(container, "Gradual")?.click();
      setCurrentRequest(structuredClone(request));

      expect(container.textContent).toContain("Question 2 of 2");
      expect(checkboxWithValue(container, "Gradual")?.checked).toBe(true);
      buttonNamed(container, "Back")?.click();
      expect(radioWithValue(container, "Production")?.checked).toBe(true);
    } finally {
      dispose();
      container.remove();
    }
  });
});
