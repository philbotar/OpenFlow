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
      question: "How should I release it?",
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

describe("StructuredAskCard", () => {
  it("renders each question as a plain list of native radio options", () => {
    const { container, dispose } = renderCard();
    try {
      const optionLists = Array.from(
        container.querySelectorAll<HTMLUListElement>("ul.structured-ask-options"),
      );

      expect(optionLists).toHaveLength(2);
      expect(optionLists.map((list) => list.querySelectorAll(":scope > li").length)).toEqual([
        2, 2,
      ]);
      expect(container.querySelectorAll('input[type="radio"]')).toHaveLength(4);
      expect(container.textContent).not.toContain("Other");
    } finally {
      dispose();
      container.remove();
    }
  });

  it("requires every question, then submits stable ids with selected labels", async () => {
    const { container, dispose, handleSubmitStructuredInput } = renderCard();
    try {
      const submit = buttonNamed(container, "Submit answers");
      expect(submit?.disabled).toBe(true);

      radioWithValue(container, "Production")?.click();
      radioWithValue(container, "Gradual")?.click();
      expect(submit?.disabled).toBe(false);

      submit?.click();
      await Promise.resolve();

      expect(handleSubmitStructuredInput).toHaveBeenCalledWith(
        "builder",
        "Structured answers:\n- target_env: Production\n- rollout: Gradual",
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
      radioWithValue(container, "Gradual")?.click();
      setCurrentRequest(structuredClone(request));

      expect(radioWithValue(container, "Production")?.checked).toBe(true);
      expect(radioWithValue(container, "Gradual")?.checked).toBe(true);
    } finally {
      dispose();
      container.remove();
    }
  });
});
