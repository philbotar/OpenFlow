// @vitest-environment jsdom
import { render } from "solid-js/web";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { AgentConfigForm } from "./AgentConfigForm";

describe("AgentConfigForm", () => {
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

  test("projects provider selection through its public callback", () => {
    const onProviderChange = vi.fn();
    dispose = render(
      () => (
        <AgentConfigForm
          providerId=""
          providerOptions={[
            { value: "", label: "Use shared provider (OpenAI)" },
            { value: "openai", label: "OpenAI" },
            { value: "anthropic", label: "Anthropic" },
          ]}
          onProviderChange={onProviderChange}
          model=""
          onModelChange={vi.fn()}
          systemPrompt=""
          onSystemPromptChange={vi.fn()}
          taskPrompt=""
          onTaskPromptChange={vi.fn()}
          schemaJson="{}"
          onSchemaChange={vi.fn()}
          knownModels={() => ["gpt-5"]}
          defaultModel={() => "gpt-5"}
        />
      ),
      container,
    );

    const providerTrigger = [...container.querySelectorAll(".text-select-trigger")].find(
      (button) => button.closest("label")?.textContent?.includes("Provider"),
    ) as HTMLButtonElement | undefined;
    expect(providerTrigger).toBeTruthy();
    providerTrigger!.click();
    const anthropicOption = [...container.querySelectorAll(".text-select-option")].find(
      (element) => element.textContent === "Anthropic",
    ) as HTMLButtonElement;
    anthropicOption.click();

    expect(onProviderChange).toHaveBeenCalledWith("anthropic");
  });
});
