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

describe("StructuredAskCard", () => {
  it("requires every question, then submits stable ids with selected labels", async () => {
    const { container, dispose, handleSubmitStructuredInput } = renderCard();
    try {
      const submit = buttonNamed(container, "Submit answers");
      expect(submit?.disabled).toBe(true);

      buttonNamed(container, "Production")?.click();
      buttonNamed(container, "Gradual")?.click();
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

  it("supports a free-form Other answer", async () => {
    const singleQuestion = { questions: [request.questions[0]] };
    const handleSubmitStructuredInput = vi.fn(async () => {});
    const container = document.createElement("div");
    document.body.appendChild(container);
    const dispose = render(
      () => (
        <AppContext.Provider
          value={{
            handleSubmitStructuredInput,
            readiness: () => ({ ready: true }),
          } as unknown as AppContextValue}
        >
          <StructuredAskCard nodeId="builder" request={singleQuestion} />
        </AppContext.Provider>
      ),
      container,
    );

    try {
      buttonNamed(container, "Other")?.click();
      const input = container.querySelector<HTMLInputElement>(
        'input[aria-label="Other answer for Target"]',
      );
      expect(input).not.toBeNull();
      input!.value = "Preview";
      input!.dispatchEvent(new InputEvent("input", { bubbles: true }));

      buttonNamed(container, "Submit answers")?.click();
      await Promise.resolve();

      expect(handleSubmitStructuredInput).toHaveBeenCalledWith(
        "builder",
        "Structured answers:\n- target_env: Preview",
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
      buttonNamed(container, "Production")?.click();
      buttonNamed(container, "Gradual")?.click();
      setCurrentRequest(structuredClone(request));

      expect(buttonNamed(container, "Production")?.getAttribute("aria-checked")).toBe(
        "true",
      );
      expect(buttonNamed(container, "Gradual")?.getAttribute("aria-checked")).toBe(
        "true",
      );
    } finally {
      dispose();
      container.remove();
    }
  });
});
